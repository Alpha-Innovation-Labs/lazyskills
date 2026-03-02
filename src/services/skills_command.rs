use std::process::Command;

use crate::config::{SkillsCommandConfig, SkillsCommandMode};

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
