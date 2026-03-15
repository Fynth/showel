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
pub enum DatabaseConnection {
    Sqlite(sqlx::SqlitePool),
}

#[derive(Debug)]
pub enum DatabaseError {
    Sqlite(sqlx::Error),
}
impl From<sqlx::Error> for DatabaseError {
    fn from(value: sqlx::Error) -> Self {
        DatabaseError::Sqlite(value)
    }
}
