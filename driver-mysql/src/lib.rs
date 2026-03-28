use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
use std::str::FromStr;

#[derive(Debug)]
pub struct MySqlConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

pub struct MySqlDriver;

impl database::DatabaseDriver for MySqlDriver {
    type Config = MySqlConfig;
    type Pool = sqlx::MySqlPool;
    type Error = sqlx::Error;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        if looks_like_dsn(&info.host) {
            let options = MySqlConnectOptions::from_str(info.host.trim())?;
            return Self::Pool::connect_with(options).await;
        }

        let host = normalized_host(&info.host);
        let username = normalized_username(&info.username);
        let database = normalized_database(&info.database);
        let port = if info.port == 0 { 3306 } else { info.port };

        let mut attempts = vec![host.clone()];
        if host == "localhost" {
            attempts.push("127.0.0.1".to_string());
        }

        let mut last_error = None;
        for attempt_host in attempts {
            let options = MySqlConnectOptions::new()
                .host(&attempt_host)
                .port(port)
                .username(&username)
                .password(&info.password)
                .database(&database)
                .ssl_mode(MySqlSslMode::Preferred);

            match Self::Pool::connect_with(options).await {
                Ok(pool) => return Ok(pool),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            sqlx::Error::Protocol("mysql connection attempts produced no result".into())
        }))
    }
}

fn looks_like_dsn(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("mysql://") || value.starts_with("mariadb://")
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
        "root".to_string()
    } else {
        username.to_string()
    }
}

fn normalized_database(database: &str) -> String {
    let database = database.trim();
    if database.is_empty() {
        "mysql".to_string()
    } else {
        database.to_string()
    }
}
