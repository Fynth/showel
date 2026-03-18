use sqlx::sqlite::SqliteConnectOptions;
use std::{path::PathBuf, str::FromStr};

pub struct SqliteDriver {}
type SqliteError = sqlx::Error;
type SqlitePool = sqlx::SqlitePool;
type SqliteConfig = String;
impl database::DatabaseDriver for SqliteDriver {
    type Config = SqliteConfig;
    type Pool = SqlitePool;
    type Error = SqliteError;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        let target = info.trim();
        let options = if target.eq_ignore_ascii_case(":memory:") || target.starts_with("sqlite:") {
            SqliteConnectOptions::from_str(target)?
        } else {
            SqliteConnectOptions::new()
                .filename(PathBuf::from(target))
                .create_if_missing(false)
        };

        SqlitePool::connect_with(options).await
    }
}
