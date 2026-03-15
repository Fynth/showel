pub struct SqliteDriver {}
type SqliteError = sqlx::Error;
type SqliteConnection = sqlx::SqlitePool;
type SqliteConfig = String;
impl database::DatabaseDriver for SqliteDriver {
    type Config = SqliteConfig;
    type Pool = SqliteConnection;
    type Error = SqliteError;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        SqliteConnection::connect(&format!("sqlite://{}", info)).await
    }
}
