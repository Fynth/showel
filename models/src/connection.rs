use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{error::Error, fmt};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
    ClickHouse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SshTunnelConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub private_key_path: String,
}

impl SshTunnelConfig {
    pub fn is_configured(&self) -> bool {
        !self.host.trim().is_empty() && !self.username.trim().is_empty()
    }

    pub fn effective_port(&self) -> u16 {
        if self.port == 0 { 22 } else { self.port }
    }

    pub fn display_name(&self) -> String {
        format!(
            "{}@{}:{}",
            self.username.trim(),
            self.host.trim(),
            self.effective_port()
        )
    }
}

#[derive(Clone, Debug)]
pub enum DatabaseConnection {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
    ClickHouse(ClickHouseFormData),
}

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(sqlx::Error),
    Postgres(sqlx::Error),
    ClickHouse(String),
    Tunnel(String),
    UnsupportedDriver(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteFormData {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresFormData {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClickHouseFormData {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
}

impl ClickHouseFormData {
    pub fn effective_username(&self) -> &str {
        if self.username.trim().is_empty() {
            "default"
        } else {
            self.username.trim()
        }
    }

    pub fn effective_database(&self) -> &str {
        if self.database.trim().is_empty() {
            "default"
        } else {
            self.database.trim()
        }
    }
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "SQLite error: {err}"),
            Self::Postgres(err) => write!(f, "PostgreSQL error: {err}"),
            Self::ClickHouse(err) => write!(f, "ClickHouse error: {err}"),
            Self::Tunnel(err) => write!(f, "SSH tunnel error: {err}"),
            Self::UnsupportedDriver(err) => write!(f, "{err}"),
        }
    }
}

impl Error for DatabaseError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionRequest {
    Sqlite(SqliteFormData),
    Postgres(PostgresFormData),
    ClickHouse(ClickHouseFormData),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub request: ConnectionRequest,
}

impl ConnectionRequest {
    pub fn kind(&self) -> DatabaseKind {
        match self {
            ConnectionRequest::Sqlite(_) => DatabaseKind::Sqlite,
            ConnectionRequest::Postgres(_) => DatabaseKind::Postgres,
            ConnectionRequest::ClickHouse(_) => DatabaseKind::ClickHouse,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            ConnectionRequest::Sqlite(data) => format!("SQLite · {}", data.path.trim()),
            ConnectionRequest::Postgres(data) => {
                let endpoint = normalized_postgres_endpoint(data);
                let mut label = format!(
                    "PostgreSQL · {}@{}:{}/{}",
                    endpoint.username, endpoint.host, endpoint.port, endpoint.database
                );
                if let Some(tunnel) = data.ssh_tunnel.as_ref().filter(|cfg| cfg.is_configured()) {
                    label.push_str(&format!(" via SSH {}", tunnel.display_name()));
                }
                label
            }
            ConnectionRequest::ClickHouse(data) => {
                let mut label = format!(
                    "ClickHouse · {}@{}/{}",
                    data.effective_username(),
                    clickhouse_endpoint_label(data),
                    data.effective_database()
                );
                if let Some(tunnel) = data.ssh_tunnel.as_ref().filter(|cfg| cfg.is_configured()) {
                    label.push_str(&format!(" via SSH {}", tunnel.display_name()));
                }
                label
            }
        }
    }

    pub fn short_name(&self) -> String {
        match self {
            ConnectionRequest::Sqlite(data) => Path::new(data.path.trim())
                .file_stem()
                .or_else(|| Path::new(data.path.trim()).file_name())
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| data.path.trim().to_string()),
            ConnectionRequest::Postgres(data) => normalized_postgres_endpoint(data).database,
            ConnectionRequest::ClickHouse(data) => data.effective_database().to_string(),
        }
    }

    pub fn identity_key(&self) -> String {
        match self {
            ConnectionRequest::Sqlite(data) => format!("sqlite:{}", data.path.trim()),
            ConnectionRequest::Postgres(data) => {
                let endpoint = normalized_postgres_endpoint(data);
                format!(
                    "postgres:{}@{}:{}/{}{}",
                    endpoint.username,
                    endpoint.host.to_ascii_lowercase(),
                    endpoint.port,
                    endpoint.database,
                    ssh_identity_suffix(data.ssh_tunnel.as_ref())
                )
            }
            ConnectionRequest::ClickHouse(data) => format!(
                "clickhouse:{}|user:{}|database:{}{}",
                normalized_clickhouse_endpoint_key(data),
                data.effective_username(),
                data.effective_database(),
                ssh_identity_suffix(data.ssh_tunnel.as_ref())
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NormalizedPostgresEndpoint {
    host: String,
    port: u16,
    username: String,
    database: String,
}

fn normalized_postgres_endpoint(data: &PostgresFormData) -> NormalizedPostgresEndpoint {
    if let Some(endpoint) = parsed_postgres_dsn(data.host.trim(), &data.username, &data.database) {
        return endpoint;
    }

    let host = normalized_postgres_host(&data.host);
    let port = if data.port == 0 { 5432 } else { data.port };
    let username = normalized_postgres_username(&data.username);
    let database = normalized_postgres_database(&data.database, &username);
    NormalizedPostgresEndpoint {
        host,
        port,
        username,
        database,
    }
}

fn looks_like_postgres_dsn(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("postgres://") || value.starts_with("postgresql://")
}

fn normalized_postgres_host(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        "localhost".to_string()
    } else {
        host.to_string()
    }
}

fn normalized_postgres_username(username: &str) -> String {
    let username = username.trim();
    if username.is_empty() {
        "postgres".to_string()
    } else {
        username.to_string()
    }
}

fn normalized_postgres_database(database: &str, username: &str) -> String {
    let database = database.trim();
    if database.is_empty() {
        username.to_string()
    } else {
        database.to_string()
    }
}

fn parsed_postgres_dsn(
    value: &str,
    fallback_username: &str,
    fallback_database: &str,
) -> Option<NormalizedPostgresEndpoint> {
    if !looks_like_postgres_dsn(value) {
        return None;
    }

    let url = Url::parse(value).ok()?;
    let host = url.host_str()?.to_string();
    let port = url.port().unwrap_or(5432);
    let username = if url.username().is_empty() {
        normalized_postgres_username(fallback_username)
    } else {
        url.username().to_string()
    };
    let database = url
        .path_segments()
        .and_then(|mut segments| segments.find(|segment| !segment.is_empty()))
        .map(str::to_string)
        .unwrap_or_else(|| normalized_postgres_database(fallback_database, &username));

    Some(NormalizedPostgresEndpoint {
        host,
        port,
        username,
        database,
    })
}

fn normalized_clickhouse_endpoint_key(data: &ClickHouseFormData) -> String {
    if let Some(url) = parsed_clickhouse_url(&data.host) {
        let host = url
            .host_str()
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| data.host.trim().to_ascii_lowercase());
        let port = url
            .port_or_known_default()
            .unwrap_or(normalized_clickhouse_port(data));
        let path = normalized_url_path(&url);
        return format!(
            "{}://{}:{}{}",
            url.scheme().to_ascii_lowercase(),
            host,
            port,
            path
        );
    }

    format!(
        "{}:{}",
        data.host.trim().to_ascii_lowercase(),
        normalized_clickhouse_port(data)
    )
}

fn clickhouse_endpoint_label(data: &ClickHouseFormData) -> String {
    if let Some(url) = parsed_clickhouse_url(&data.host) {
        let host = url.host_str().unwrap_or(data.host.trim());
        let port = url
            .port_or_known_default()
            .unwrap_or(normalized_clickhouse_port(data));
        let path = normalized_url_path(&url);
        return format!("{}://{}:{}{}", url.scheme(), host, port, path);
    }

    format!("{}:{}", data.host.trim(), normalized_clickhouse_port(data))
}

fn normalized_clickhouse_port(data: &ClickHouseFormData) -> u16 {
    if data.port == 0 { 8123 } else { data.port }
}

fn parsed_clickhouse_url(value: &str) -> Option<Url> {
    let value = value.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        Url::parse(value).ok()
    } else {
        None
    }
}

