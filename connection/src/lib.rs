use connection_ssh::{open_ssh_tunnel, register_ssh_tunnel};
use database::DatabaseDriver;
use driver_clickhouse::ClickHouseDriver;
use driver_postgres::{PgConfig, PgDriver};
use driver_sqlite::SqliteDriver;
use models::{ClickHouseFormData, ConnectionRequest, DatabaseConnection, DatabaseError};
use reqwest::Url;

pub use connection_ssh::release_ssh_tunnel;

pub async fn connect_to_db(
    request: ConnectionRequest,
) -> Result<DatabaseConnection, DatabaseError> {
    let session_key = request.identity_key();

    match request {
        ConnectionRequest::Sqlite(data) => {
            let pool = SqliteDriver::connect(data.path)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(DatabaseConnection::Sqlite(pool))
        }
        ConnectionRequest::Postgres(mut data) => {
            let tunnel = if let Some(config) = data.ssh_tunnel.as_ref() {
                if !config.is_configured() {
                    return Err(DatabaseError::Tunnel(
                        "SSH tunnel is enabled, but SSH host or username is empty".to_string(),
                    ));
                }

                if looks_like_postgres_dsn(&data.host) {
                    return Err(DatabaseError::Tunnel(
                        "SSH tunnel is not supported with PostgreSQL DSN input. Use host and port fields.".to_string(),
                    ));
                }

                let remote_host = normalize_postgres_host(&data.host);
                let remote_port = if data.port == 0 { 5432 } else { data.port };
                let tunnel = open_ssh_tunnel(config, &remote_host, remote_port)
                    .await
                    .map_err(DatabaseError::Tunnel)?;
                data.host = "127.0.0.1".to_string();
                data.port = tunnel.local_port;
                Some(tunnel)
            } else {
                None
            };

            // Use a closure to ensure tunnel cleanup on error
            let connect_postgres = || async {
                let config = PgConfig {
                    host: data.host.clone(),
                    port: data.port,
                    username: data.username.clone(),
                    password: data.password.clone(),
                    database: data.database.clone(),
                };
                PgDriver::connect(config)
                    .await
                    .map_err(DatabaseError::Postgres)
                    .map(DatabaseConnection::Postgres)
            };

            let result = connect_postgres().await;

            if let Some(tunnel) = tunnel {
                if result.is_ok() {
                    register_ssh_tunnel(session_key, tunnel);
                } else {
                    // Clean up the tunnel if connection failed
                    release_ssh_tunnel(&session_key);
                }
            }

            result
        }
        ConnectionRequest::ClickHouse(mut data) => {
            let tunnel = if let Some(config) = data.ssh_tunnel.as_ref() {
                if !config.is_configured() {
                    return Err(DatabaseError::Tunnel(
                        "SSH tunnel is enabled, but SSH host or username is empty".to_string(),
                    ));
                }

                let target = parse_clickhouse_target(&data)?;
                let tunnel = open_ssh_tunnel(config, &target.remote_host, target.remote_port)
                    .await
                    .map_err(DatabaseError::Tunnel)?;
                data.host = target.connect_host(tunnel.local_port);
                data.port = tunnel.local_port;
                Some(tunnel)
            } else {
                None
            };

            // Use a closure to ensure tunnel cleanup on error
            let connect_clickhouse = || async {
                ClickHouseDriver::connect(data.clone())
                    .await
                    .map_err(DatabaseError::ClickHouse)
                    .map(DatabaseConnection::ClickHouse)
            };

            let result = connect_clickhouse().await;

            if let Some(tunnel) = tunnel {
                if result.is_ok() {
                    register_ssh_tunnel(session_key, tunnel);
                } else {
                    // Clean up the tunnel if connection failed
                    release_ssh_tunnel(&session_key);
                }
            }

            result
        }
    }
}

fn looks_like_postgres_dsn(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("postgres://") || value.starts_with("postgresql://")
}

fn normalize_postgres_host(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        "localhost".to_string()
    } else {
        host.to_string()
    }
}

struct ClickHouseTarget {
    remote_host: String,
    remote_port: u16,
    scheme: Option<String>,
}

impl ClickHouseTarget {
    fn connect_host(&self, local_port: u16) -> String {
        match self.scheme.as_deref() {
            Some(scheme) => format!("{scheme}://127.0.0.1:{local_port}"),
            None => "127.0.0.1".to_string(),
        }
    }
}

fn parse_clickhouse_target(data: &ClickHouseFormData) -> Result<ClickHouseTarget, DatabaseError> {
    let host = data.host.trim();
    if host.is_empty() {
        return Err(DatabaseError::Tunnel(
            "ClickHouse host is empty, nothing to tunnel to".to_string(),
        ));
    }

    if host.starts_with("http://") || host.starts_with("https://") {
        let url = Url::parse(host)
            .map_err(|err| DatabaseError::Tunnel(format!("invalid ClickHouse URL: {err}")))?;
        if url.scheme().eq_ignore_ascii_case("https") {
            return Err(DatabaseError::Tunnel(
                "ClickHouse HTTPS endpoints are not supported through the current SSH tunnel implementation because TLS host validation would target 127.0.0.1. Use an HTTP endpoint over SSH or connect directly.".to_string(),
            ));
        }
        let remote_host = url
            .host_str()
            .ok_or_else(|| DatabaseError::Tunnel("ClickHouse URL has no host".to_string()))?
            .to_string();
        let remote_port =
            url.port_or_known_default()
                .unwrap_or(if data.port == 0 { 8123 } else { data.port });
        return Ok(ClickHouseTarget {
            remote_host,
            remote_port,
            scheme: Some(url.scheme().to_string()),
        });
    }

    Ok(ClickHouseTarget {
        remote_host: host.to_string(),
        remote_port: if data.port == 0 { 8123 } else { data.port },
        scheme: None,
    })
}
