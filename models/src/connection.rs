use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
    ClickHouse,
}

#[derive(Clone, Debug)]
pub enum DatabaseConnection {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
}

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(sqlx::Error),
    Postgres(sqlx::Error),
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClickHouseFormData {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

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
            ConnectionRequest::Sqlite(data) => format!("SQLite · {}", data.path),
            ConnectionRequest::Postgres(data) => format!(
                "PostgreSQL · {}@{}:{}/{}",
                data.username, data.host, data.port, data.database
            ),
            ConnectionRequest::ClickHouse(data) => format!(
                "ClickHouse · {}@{}:{}/{}",
                data.username, data.host, data.port, data.database
            ),
        }
    }
}
