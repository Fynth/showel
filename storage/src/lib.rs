mod chat;
mod fs_store;
mod history;
mod saved_queries;
mod settings;

pub use chat::{
    create_chat_thread, delete_chat_thread, load_chat_thread_messages, load_chat_threads,
    save_chat_thread_snapshot,
};
pub use history::{
    append_query_history, load_query_history, load_saved_connections, load_session_state,
    load_session_state_sync, save_connection_request, save_session_state, save_session_state_sync,
};
pub use saved_queries::{delete_saved_query, load_saved_queries, save_saved_query};
pub use settings::{
    load_app_ui_settings, load_sql_format_settings, save_app_ui_settings, save_sql_format_settings,
};

pub fn acp_workspace_root() -> Result<std::path::PathBuf, String> {
    let path = fs_store::acp_workspace_root();
    std::fs::create_dir_all(&path)
        .map_err(|err| format!("failed to create ACP workspace {}: {err}", path.display()))?;
    Ok(path)
}
