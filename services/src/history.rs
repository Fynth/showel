use keyring::{Entry, Error as KeyringError};
use models::{
    ClickHouseFormData, ConnectionRequest, PostgresFormData, QueryHistoryItem, SavedConnection,
    SqliteFormData, SshTunnelConfig,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::storage::{
    query_history_path, read_json_file, read_text_file, saved_connections_path, session_state_path,
    write_json_file,
};

const MAX_SAVED_CONNECTIONS: usize = 10;
const MAX_HISTORY_ITEMS: usize = 20;
const KEYRING_SERVICE: &str = "showel.connections";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedSavedConnection {
    name: String,
    request: PersistedConnectionRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum PersistedConnectionRequest {
    Sqlite(SqliteFormData),
    Postgres(PostgresConnectionMetadata),
    ClickHouse(ClickHouseConnectionMetadata),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PostgresConnectionMetadata {
    host: String,
    port: u16,
    username: String,
    database: String,
    #[serde(default)]
    ssh_tunnel: Option<SshTunnelConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ClickHouseConnectionMetadata {
    host: String,
    port: u16,
    username: String,
    database: String,
    #[serde(default)]
    ssh_tunnel: Option<SshTunnelConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedSessionState {
    #[serde(default)]
    open_connections: Vec<PersistedSavedConnection>,
    active_connection_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct LegacySessionState {
    #[serde(default)]
    open_requests: Vec<ConnectionRequest>,
    active_connection_name: Option<String>,
}

pub async fn load_saved_connections() -> Result<Vec<SavedConnection>, String> {
    let path = saved_connections_path();
    let Some(content) = read_text_file(&path).await? else {
        return Ok(Vec::new());
    };
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    if let Ok(persisted) = serde_json::from_str::<Vec<PersistedSavedConnection>>(&content) {
        return hydrate_saved_connections(persisted);
    }

    let legacy = serde_json::from_str::<Vec<SavedConnection>>(&content)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    persist_saved_connections(&legacy, &[]).await?;
    Ok(legacy)
}

pub async fn save_connection_request(request: ConnectionRequest) -> Result<(), String> {
    let mut saved_connections = load_saved_connections().await.unwrap_or_default();
    let previous_names = saved_connections
        .iter()
        .map(|saved| saved.name.clone())
        .collect::<Vec<_>>();
    let name = request.display_name();

    saved_connections.retain(|saved| saved.name != name);
    saved_connections.insert(
        0,
        SavedConnection {
            name: name.clone(),
            request,
        },
    );
    if saved_connections.len() > MAX_SAVED_CONNECTIONS {
        saved_connections.truncate(MAX_SAVED_CONNECTIONS);
    }

    persist_saved_connections(&saved_connections, &previous_names).await
}

pub async fn load_query_history() -> Result<Vec<QueryHistoryItem>, String> {
    read_json_file(query_history_path()).await
}

pub async fn append_query_history(item: QueryHistoryItem) -> Result<(), String> {
    let mut history = load_query_history().await.unwrap_or_default();
    history.insert(0, item);
    if history.len() > MAX_HISTORY_ITEMS {
        history.truncate(MAX_HISTORY_ITEMS);
    }

    write_json_file(query_history_path(), &history).await
}

pub async fn save_session_state(
    open_requests: Vec<ConnectionRequest>,
    active_connection_name: Option<String>,
) -> Result<(), String> {
    let (state, secret_errors) =
        build_persisted_session_state(open_requests, active_connection_name);
    write_json_file(session_state_path(), &state)
        .await
        .and_then(|_| finalize_secret_errors("session state", secret_errors))
}

pub async fn load_session_state() -> Result<(Vec<ConnectionRequest>, Option<String>), String> {
    let state = read_session_state_async(session_state_path()).await?;
    let open_requests = hydrate_session_requests(state.open_connections)?;
    Ok((open_requests, state.active_connection_name))
}

pub fn save_session_state_sync(
    open_requests: Vec<ConnectionRequest>,
    active_connection_name: Option<String>,
) -> Result<(), String> {
    let (state, secret_errors) =
        build_persisted_session_state(open_requests, active_connection_name);
    write_session_state_sync(session_state_path(), &state)
        .and_then(|_| finalize_secret_errors("session state", secret_errors))
}

pub fn load_session_state_sync() -> Result<(Vec<ConnectionRequest>, Option<String>), String> {
    let state = read_session_state_sync(session_state_path())?;
    let open_requests = hydrate_session_requests(state.open_connections)?;
    Ok((open_requests, state.active_connection_name))
}

async fn persist_saved_connections(
    saved_connections: &[SavedConnection],
    previous_names: &[String],
) -> Result<(), String> {
    let mut secret_errors = Vec::new();

    for saved_connection in saved_connections {
        if let Err(err) = sync_connection_secret(saved_connection) {
            secret_errors.push(err);
        }
    }

    for removed_name in previous_names {
        if !saved_connections
            .iter()
            .any(|saved| &saved.name == removed_name)
            && let Err(err) = delete_secret(removed_name)
        {
            secret_errors.push(err);
        }
    }

    let persisted = saved_connections
        .iter()
        .cloned()
        .map(to_persisted_connection)
        .collect::<Vec<_>>();
    write_json_file(saved_connections_path(), &persisted).await?;

    if secret_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "saved connection metadata, but secure storage had issues: {}",
            secret_errors.join("; ")
        ))
    }
}

fn hydrate_saved_connections(
    persisted: Vec<PersistedSavedConnection>,
) -> Result<Vec<SavedConnection>, String> {
    let mut restored = Vec::with_capacity(persisted.len());

    for saved_connection in persisted {
        restored.push(hydrate_saved_connection(saved_connection)?);
    }

    Ok(restored)
}

fn hydrate_saved_connection(
    saved_connection: PersistedSavedConnection,
) -> Result<SavedConnection, String> {
    let password = load_secret(&saved_connection.name).ok().flatten();
    let request = match saved_connection.request {
        PersistedConnectionRequest::Sqlite(data) => ConnectionRequest::Sqlite(data),
        PersistedConnectionRequest::Postgres(data) => {
            ConnectionRequest::Postgres(PostgresFormData {
                host: data.host,
                port: data.port,
                username: data.username,
                password: password.clone().unwrap_or_default(),
                database: data.database,
                ssh_tunnel: data.ssh_tunnel,
            })
        }
        PersistedConnectionRequest::ClickHouse(data) => {
            ConnectionRequest::ClickHouse(ClickHouseFormData {
                host: data.host,
                port: data.port,
                username: data.username,
                password: password.unwrap_or_default(),
                database: data.database,
                ssh_tunnel: data.ssh_tunnel,
            })
        }
    };

    Ok(SavedConnection {
        name: saved_connection.name,
        request,
    })
}

fn to_persisted_connection(saved_connection: SavedConnection) -> PersistedSavedConnection {
    let request = match saved_connection.request {
        ConnectionRequest::Sqlite(data) => PersistedConnectionRequest::Sqlite(data),
        ConnectionRequest::Postgres(data) => {
            PersistedConnectionRequest::Postgres(PostgresConnectionMetadata {
                host: data.host,
                port: data.port,
                username: data.username,
                database: data.database,
                ssh_tunnel: data.ssh_tunnel,
            })
        }
        ConnectionRequest::ClickHouse(data) => {
            PersistedConnectionRequest::ClickHouse(ClickHouseConnectionMetadata {
                host: data.host,
                port: data.port,
                username: data.username,
                database: data.database,
                ssh_tunnel: data.ssh_tunnel,
            })
        }
    };

    PersistedSavedConnection {
        name: saved_connection.name,
        request,
    }
}

fn sync_connection_secret(saved_connection: &SavedConnection) -> Result<(), String> {
    match &saved_connection.request {
        ConnectionRequest::Sqlite(_) => {
            delete_secret(&saved_connection.name)?;
        }
        ConnectionRequest::Postgres(data) => {
            store_secret(&saved_connection.name, &data.password)?;
        }
        ConnectionRequest::ClickHouse(data) => {
            store_secret(&saved_connection.name, &data.password)?;
        }
    }

    Ok(())
}

fn store_secret(connection_name: &str, secret: &str) -> Result<(), String> {
    let entry = secret_entry(connection_name)?;
    if secret.is_empty() {
        return delete_secret(connection_name);
    }

    entry
        .set_password(secret)
        .map_err(|err| format!("failed to store secret for {connection_name}: {err}"))
}

fn load_secret(connection_name: &str) -> Result<Option<String>, String> {
    let entry = secret_entry(connection_name)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(format!(
            "failed to read secret for {connection_name} from secure storage: {err}"
        )),
    }
}

fn delete_secret(connection_name: &str) -> Result<(), String> {
    let entry = secret_entry(connection_name)?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) => Err(format!(
            "failed to delete secret for {connection_name} from secure storage: {err}"
        )),
    }
}

