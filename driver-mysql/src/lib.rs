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

        let (host, embedded_port) = split_host_and_port(&info.host);
        let host = normalized_host(&host);
        let username = normalized_username(&info.username);
        let database = normalized_database(&info.database);
        let port = embedded_port.unwrap_or(if info.port == 0 { 3306 } else { info.port });

        let mut attempts = vec![host.clone()];
        if host == "localhost" {
            attempts.push("127.0.0.1".to_string());
        }

        let mut last_error = None;
        for attempt_host in attempts {
            let mut options = MySqlConnectOptions::new()
                .host(&attempt_host)
                .port(port)
                .username(&username)
                .password(&info.password)
                .ssl_mode(MySqlSslMode::Preferred);
            if !database.is_empty() {
                options = options.database(&database);
            }

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
    database.trim().to_string()
}

fn split_host_and_port(value: &str) -> (String, Option<u16>) {
    let value = value.trim();
    if value.is_empty() {
        return (String::new(), None);
    }

    if value.starts_with('[')
        && let Some(end_bracket) = value.find(']')
    {
        let host = value[1..end_bracket].to_string();
        let remainder = value[end_bracket + 1..].trim();
        if remainder.is_empty() {
            return (host, None);
        }
        if let Some(port) = remainder
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
        {
            return (host, Some(port));
        }
        return (value.to_string(), None);
    }

    if value.matches(':').count() == 1
        && let Some((host, port)) = value.rsplit_once(':')
        && !host.trim().is_empty()
        && let Ok(port) = port.trim().parse::<u16>()
    {
        return (host.trim().to_string(), Some(port));
    }

    (value.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::split_host_and_port;

    #[test]
    fn splits_mysql_host_with_embedded_port() {
        assert_eq!(
            split_host_and_port("db.example.com:3307"),
            ("db.example.com".to_string(), Some(3307))
        );
        assert_eq!(
            split_host_and_port("[::1]:4406"),
            ("::1".to_string(), Some(4406))
        );
        assert_eq!(
            split_host_and_port("db.example.com"),
            ("db.example.com".to_string(), None)
        );
    }
}
