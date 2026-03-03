use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

const REPO_ROOT: &str = ".";
const SRC_ROOT: &str = "src";
const DOCS_ROOT: &str = "docs/content/docs";
const GENERATED_SUBTREE: &str = "reference";

#[derive(Clone, Debug)]
struct SymbolDoc {
    module_path: Vec<String>,
    kind: String,
    name: String,
    signature: String,
    docs_markdown: String,
    source_path: String,
    symbol_path: String,
}

fn main() -> Result<()> {
    let mode = env::args().nth(1).unwrap_or_else(|| "sync".to_string());
    match mode.as_str() {
        "sync" => run_sync(false),
        "check" => run_sync(true),
        other => bail!("unknown mode '{other}', expected 'sync' or 'check'"),
    }
}

fn run_sync(check_only: bool) -> Result<()> {
    let repo_root = Path::new(REPO_ROOT);
    let src_root = repo_root.join(SRC_ROOT);
    let docs_root = repo_root.join(DOCS_ROOT);
    let generated_root = docs_root.join(GENERATED_SUBTREE);

    let symbols = collect_symbols(&src_root)?;
    let rendered = render_output_map(&symbols)?;

    if check_only {
        let existing = snapshot_tree(&generated_root)?;
        let drift = diff_maps(&existing, &rendered);
        if !drift.is_empty() {
            eprintln!("docs-sync-check failed; generated docs drift detected:");
            for line in drift {
                eprintln!("  {line}");
            }
            std::process::exit(1);
        }
        println!("docs-sync-check passed; generated reference docs are up to date");
        return Ok(());
    }

    write_generated_tree_atomically(&generated_root, &rendered)?;
    ensure_root_meta(&docs_root)?;
    println!("generated {} symbol reference pages", symbols.len());
    Ok(())
}

fn collect_symbols(src_root: &Path) -> Result<Vec<SymbolDoc>> {
    let mut symbols = Vec::new();

    for entry in WalkDir::new(src_root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if path
            .components()
            .any(|component| component.as_os_str() == "bin")
        {
            continue;
        }

        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let module_path = module_segments(src_root, path)?;
        let source_path = path
            .strip_prefix(REPO_ROOT)
            .unwrap_or(path)
            .display()
            .to_string();

        let mut module_docs = Vec::new();
        let mut symbol_docs = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(doc) = trimmed.strip_prefix("//!") {
                module_docs.push(doc.trim().to_string());
                continue;
            }

            if let Some(doc) = trimmed.strip_prefix("///") {
                symbol_docs.push(doc.trim().to_string());
                continue;
            }

            if trimmed.is_empty() {
                continue;
            }

            if let Some((kind, name, signature)) = parse_symbol(trimmed) {
                let docs_markdown = symbol_docs.join("\n");
                let symbol_path = format!("crate::{}::{}", module_path.join("::"), name);

                symbols.push(SymbolDoc {
                    module_path: module_path.clone(),
                    kind,
                    name,
                    signature,
                    docs_markdown,
                    source_path: source_path.clone(),
                    symbol_path,
                });
                symbol_docs.clear();
                continue;
            }

            if !trimmed.starts_with("#") {
                symbol_docs.clear();
            }
        }

        if !module_docs.is_empty() {
            let module_name = module_path
                .last()
                .cloned()
                .unwrap_or_else(|| "crate".to_string());
            let symbol_path = format!("crate::{}", module_path.join("::"));

            symbols.push(SymbolDoc {
                module_path: module_path.clone(),
                kind: "module".to_string(),
                name: module_name,
                signature: format!("mod {}", module_path.join("::")),
                docs_markdown: module_docs.join("\n"),
                source_path,
                symbol_path,
            });
        }
    }

    symbols.sort_by(|a, b| {
        a.module_path
            .cmp(&b.module_path)
            .then(a.name.cmp(&b.name))
            .then(a.kind.cmp(&b.kind))
    });

    Ok(symbols)
}

fn parse_symbol(line: &str) -> Option<(String, String, String)> {
    let normalized = line.trim_end_matches('{').trim().to_string();
    let regex =
        Regex::new(r"^pub\s+(?:async\s+)?(struct|enum|trait|fn|mod)\s+([A-Za-z_][A-Za-z0-9_]*)")
            .ok()?;
    let captures = regex.captures(&normalized)?;
    let kind = captures.get(1)?.as_str().to_string();
    let name = captures.get(2)?.as_str().to_string();

    Some((kind, name, normalized))
}

fn module_segments(src_root: &Path, file_path: &Path) -> Result<Vec<String>> {
    let relative = file_path
        .strip_prefix(src_root)
        .with_context(|| format!("failed to strip source root for {}", file_path.display()))?;

    let mut segments: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();

    if segments.is_empty() {
        return Ok(vec!["crate".to_string()]);
    }

    let file_name = segments.pop().unwrap_or_default();
    if file_name != "mod.rs" {
        let stem = file_name.trim_end_matches(".rs");
        if stem != "lib" && stem != "main" {
            segments.push(stem.to_string());
        }
    }

    if segments.is_empty() {
        Ok(vec!["crate".to_string()])
    } else {
        Ok(segments)
    }
}

