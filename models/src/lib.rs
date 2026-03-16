pub enum DatabaseConfig {
    Sqlite(String),
    Postgres {
        host: String,
        port: u16,
        username: String,
        password: String,
        database: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
    ClickHouse,
}

pub enum DatabaseConnection {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
}

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(sqlx::Error),
    Postgres(sqlx::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionFormData {
    pub db_type: String,
    pub sqlite_path: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug)]
pub struct SqliteFormData {
    pub path: String,
}

#[derive(Debug)]
pub struct PostgresFormData {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug)]
pub struct ClickHouseFormData {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug)]
pub enum ConnectionRequest {
    Sqlite(SqliteFormData),
    Postgres(PostgresFormData),
    ClickHouse(ClickHouseFormData),
}
