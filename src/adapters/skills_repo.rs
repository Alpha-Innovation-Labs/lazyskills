use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

#[derive(Clone, Debug)]
pub struct SkillDoc {
    pub title: String,
    pub relative_path: String,
    pub content: String,
}

pub fn load_skill_docs(root: &Path) -> anyhow::Result<Vec<SkillDoc>> {
    let mut markdown_paths = Vec::new();
    collect_markdown_files(root, &mut markdown_paths)?;

    markdown_paths.sort();

    let mut docs = Vec::new();
    for path in markdown_paths {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let title = relative_path
            .trim_end_matches(".md")
            .trim_end_matches("/SKILL")
            .replace("/", " > ");

        docs.push(SkillDoc {
            title,
            relative_path,
            content,
        });
    }

    Ok(docs)
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to read directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_markdown_files(&path, out)?;
            continue;
        }

        let is_markdown = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if is_markdown {
            out.push(path);
        }
    }

    Ok(())
}
