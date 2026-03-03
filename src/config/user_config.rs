use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::default_paths::default_user_data_dir;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FavoriteSkill {
    pub display_slug: String,
    pub install_skill: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FavoriteSkillInput {
    Legacy(String),
    Structured(FavoriteSkill),
}

fn deserialize_favorites<'de, D>(deserializer: D) -> Result<Vec<FavoriteSkill>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<FavoriteSkillInput>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|entry| match entry {
            FavoriteSkillInput::Legacy(slug) => FavoriteSkill {
                display_slug: slug.clone(),
                install_skill: slug,
                source: None,
                source_type: None,
            },
            FavoriteSkillInput::Structured(entry) => entry,
        })
        .collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiPreferences {
    #[serde(default = "default_true")]
    pub show_markdown_pane: bool,
    #[serde(default = "default_true")]
    pub show_detail_pane: bool,
}

fn default_true() -> bool {
    true
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            show_markdown_pane: true,
            show_detail_pane: true,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default, deserialize_with = "deserialize_favorites")]
    pub favorites: Vec<FavoriteSkill>,
    #[serde(default)]
    pub ui: UiPreferences,
}

pub fn user_config_path() -> io::Result<PathBuf> {
    let base = default_user_data_dir()?;

    Ok(base.join("lazyskills").join("config.json"))
}

pub fn load_user_config() -> io::Result<UserConfig> {
    let path = user_config_path()?;
    if !path.exists() {
        return Ok(UserConfig::default());
    }

    let raw = fs::read_to_string(path)?;
    let parsed: UserConfig = serde_json::from_str(&raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(parsed)
}

pub fn persist_user_config(config: &UserConfig) -> io::Result<()> {
    let path = user_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    fs::write(path, format!("{}\n", serialized))
}

#[cfg(test)]
mod tests {
    use super::{persist_user_config, user_config_path, FavoriteSkill, UiPreferences, UserConfig};

    #[test]
    fn persists_config_at_default_location() {
        let path = user_config_path().expect("resolve user config path");
        persist_user_config(&UserConfig {
            favorites: vec![FavoriteSkill {
                display_slug: "ratkit".to_string(),
                install_skill: "ratkit".to_string(),
                source: Some("Alpha-Innovation-Labs/ratkit".to_string()),
                source_type: Some("github".to_string()),
            }],
            ui: UiPreferences {
                show_markdown_pane: true,
                show_detail_pane: true,
            },
        })
        .expect("persist user config");

        assert!(
            path.exists(),
            "expected config file to exist at {}",
            path.display()
        );
        println!("created user config at {}", path.display());
    }
}
