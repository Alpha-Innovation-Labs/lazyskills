use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::{FavoriteSkill, SkillsCommandConfig, SkillsCommandMode};

pub fn run_configured_skills_command(
    config: &SkillsCommandConfig,
    args: &[&str],
) -> Result<String, String> {
    let mut command = if matches!(config.mode, SkillsCommandMode::Npx) {
        let mut cmd = Command::new(&config.npx_command);
        cmd.arg(&config.npx_package);
        cmd
    } else {
        Command::new(&config.global_command)
    };

    let output = command
        .args(args)
        .output()
        .map_err(|err| format!("Failed to run command: {}", err))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok("ok".to_string())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Command exited with status {}", output.status)
        };
        Err(msg)
    }
}

pub fn install_skill_from_slug(config: &SkillsCommandConfig, slug: &str) -> Result<String, String> {
    install_skill_from_slug_with_agents(config, slug, &[])
}

pub fn install_skill_from_slug_with_agents(
    config: &SkillsCommandConfig,
    slug: &str,
    agents: &[String],
) -> Result<String, String> {
    let mut parts = slug.split('/');
    let owner = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let repo = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let skill = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let source = format!("{owner}/{repo}");

    let mut args = vec![
        "add".to_string(),
        source,
        "--skill".to_string(),
        skill.to_string(),
        "-y".to_string(),
    ];
    if !agents.is_empty() {
        args.push("--agent".to_string());
        args.extend(agents.iter().cloned());
    }

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_configured_skills_command(config, &arg_refs)
}

pub fn install_skill_from_slug_global(
    config: &SkillsCommandConfig,
    slug: &str,
) -> Result<String, String> {
    let mut parts = slug.split('/');
    let owner = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let repo = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let skill = parts
        .next()
        .ok_or_else(|| format!("Invalid install slug: {slug}"))?;
    let source = format!("{owner}/{repo}");
    run_configured_skills_command(config, &["add", &source, "--skill", skill, "-y", "-g"])
}

pub fn remove_skill_noninteractive(
    config: &SkillsCommandConfig,
    skill_name: &str,
) -> Result<String, String> {
    remove_skill_noninteractive_scoped(config, skill_name, false)
}

pub fn remove_skill_noninteractive_scoped(
    config: &SkillsCommandConfig,
    skill_name: &str,
    global_scope: bool,
) -> Result<String, String> {
    if global_scope {
        run_configured_skills_command(config, &["remove", "-g", skill_name, "-y"])
    } else {
        run_configured_skills_command(config, &["remove", skill_name, "-y"])
    }
}

pub fn patch_project_lock_after_remove(
    cwd: &Path,
    favorite: &FavoriteSkill,
) -> Result<bool, String> {
    let lock_path = cwd.join("skills-lock.json");
    if !lock_path.exists() {
        return Ok(false);
    }

    let raw = fs::read_to_string(&lock_path)
        .map_err(|err| format!("Failed to read {}: {}", lock_path.display(), err))?;
    let mut root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|err| format!("Invalid {}: {}", lock_path.display(), err))?;

    let Some(skills) = root.get_mut("skills").and_then(|v| v.as_object_mut()) else {
        return Ok(false);
    };

    let install_name = favorite
        .install_skill
        .rsplit('/')
        .next()
        .unwrap_or(&favorite.install_skill)
        .to_string();

    let mut remove_keys = Vec::new();
    for key in [favorite.display_slug.as_str(), install_name.as_str()] {
        if !key.is_empty() && skills.contains_key(key) {
            remove_keys.push(key.to_string());
        }
    }

    for (key, entry) in skills.iter() {
        let source = entry
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let candidate = format!("{}/{}", source.trim_end_matches(".git"), key);
        if !favorite.install_skill.is_empty() && candidate == favorite.install_skill {
            remove_keys.push(key.clone());
        }
    }

    remove_keys.sort();
    remove_keys.dedup();

    if remove_keys.is_empty() {
        return Ok(false);
    }

    for key in &remove_keys {
        let _ = skills.remove(key);
    }

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|err| format!("Failed to serialize {}: {}", lock_path.display(), err))?;
    fs::write(&lock_path, format!("{}\n", serialized))
        .map_err(|err| format!("Failed to write {}: {}", lock_path.display(), err))?;

    Ok(true)
}

pub fn verify_global_skills_command(cfg: &SkillsCommandConfig) -> Option<String> {
    let identity = cfg.expected_identity_substring.to_ascii_lowercase();

    for args in [["--version"].as_slice(), ["version"].as_slice()] {
        if let Some(output) = run_command_for_output(&cfg.global_command, args) {
            let lowered = output.to_ascii_lowercase();
            if lowered.contains(&identity) || looks_like_semverish(&output) {
                return Some(output.lines().next().unwrap_or("skills").trim().to_string());
            }
        }
    }

    None
}

pub fn install_global_skills_cli() -> Result<(), String> {
    let output = Command::new("npm")
        .args(["install", "-g", "skills"])
        .output()
        .map_err(|err| format!("Failed to run npm: {}", err))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("npm exited with status {}", output.status)
    };

    let concise = details
        .lines()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(220)
        .collect::<String>();

    Err(format!(
        "Global install failed (`npm install -g skills`). {}",
        concise
    ))
}

fn run_command_for_output(bin: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(bin).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Some(stderr);
    }

    None
}

fn looks_like_semverish(text: &str) -> bool {
    let compact = text
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '.' || *ch == '-')
        .collect::<String>();
    compact.chars().any(|ch| ch.is_ascii_digit()) && compact.contains('.')
}
