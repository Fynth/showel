use database::DatabaseDriver;
use drivers::{
    postgres::{PgConfig, PgDriver},
    sqlite::SqliteDriver,
};
use models::{ConnectionRequest, DatabaseConnection, DatabaseError};

pub async fn connect_to_db(
    request: ConnectionRequest,
) -> Result<DatabaseConnection, DatabaseError> {
    match request {
        ConnectionRequest::Sqlite(data) => {
            let pool = SqliteDriver::connect(data.path)
                .await
                .map_err(DatabaseError::Sqlite)?;
            Ok(DatabaseConnection::Sqlite(pool))
        }
        ConnectionRequest::Postgres(data) => {
            let config = PgConfig {
                host: data.host,
                port: data.port,
                username: data.username,
                password: data.password,
                database: data.database,
            };
            let pool = PgDriver::connect(config)
                .await
                .map_err(DatabaseError::Postgres)?;
            Ok(DatabaseConnection::Postgres(pool))
        }
        ConnectionRequest::ClickHouse(_) => Err(DatabaseError::UnsupportedDriver(
            "ClickHouse driver is not implemented yet".to_string(),
        )),
    }
}