fn secret_entry(connection_name: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, &secret_key(connection_name)).map_err(|err| {
        format!("failed to create secure storage entry for {connection_name}: {err}")
    })
}

fn secret_key(connection_name: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in connection_name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("connection-{hash:016x}")
}

fn build_persisted_session_state(
    open_requests: Vec<ConnectionRequest>,
    active_connection_name: Option<String>,
) -> (PersistedSessionState, Vec<String>) {
    let mut secret_errors = Vec::new();
    let open_connections = open_requests
        .into_iter()
        .map(|request| SavedConnection {
            name: request.display_name(),
            request,
        })
        .map(|saved_connection| {
            if let Err(err) = sync_connection_secret(&saved_connection) {
                secret_errors.push(err);
            }
            to_persisted_connection(saved_connection)
        })
        .collect::<Vec<_>>();

    (
        PersistedSessionState {
            open_connections,
            active_connection_name,
        },
        secret_errors,
    )
}

fn hydrate_session_requests(
    open_connections: Vec<PersistedSavedConnection>,
) -> Result<Vec<ConnectionRequest>, String> {
    hydrate_saved_connections(open_connections).map(|saved_connections| {
        saved_connections
            .into_iter()
            .map(|saved_connection| saved_connection.request)
            .collect()
    })
}

