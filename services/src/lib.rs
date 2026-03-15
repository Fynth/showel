use database::DatabaseDriver;
use drivers::sqlite::SqliteDriver;

pub async fn connect_to_db(
    config: models::DatabaseConfig,
) -> Result<models::DatabaseConnection, models::DatabaseError> {
    match config {
        models::DatabaseConfig::Sqlite(path) => {
            let pool = SqliteDriver::connect(path).await?;
            Ok(models::DatabaseConnection::Sqlite(pool))
        }
        models::DatabaseConfig::Postgres {
            host,
            port,
            username,
            password,
            database,
        } => {
            // postgres connect...
            todo!()
        }
    }
}
