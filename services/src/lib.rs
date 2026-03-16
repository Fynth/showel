use database::DatabaseDriver;
use drivers::{
    postgres::{PgConfig, PgDriver},
    sqlite::SqliteDriver,
};
use models::*;

pub async fn connect_to_db(
    request: models::ConnectionRequest,
) -> Result<models::DatabaseConnection, models::DatabaseError> {
    match request {
        ConnectionRequest::Sqlite(data) => {
            let pool = SqliteDriver::connect(data.path)
                .await
                .map_err(models::DatabaseError::Sqlite)?;
            Ok(models::DatabaseConnection::Sqlite(pool))
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
                .map_err(models::DatabaseError::Postgres)?;
            Ok(models::DatabaseConnection::Postgres(pool))
        }
        ConnectionRequest::ClickHouse(data) => {
            todo!()
        }
    }
}
