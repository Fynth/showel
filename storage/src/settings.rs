use models::{AppUiSettings, SqlFormatSettings};

use crate::fs_store::{
    app_ui_settings_path, read_json_file, sql_format_settings_path, write_json_file,
};

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
