use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const APP_CONFIG_PATH: &str = ".agents/skills-tui-config.json";
pub const SKILLS_CONFIG_VERSION: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillsCommandMode {
    Global,
    Npx,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillsCommandConfig {
    pub mode: SkillsCommandMode,
    pub global_command: String,
    pub npx_command: String,
    pub npx_package: String,
    pub expected_identity_substring: String,
    pub global_command_verified: bool,
    pub global_command_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u8,
    pub skills_command: SkillsCommandConfig,
    #[serde(default)]
    pub favorite_skills: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: SKILLS_CONFIG_VERSION,
            skills_command: SkillsCommandConfig {
                mode: SkillsCommandMode::Global,
                global_command: "skills".to_string(),
                npx_command: "npx".to_string(),
                npx_package: "skills".to_string(),
                expected_identity_substring: "skills".to_string(),
                global_command_verified: false,
                global_command_version: None,
            },
            favorite_skills: Vec::new(),
        }
    }
}

pub struct StartupConfigOutcome {
    pub config: AppConfig,
    pub existing_config: bool,
    pub verified_version: Option<String>,
}

pub fn app_config_path() -> PathBuf {
    PathBuf::from(APP_CONFIG_PATH)
}

pub fn write_app_config(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{}\n", serialized))
}

pub fn load_app_config() -> io::Result<AppConfig> {
    let path = app_config_path();
    let raw = fs::read_to_string(&path)?;
    let config: AppConfig = serde_json::from_str(&raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(config)
}

pub fn persist_app_config(config: &AppConfig) -> io::Result<()> {
    write_app_config(&app_config_path(), config)
}

pub fn verify_global_skills_command(cfg: &SkillsCommandConfig) -> Option<String> {
    crate::services::skills_command::verify_global_skills_command(cfg)
}

pub fn initialize_skills_command_config() -> io::Result<StartupConfigOutcome> {
    let path = app_config_path();
    if path.exists() {
        return Ok(StartupConfigOutcome {
            config: load_app_config()?,
            existing_config: true,
            verified_version: None,
        });
    }

    let mut config = AppConfig::default();
    let verified_version = verify_global_skills_command(&config.skills_command);
    config.skills_command.global_command_verified = verified_version.is_some();
    config.skills_command.global_command_version = verified_version.clone();
    if config.skills_command.global_command_verified {
        config.skills_command.mode = SkillsCommandMode::Global;
        persist_app_config(&config)?;
    }

    Ok(StartupConfigOutcome {
        config,
        existing_config: false,
        verified_version,
    })
}
