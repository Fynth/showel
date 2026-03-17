pub struct SqliteDriver {}
type SqliteError = sqlx::Error;
type SqlitePool = sqlx::SqlitePool;
type SqliteConfig = String;
impl database::DatabaseDriver for SqliteDriver {
    type Config = SqliteConfig;
    type Pool = SqlitePool;
    type Error = SqliteError;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        SqlitePool::connect(&format!("sqlite://{}", info)).await
    }
}