async fn read_session_state_async(path: PathBuf) -> Result<PersistedSessionState, String> {
    match read_text_file(&path).await? {
        Some(content) => parse_session_state(&content, &path),
        None => Ok(PersistedSessionState::default()),
    }
}

fn parse_session_state(content: &str, path: &Path) -> Result<PersistedSessionState, String> {
    if content.trim().is_empty() {
        return Ok(PersistedSessionState::default());
    }

    if let Ok(state) = serde_json::from_str::<PersistedSessionState>(content) {
        return Ok(state);
    }

    match serde_json::from_str::<LegacySessionState>(content) {
        Ok(legacy) => Ok(PersistedSessionState {
            open_connections: legacy
                .open_requests
                .into_iter()
                .map(|request| SavedConnection {
                    name: request.display_name(),
                    request,
                })
                .map(to_persisted_connection)
                .collect(),
            active_connection_name: legacy.active_connection_name,
        }),
        Err(err) => Err(format!("failed to parse {}: {err}", path.display())),
    }
}

fn finalize_secret_errors(context: &str, secret_errors: Vec<String>) -> Result<(), String> {
    if secret_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "saved {context}, but secure storage had issues: {}",
            secret_errors.join("; ")
        ))
    }
}

fn read_session_state_sync(path: PathBuf) -> Result<PersistedSessionState, String> {
    match fs::read_to_string(&path) {
        Ok(content) => parse_session_state(&content, &path),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(PersistedSessionState::default()),
        Err(err) => Err(format!("failed to read {}: {err}", path.display())),
    }
}

fn write_session_state_sync(path: PathBuf, value: &PersistedSessionState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create storage dir {}: {err}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(&path, json).map_err(|err| format!("failed to write {}: {err}", path.display()))
}
