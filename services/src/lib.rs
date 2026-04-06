//! Services facade — unified API for Showel operations.
//!
//! This crate re-exports the public APIs from individual service crates
//! to provide a single dependency point. Other crates (especially `ui`)
//! can depend on `services` instead of importing from each crate individually.
//!
//! # Crates covered
//!
//! - **connection** — database connection management
//! - **explorer** — schema exploration and table metadata
//! - **query** — query execution, formatting, import/export, and table editing
//! - **storage** — local persistence for settings, sessions, queries, and chat
//! - **acp** — ACP agent runtime, registry, and context building

// --- Connection management ---

pub use connection::connect_to_db;

// --- Schema exploration ---

pub use explorer::{describe_table, load_connection_tree, load_table_columns};

// --- Query execution and table editing ---

pub use query::{
    create_table, delete_table_row, drop_table, execute_query, execute_query_page,
    export_query_page_csv, export_query_page_json, export_query_page_xlsx, format_sql,
    import_csv_into_table, insert_table_row, insert_table_row_with_values, load_table_preview_page,
    next_table_primary_key_id, update_table_cell,
};

// --- Persistence ---

pub use storage::{
    append_query_history, create_chat_thread, delete_chat_thread, delete_saved_query,
    load_chat_thread_messages, load_chat_threads, load_query_history, load_saved_connections,
    load_saved_queries, load_session_state, load_session_state_sync, save_chat_thread_snapshot,
    save_connection_request, save_saved_query, save_session_state, save_session_state_sync,
};

// --- ACP agent runtime ---

pub use acp::{
    build_acp_database_context, cancel_acp_prompt, connect_acp_agent, disconnect_acp_agent,
    drain_acp_events, install_acp_registry_agent, load_acp_registry_agents, respond_acp_permission,
    send_acp_prompt, warm_acp_database_schema_context,
};
