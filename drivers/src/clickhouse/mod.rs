use models::ClickHouseFormData;
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct ClickHouseJsonMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClickHouseJsonResponse {
    #[serde(default)]
    pub meta: Vec<ClickHouseJsonMeta>,
    #[serde(default)]
    pub data: Vec<Vec<serde_json::Value>>,
}

pub struct ClickHouseDriver;

impl database::DatabaseDriver for ClickHouseDriver {
    type Config = ClickHouseFormData;
    type Pool = ClickHouseFormData;
    type Error = String;

    async fn connect(info: Self::Config) -> Result<Self::Pool, Self::Error> {
        execute_text_query(&info, "SELECT 1").await?;
        Ok(info)
    }
}

pub async fn execute_json_query(
    info: &ClickHouseFormData,
    sql: &str,
) -> Result<ClickHouseJsonResponse, String> {
    let sql = ensure_json_compact(sql);
    let response = build_request(info, &sql)?
        .send()
        .await
        .map_err(|err| format!("ClickHouse request failed: {err}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Failed to read ClickHouse response: {err}"))?;

    if !status.is_success() {
        return Err(format!("ClickHouse returned {status}: {body}"));
    }

    serde_json::from_str(&body).map_err(|err| format!("Failed to parse ClickHouse JSON: {err}"))
}

pub async fn execute_text_query(info: &ClickHouseFormData, sql: &str) -> Result<String, String> {
    let response = build_request(info, sql)?
        .send()
        .await
        .map_err(|err| format!("ClickHouse request failed: {err}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Failed to read ClickHouse response: {err}"))?;

    if !status.is_success() {
        return Err(format!("ClickHouse returned {status}: {body}"));
    }

    Ok(body)
}

fn build_request(info: &ClickHouseFormData, sql: &str) -> Result<reqwest::RequestBuilder, String> {
    let base_url = normalize_base_url(info)?;
    let client = http_client();
    let mut request = client.post(format!("{base_url}/")).body(sql.to_string());

    request = request.basic_auth(info.effective_username(), Some(&info.password));
    request = request.query(&[("database", info.effective_database())]);

    Ok(request)
}

fn ensure_json_compact(sql: &str) -> String {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.to_ascii_lowercase().contains(" format ") {
        trimmed.to_string()
    } else {
        format!("{trimmed} FORMAT JSONCompact")
    }
}

fn normalize_base_url(info: &ClickHouseFormData) -> Result<String, String> {
    let host = info.host.trim();
    if host.is_empty() {
        return Err("ClickHouse host is empty".to_string());
    }

    let normalized = if host.starts_with("http://") || host.starts_with("https://") {
        host.trim_end_matches('/').to_string()
    } else {
        format!("http://{}:{}", host, info.port)
    };

    Ok(normalized)
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build ClickHouse HTTP client")
    })
}
