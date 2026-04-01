use connection_ssh::{open_ssh_tunnel, register_ssh_tunnel};
use database::DatabaseDriver;
use driver_clickhouse::ClickHouseDriver;
use driver_mysql::{MySqlConfig, MySqlDriver};
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
        ConnectionRequest::MySql(mut data) => {
            let tunnel = if let Some(config) = data.ssh_tunnel.as_ref() {
                if !config.is_configured() {
                    return Err(DatabaseError::Tunnel(
                        "SSH tunnel is enabled, but SSH host or username is empty".to_string(),
                    ));
                }

                if looks_like_mysql_dsn(&data.host) {
                    return Err(DatabaseError::Tunnel(
                        "SSH tunnel is not supported with MySQL DSN input. Use host and port fields.".to_string(),
                    ));
                }

                let (remote_host, embedded_port) = split_mysql_host_and_port(&data.host);
                let remote_host = normalize_mysql_host(&remote_host);
                let remote_port =
                    embedded_port.unwrap_or(if data.port == 0 { 3306 } else { data.port });
                let tunnel = open_ssh_tunnel(config, &remote_host, remote_port)
                    .await
                    .map_err(DatabaseError::Tunnel)?;
                data.host = "127.0.0.1".to_string();
                data.port = tunnel.local_port;
                Some(tunnel)
            } else {
                None
            };

            let connect_mysql = || async {
                let config = MySqlConfig {
                    host: data.host.clone(),
                    port: data.port,
                    username: data.username.clone(),
                    password: data.password.clone(),
                    database: data.database.clone(),
                };
                MySqlDriver::connect(config)
                    .await
                    .map_err(DatabaseError::MySql)
                    .map(DatabaseConnection::MySql)
            };

            let result = connect_mysql().await;

            if let Some(tunnel) = tunnel {
                if result.is_ok() {
                    register_ssh_tunnel(session_key, tunnel);
                } else {
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

fn looks_like_mysql_dsn(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("mysql://") || value.starts_with("mariadb://")
}

fn normalize_mysql_host(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        "localhost".to_string()
    } else {
        host.to_string()
    }
}

fn split_mysql_host_and_port(value: &str) -> (String, Option<u16>) {
    let value = value.trim();
    if value.is_empty() {
        return (String::new(), None);
    }

    if value.starts_with('[')
        && let Some(end_bracket) = value.find(']')
    {
        let host = value[1..end_bracket].to_string();
        let remainder = value[end_bracket + 1..].trim();
        if remainder.is_empty() {
            return (host, None);
        }
        if let Some(port) = remainder
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
        {
            return (host, Some(port));
        }
        return (value.to_string(), None);
    }

    if value.matches(':').count() == 1
        && let Some((host, port)) = value.rsplit_once(':')
        && !host.trim().is_empty()
        && let Ok(port) = port.trim().parse::<u16>()
    {
        return (host.trim().to_string(), Some(port));
    }

    (value.to_string(), None)
}

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── looks_like_postgres_dsn ──────────────────────────────────────

    #[test]
    fn postgres_dsn_detects_postgres_scheme() {
        assert!(looks_like_postgres_dsn("postgres://user@host/db"));
        assert!(looks_like_postgres_dsn("POSTGRES://user@host/db"));
    }

    #[test]
    fn postgres_dsn_detects_postgresql_scheme() {
        assert!(looks_like_postgres_dsn("postgresql://user@host/db"));
        assert!(looks_like_postgres_dsn("PostgreSQL://user@host/db"));
    }

    #[test]
    fn postgres_dsn_rejects_non_dsn() {
        assert!(!looks_like_postgres_dsn("localhost"));
        assert!(!looks_like_postgres_dsn("http://example.com"));
        assert!(!looks_like_postgres_dsn(""));
    }

    #[test]
    fn postgres_dsn_ignores_leading_whitespace() {
        assert!(looks_like_postgres_dsn("  postgres://host/db"));
    }

    // ── normalize_postgres_host ──────────────────────────────────────

    #[test]
    fn normalize_postgres_host_defaults_to_localhost() {
        assert_eq!(normalize_postgres_host(""), "localhost");
        assert_eq!(normalize_postgres_host("   "), "localhost");
    }

    #[test]
    fn normalize_postgres_host_trims_input() {
        assert_eq!(
            normalize_postgres_host("  db.example.com  "),
            "db.example.com"
        );
    }

    #[test]
    fn normalize_postgres_host_preserves_value() {
        assert_eq!(normalize_postgres_host("192.168.1.1"), "192.168.1.1");
    }

    // ── looks_like_mysql_dsn ─────────────────────────────────────────

    #[test]
    fn mysql_dsn_detects_mysql_scheme() {
        assert!(looks_like_mysql_dsn("mysql://user@host/db"));
        assert!(looks_like_mysql_dsn("MYSQL://user@host/db"));
    }

    #[test]
    fn mysql_dsn_detects_mariadb_scheme() {
        assert!(looks_like_mysql_dsn("mariadb://user@host/db"));
        assert!(looks_like_mysql_dsn("MariaDB://user@host/db"));
    }

    #[test]
    fn mysql_dsn_rejects_non_dsn() {
        assert!(!looks_like_mysql_dsn("localhost"));
        assert!(!looks_like_mysql_dsn("postgres://host"));
        assert!(!looks_like_mysql_dsn(""));
    }

    // ── normalize_mysql_host ─────────────────────────────────────────

    #[test]
    fn normalize_mysql_host_defaults_to_localhost() {
        assert_eq!(normalize_mysql_host(""), "localhost");
        assert_eq!(normalize_mysql_host("   "), "localhost");
    }

    #[test]
    fn normalize_mysql_host_trims_input() {
        assert_eq!(normalize_mysql_host("  db.example.com  "), "db.example.com");
    }

    // ── split_mysql_host_and_port ────────────────────────────────────

    #[test]
    fn split_mysql_host_and_port_standard() {
        assert_eq!(
            split_mysql_host_and_port("db.example.com:3307"),
            ("db.example.com".to_string(), Some(3307))
        );
    }

    #[test]
    fn split_mysql_host_and_port_no_port() {
        assert_eq!(
            split_mysql_host_and_port("db.example.com"),
            ("db.example.com".to_string(), None)
        );
    }

    #[test]
    fn split_mysql_host_and_port_ipv6_with_port() {
        assert_eq!(
            split_mysql_host_and_port("[::1]:4406"),
            ("::1".to_string(), Some(4406))
        );
    }

    #[test]
    fn split_mysql_host_and_port_ipv6_without_port() {
        assert_eq!(
            split_mysql_host_and_port("[::1]"),
            ("::1".to_string(), None)
        );
    }

    #[test]
    fn split_mysql_host_and_port_empty() {
        assert_eq!(split_mysql_host_and_port(""), (String::new(), None));
        assert_eq!(split_mysql_host_and_port("  "), (String::new(), None));
    }

    #[test]
    fn split_mysql_host_and_port_multiple_colons() {
        // IPv6 without brackets – too many colons, treated as opaque
        let (host, port) = split_mysql_host_and_port("::1");
        assert_eq!(host, "::1");
        assert_eq!(port, None);
    }

    #[test]
    fn split_mysql_host_and_port_invalid_port() {
        assert_eq!(
            split_mysql_host_and_port("host:notaport"),
            ("host:notaport".to_string(), None)
        );
    }

    // ── ClickHouseTarget::connect_host ───────────────────────────────

    #[test]
    fn clickhouse_connect_host_with_scheme() {
        let target = ClickHouseTarget {
            remote_host: "ch.example.com".to_string(),
            remote_port: 8123,
            scheme: Some("http".to_string()),
        };
        assert_eq!(target.connect_host(9999), "http://127.0.0.1:9999");
    }

    #[test]
    fn clickhouse_connect_host_without_scheme() {
        let target = ClickHouseTarget {
            remote_host: "ch.example.com".to_string(),
            remote_port: 8123,
            scheme: None,
        };
        assert_eq!(target.connect_host(9999), "127.0.0.1");
    }

    // ── parse_clickhouse_target ──────────────────────────────────────

    fn ch_form(host: &str, port: u16) -> ClickHouseFormData {
        ClickHouseFormData {
            host: host.to_string(),
            port,
            username: "default".to_string(),
            password: String::new(),
            database: "default".to_string(),
            ssh_tunnel: None,
        }
    }

    #[test]
    fn parse_clickhouse_target_empty_host_is_error() {
        let result = parse_clickhouse_target(&ch_form("", 8123));
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DatabaseError::Tunnel(msg) => assert!(msg.contains("empty")),
            other => panic!("expected Tunnel error, got {other:?}"),
        }
    }

    #[test]
    fn parse_clickhouse_target_plain_host() {
        let result = parse_clickhouse_target(&ch_form("ch.example.com", 8123));
        let target = result.unwrap();
        assert_eq!(target.remote_host, "ch.example.com");
        assert_eq!(target.remote_port, 8123);
        assert!(target.scheme.is_none());
    }

    #[test]
    fn parse_clickhouse_target_plain_host_default_port() {
        let result = parse_clickhouse_target(&ch_form("ch.example.com", 0));
        let target = result.unwrap();
        assert_eq!(target.remote_port, 8123);
    }

    #[test]
    fn parse_clickhouse_target_http_url() {
        let result = parse_clickhouse_target(&ch_form("http://ch.example.com:9000", 0));
        let target = result.unwrap();
        assert_eq!(target.remote_host, "ch.example.com");
        assert_eq!(target.remote_port, 9000);
        assert_eq!(target.scheme.as_deref(), Some("http"));
    }

    #[test]
    fn parse_clickhouse_target_http_url_default_port() {
        let result = parse_clickhouse_target(&ch_form("http://ch.example.com", 0));
        let target = result.unwrap();
        assert_eq!(target.remote_port, 8123);
    }

    #[test]
    fn parse_clickhouse_target_https_url_is_error() {
        let result = parse_clickhouse_target(&ch_form("https://ch.example.com", 0));
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            DatabaseError::Tunnel(msg) => assert!(msg.contains("HTTPS")),
            other => panic!("expected Tunnel error about HTTPS, got {other:?}"),
        }
    }

    #[test]
    fn parse_clickhouse_target_invalid_url_is_error() {
        let result = parse_clickhouse_target(&ch_form("http://[invalid", 0));
        assert!(result.is_err());
    }

    // ── connect_to_db (integration note) ────────────────────────────

    #[test]
    fn connect_to_db_requires_live_databases() {
        // `connect_to_db` dispatches to driver-specific connect calls that
        // require live database servers. It should be tested via integration
        // tests with actual database instances or Docker containers.
        //
        // The pure helper functions tested above cover all the routing and
        // validation logic that runs before any network call.
    }
}
