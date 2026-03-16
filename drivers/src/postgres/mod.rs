#[derive(Debug)]
pub struct PgConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

pub struct PgDriver {}
impl database::DatabaseDriver for PgDriver {
    type Config = PgConfig;
    type Pool = sqlx::PgPool;
    type Error = sqlx::Error;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            info.username, info.password, info.host, info.port, info.database
        );

        Self::Pool::connect(&url).await
    }
}
