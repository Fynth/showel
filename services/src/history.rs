use keyring::{Entry, Error as KeyringError};
use models::{
    ClickHouseFormData, ConnectionRequest, PostgresFormData, QueryHistoryItem, SavedConnection,
    SqliteFormData,
};
use serde::{Deserialize, Serialize};

use crate::storage::{
    query_history_path, read_json_file, read_text_file, saved_connections_path, write_json_file,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ClickHouseConnectionMetadata {
    host: String,
    port: u16,
    username: String,
    database: String,
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

async fn persist_saved_connections(
    saved_connections: &[SavedConnection],
    previous_names: &[String],
) -> Result<(), String> {
    for saved_connection in saved_connections {
        sync_connection_secret(saved_connection)?;
    }

    for removed_name in previous_names {
        if !saved_connections
            .iter()
            .any(|saved| &saved.name == removed_name)
        {
            delete_secret(removed_name)?;
        }
    }

    let persisted = saved_connections
        .iter()
        .cloned()
        .map(to_persisted_connection)
        .collect::<Vec<_>>();
    write_json_file(saved_connections_path(), &persisted).await
}

fn hydrate_saved_connections(
    persisted: Vec<PersistedSavedConnection>,
) -> Result<Vec<SavedConnection>, String> {
    persisted
        .into_iter()
        .map(|saved_connection| {
            let password = load_secret(&saved_connection.name)?;
            let request = match saved_connection.request {
                PersistedConnectionRequest::Sqlite(data) => ConnectionRequest::Sqlite(data),
                PersistedConnectionRequest::Postgres(data) => {
                    ConnectionRequest::Postgres(PostgresFormData {
                        host: data.host,
                        port: data.port,
                        username: data.username,
                        password: password.unwrap_or_default(),
                        database: data.database,
                    })
                }
                PersistedConnectionRequest::ClickHouse(data) => {
                    ConnectionRequest::ClickHouse(ClickHouseFormData {
                        host: data.host,
                        port: data.port,
                        username: data.username,
                        password: password.unwrap_or_default(),
                        database: data.database,
                    })
                }
            };

            Ok(SavedConnection {
                name: saved_connection.name,
                request,
            })
        })
        .collect()
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
            })
        }
        ConnectionRequest::ClickHouse(data) => {
            PersistedConnectionRequest::ClickHouse(ClickHouseConnectionMetadata {
                host: data.host,
                port: data.port,
                username: data.username,
                database: data.database,
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
