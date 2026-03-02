use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::config::user_config::user_config_path;

fn cache_root_dir() -> std::io::Result<PathBuf> {
    let config_path = user_config_path()?;
    let parent = config_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "user config parent directory is unavailable",
        )
    })?;
    Ok(parent.join("cache"))
}

fn hash_key(key: &str) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn cache_file_path(namespace: &str, key: &str) -> std::io::Result<PathBuf> {
    Ok(cache_root_dir()?
        .join(namespace)
        .join(format!("{}.json", hash_key(key))))
}

pub fn read_json<T: DeserializeOwned>(namespace: &str, key: &str) -> Option<T> {
    let path = cache_file_path(namespace, key).ok()?;
    if !path.exists() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn write_json<T: Serialize>(namespace: &str, key: &str, value: &T) -> std::io::Result<()> {
    let path = cache_file_path(namespace, key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string(value)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    fs::write(path, serialized)
}
