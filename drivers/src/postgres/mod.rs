use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::str::FromStr;

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
        if looks_like_dsn(&info.host) {
            let options = PgConnectOptions::from_str(info.host.trim())?;
            return Self::Pool::connect_with(options).await;
        }

        let host = normalized_host(&info.host);
        let username = normalized_username(&info.username);
        let database = normalized_database(&info.database, &username);
        let port = if info.port == 0 { 5432 } else { info.port };

        let mut attempts = vec![host.clone()];
        if host == "localhost" {
            attempts.push("127.0.0.1".to_string());
        }

        let mut last_error = None;
        for attempt_host in attempts {
            let options = PgConnectOptions::new_without_pgpass()
                .host(&attempt_host)
                .port(port)
                .username(&username)
                .password(&info.password)
                .database(&database)
                .ssl_mode(PgSslMode::Prefer);

            match Self::Pool::connect_with(options).await {
                Ok(pool) => return Ok(pool),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error.expect("postgres connection attempts should produce an error"))
    }
}

fn looks_like_dsn(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("postgres://") || value.starts_with("postgresql://")
}

fn normalized_host(host: &str) -> String {
    let host = host.trim();
    if host.is_empty() {
        "localhost".to_string()
    } else {
        host.to_string()
    }
}

fn normalized_username(username: &str) -> String {
    let username = username.trim();
    if username.is_empty() {
        "postgres".to_string()
    } else {
        username.to_string()
    }
}

fn normalized_database(database: &str, username: &str) -> String {
    let database = database.trim();
    if database.is_empty() {
        username.to_string()
    } else {
        database.to_string()
    }
}
