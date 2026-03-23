use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};
use tokio::fs;

fn storage_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("showel")
}

pub(crate) fn saved_connections_path() -> PathBuf {
    storage_root().join("saved_connections.json")
}

pub(crate) fn chat_db_path() -> PathBuf {
    storage_root().join("showel.db")
}

pub(crate) fn query_history_path() -> PathBuf {
    storage_root().join("query_history.json")
}

pub(crate) fn saved_queries_path() -> PathBuf {
    storage_root().join("saved_queries.json")
}

pub(crate) fn sql_format_settings_path() -> PathBuf {
    storage_root().join("sql_format_settings.json")
}

pub(crate) fn app_ui_settings_path() -> PathBuf {
    storage_root().join("app_ui_settings.json")
}

pub(crate) fn session_state_path() -> PathBuf {
    storage_root().join("session_state.json")
}

async fn ensure_storage_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("failed to create storage dir: {err}"))?;
    }
    Ok(())
}

pub(crate) async fn read_json_file<T>(path: PathBuf) -> Result<T, String>
where
    T: DeserializeOwned + Default,
{
    match read_text_file(&path).await? {
        Some(content) => serde_json::from_str(&content)
            .map_err(|err| format!("failed to parse {}: {err}", path.display())),
        None => Ok(T::default()),
    }
}

pub(crate) async fn write_json_file<T>(path: PathBuf, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    ensure_storage_dir(&path).await?;
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(&path, json)
        .await
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

pub(crate) async fn read_text_file(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("failed to read {}: {err}", path.display())),
    }
}
