use keyring::{Entry, Error as KeyringError};
use models::{AppUiSettings, SqlFormatSettings};

use crate::fs_store::{
    app_ui_settings_path, read_json_file, sql_format_settings_path, write_json_file,
};
use crate::secrets::{delete_fallback_secret, load_fallback_secret, save_fallback_secret};

const CODESTRAL_KEYRING_SERVICE: &str = "shovel.codestral";
const CODESTRAL_KEYRING_ACCOUNT: &str = "default";

pub async fn load_app_ui_settings() -> Result<AppUiSettings, String> {
    read_json_file(app_ui_settings_path()).await
}

pub async fn save_app_ui_settings(settings: AppUiSettings) -> Result<(), String> {
    write_json_file(app_ui_settings_path(), &settings).await
}

pub async fn load_sql_format_settings() -> Result<SqlFormatSettings, String> {
    read_json_file(sql_format_settings_path()).await
}

pub async fn save_sql_format_settings(settings: SqlFormatSettings) -> Result<(), String> {
    write_json_file(sql_format_settings_path(), &settings).await
}

pub async fn load_codestral_api_key() -> Result<String, String> {
    tokio::task::spawn_blocking(load_codestral_api_key_sync)
        .await
        .map_err(|err| format!("failed to join CodeStral secret task: {err}"))?
}

pub async fn save_codestral_api_key(api_key: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || save_codestral_api_key_sync(&api_key))
        .await
        .map_err(|err| format!("failed to join CodeStral secret task: {err}"))?
}

fn load_codestral_api_key_sync() -> Result<String, String> {
    let entry = Entry::new(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT);
    match entry {
        Ok(entry) => match entry.get_password() {
            Ok(api_key) => {
                let _ =
                    delete_fallback_secret(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT);
                Ok(api_key)
            }
            Err(KeyringError::NoEntry) => Ok(load_fallback_secret(
                CODESTRAL_KEYRING_SERVICE,
                CODESTRAL_KEYRING_ACCOUNT,
            )?
            .unwrap_or_default()),
            Err(_) => Ok(load_fallback_secret(
                CODESTRAL_KEYRING_SERVICE,
                CODESTRAL_KEYRING_ACCOUNT,
            )?
            .unwrap_or_default()),
        },
        Err(_) => Ok(
            load_fallback_secret(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT)?
                .unwrap_or_default(),
        ),
    }
}

fn save_codestral_api_key_sync(api_key: &str) -> Result<(), String> {
    let entry = Entry::new(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT);

    if api_key.trim().is_empty() {
        if let Ok(entry) = entry {
            match entry.delete_credential() {
                Ok(()) | Err(KeyringError::NoEntry) => {}
                Err(_) => {}
            }
        }
        delete_fallback_secret(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT)
    } else if let Ok(entry) = entry {
        match entry.set_password(api_key) {
            Ok(()) => {
                let _ =
                    delete_fallback_secret(CODESTRAL_KEYRING_SERVICE, CODESTRAL_KEYRING_ACCOUNT);
                Ok(())
            }
            Err(_) => save_fallback_secret(
                CODESTRAL_KEYRING_SERVICE,
                CODESTRAL_KEYRING_ACCOUNT,
                api_key,
            ),
        }
    } else {
        save_fallback_secret(
            CODESTRAL_KEYRING_SERVICE,
            CODESTRAL_KEYRING_ACCOUNT,
            api_key,
        )
    }
}