fn render_output_map(symbols: &[SymbolDoc]) -> Result<BTreeMap<String, String>> {
    let mut outputs = BTreeMap::new();
    let mut groups: BTreeMap<Vec<String>, Vec<&SymbolDoc>> = BTreeMap::new();

    for symbol in symbols {
        groups
            .entry(symbol.module_path.clone())
            .or_default()
            .push(symbol);
    }

    for (module_path, items) in &groups {
        let relative_dir = module_path.join("/");
        let mut meta_pages = Vec::new();

        for symbol in items {
            let slug = slugify(&symbol.name);
            let relative_path = format!("{relative_dir}/{slug}.mdx");
            outputs.insert(relative_path.clone(), render_symbol_page(symbol, items));
            meta_pages.push(slug);
        }

        meta_pages.sort();
        let module_title = module_path
            .last()
            .cloned()
            .unwrap_or_else(|| "reference".to_string());
        let meta_content = render_meta_json(&title_case(&module_title), &meta_pages);
        outputs.insert(format!("{relative_dir}/meta.json"), meta_content);
    }

    let mut top_sections = BTreeSet::new();
    for key in outputs.keys() {
        if let Some((section, _)) = key.split_once('/') {
            top_sections.insert(section.to_string());
        }
    }

    outputs.insert(
        "meta.json".to_string(),
        render_meta_json("Reference", &top_sections.into_iter().collect::<Vec<_>>()),
    );

    Ok(outputs)
}

fn render_symbol_page(symbol: &SymbolDoc, siblings: &[&SymbolDoc]) -> String {
    let summary = symbol
        .docs_markdown
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Generated reference entry.")
        .trim()
        .to_string();

    let mut related: Vec<String> = siblings
        .iter()
        .filter(|candidate| candidate.name != symbol.name)
        .map(|candidate| format!("- [{}](./{})", candidate.name, slugify(&candidate.name)))
        .collect();
    related.sort();

    let docs_block = if symbol.docs_markdown.trim().is_empty() {
        "No rustdoc comments were found for this public symbol.".to_string()
    } else {
        format!("```text\n{}\n```", symbol.docs_markdown.trim())
    };

    let related_block = if related.is_empty() {
        "- No related public items in this module.".to_string()
    } else {
        related.join("\n")
    };

    format!(
        "---\ntitle: {title}\ndescription: {summary}\ngenerated: true\nsource_path: {source_path}\nsymbol_path: {symbol_path}\n---\n\n## Signature\n\n```rust\n{signature}\n```\n\n## Documentation\n\n{docs_block}\n\n## Related Items\n\n{related_block}\n",
        title = yaml_quote(&symbol.name),
        summary = yaml_quote(&summary),
        source_path = yaml_quote(&symbol.source_path),
        symbol_path = yaml_quote(&symbol.symbol_path),
        signature = symbol.signature,
    )
}

fn yaml_quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ");
    format!("\"{escaped}\"")
}

fn render_meta_json(title: &str, pages: &[String]) -> String {
    let pages_json = pages
        .iter()
        .map(|page| format!("    \"{page}\""))
        .collect::<Vec<_>>()
        .join(",\n");

    format!("{{\n  \"title\": \"{title}\",\n  \"pages\": [\n{pages_json}\n  ]\n}}\n")
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!(
            "{}{}",
            first.to_ascii_uppercase(),
            chars.collect::<String>()
        ),
        None => value.to_string(),
    }
}

fn write_generated_tree_atomically(
    target_root: &Path,
    outputs: &BTreeMap<String, String>,
) -> Result<()> {
    let parent = target_root
        .parent()
        .with_context(|| format!("target root has no parent: {}", target_root.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create parent directory {}", parent.display()))?;

    let temp_root = parent.join("reference.__tmp");
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root)
            .with_context(|| format!("failed to clear temp output {}", temp_root.display()))?;
    }
    fs::create_dir_all(&temp_root)
        .with_context(|| format!("failed to create temp output {}", temp_root.display()))?;

    for (relative_path, content) in outputs {
        let destination = temp_root.join(relative_path);
        if let Some(dir) = destination.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create output directory {}", dir.display()))?;
        }
        fs::write(&destination, content)
            .with_context(|| format!("failed to write generated file {}", destination.display()))?;
    }

    let backup_root = parent.join("reference.__old");
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root).with_context(|| {
            format!("failed to clear backup directory {}", backup_root.display())
        })?;
    }

    if target_root.exists() {
        fs::rename(target_root, &backup_root)
            .with_context(|| format!("failed to move old output {}", target_root.display()))?;
    }

    fs::rename(&temp_root, target_root)
        .with_context(|| format!("failed to move generated output {}", target_root.display()))?;

    if backup_root.exists() {
        fs::remove_dir_all(&backup_root)
            .with_context(|| format!("failed to remove backup output {}", backup_root.display()))?;
    }

    Ok(())
}

fn snapshot_tree(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let relative = entry
            .path()
            .strip_prefix(root)
            .with_context(|| format!("failed to strip prefix for {}", entry.path().display()))?
            .to_string_lossy()
            .to_string();
        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read {}", entry.path().display()))?;
        files.insert(relative, content);
    }

    Ok(files)
}

fn diff_maps(
    existing: &BTreeMap<String, String>,
    rendered: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut output = Vec::new();
    let mut keys = BTreeSet::new();
    keys.extend(existing.keys().cloned());
    keys.extend(rendered.keys().cloned());

    for key in keys {
        match (existing.get(&key), rendered.get(&key)) {
            (Some(left), Some(right)) if left == right => {}
            (Some(_), Some(_)) => output.push(format!("changed: {key}")),
            (None, Some(_)) => output.push(format!("missing: {key}")),
            (Some(_), None) => output.push(format!("extra: {key}")),
            (None, None) => {}
        }
    }

    output
}

fn ensure_root_meta(docs_root: &Path) -> Result<()> {
    let meta_path = docs_root.join("meta.json");
    let pages: Vec<String> = vec![
        "index".to_string(),
        "overview".to_string(),
        "skills-cli".to_string(),
        "features".to_string(),
        "getting-started".to_string(),
    ];

    fs::write(&meta_path, render_meta_json("Documentation", &pages))
        .with_context(|| format!("failed to write {}", meta_path.display()))?;

    Ok(())
}