fn normalized_url_path(url: &Url) -> String {
    let path = url.path().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        String::new()
    } else {
        path.to_string()
    }
}

fn ssh_identity_suffix(config: Option<&SshTunnelConfig>) -> String {
    let Some(config) = config.filter(|config| config.is_configured()) else {
        return String::new();
    };

    let key_path = config.private_key_path.trim();
    if key_path.is_empty() {
        format!(
            "|ssh:{}@{}:{}",
            config.username.trim(),
            config.host.trim().to_ascii_lowercase(),
            config.effective_port()
        )
    } else {
        format!(
            "|ssh:{}@{}:{}|key:{}",
            config.username.trim(),
            config.host.trim().to_ascii_lowercase(),
            config.effective_port(),
            key_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClickHouseFormData, ConnectionRequest, PostgresFormData, SqliteFormData, SshTunnelConfig,
    };

    #[test]
    fn postgres_dsn_display_name_redacts_password() {
        let request = ConnectionRequest::Postgres(PostgresFormData {
            host: "postgres://alice:super-secret@db.example.com:5433/app?sslmode=require"
                .to_string(),
            port: 5432,
            username: String::new(),
            password: "ignored".to_string(),
            database: String::new(),
            ssh_tunnel: None,
        });

        assert_eq!(
            request.display_name(),
            "PostgreSQL · alice@db.example.com:5433/app"
        );
        assert_eq!(request.short_name(), "app");
        assert!(!request.display_name().contains("super-secret"));
        assert!(!request.identity_key().contains("super-secret"));
    }

    #[test]
    fn postgres_short_name_matches_runtime_default_database() {
        let request = ConnectionRequest::Postgres(PostgresFormData {
            host: "localhost".to_string(),
            port: 5432,
            username: "analytics".to_string(),
            password: String::new(),
            database: String::new(),
            ssh_tunnel: None,
        });

        assert_eq!(request.short_name(), "analytics");
        assert_eq!(
            request.identity_key(),
            "postgres:analytics@localhost:5432/analytics"
        );
    }

    #[test]
    fn clickhouse_url_identity_key_redacts_credentials_and_keeps_path() {
        let request = ConnectionRequest::ClickHouse(ClickHouseFormData {
            host: "https://svc-user:super-secret@click.example.com:8443/proxy".to_string(),
            port: 8123,
            username: "default".to_string(),
            password: "top-secret".to_string(),
            database: "warehouse".to_string(),
            ssh_tunnel: Some(SshTunnelConfig {
                host: "bastion.example.com".to_string(),
                port: 22,
                username: "ops".to_string(),
                private_key_path: "/keys/prod".to_string(),
            }),
        });

        assert_eq!(
            request.display_name(),
            "ClickHouse · default@https://click.example.com:8443/proxy/warehouse via SSH ops@bastion.example.com:22"
        );
        assert!(
            request
                .identity_key()
                .contains("https://click.example.com:8443/proxy")
        );
        assert!(!request.identity_key().contains("super-secret"));
    }

    #[test]
    fn sqlite_identity_key_uses_trimmed_path() {
        let request = ConnectionRequest::Sqlite(SqliteFormData {
            path: " /tmp/app.db ".to_string(),
        });

        assert_eq!(request.display_name(), "SQLite · /tmp/app.db");
        assert_eq!(request.identity_key(), "sqlite:/tmp/app.db");
    }
}
