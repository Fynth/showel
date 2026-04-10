use models::CodeStralSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const CODESTRAL_API_URL: &str = "https://codestral.mistral.ai/v1/fim/completions";

#[derive(Debug, Clone)]
pub struct CodeStralClient {
    client: Client,
    settings: CodeStralSettings,
}

#[derive(Debug, Serialize)]
struct CodeStralRequest {
    model: String,
    prompt: String,
    suffix: Option<String>,
    max_tokens: usize,
    temperature: f32,
    top_p: f32,
    stop: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CodeStralResponse {
    choices: Vec<CodeStralChoice>,
}

#[derive(Debug, Deserialize)]
struct CodeStralChoice {
    message: Option<CodeStralMessage>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodeStralMessage {
    content: Option<String>,
}

impl CodeStralClient {
    pub fn new(settings: CodeStralSettings) -> Self {
        Self {
            client: Client::new(),
            settings,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.settings.enabled && !self.settings.api_key.is_empty()
    }

    pub async fn get_completion(
        &self,
        prefix: &str,
        suffix: Option<&str>,
    ) -> Result<Option<String>, CodeStralError> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let request = CodeStralRequest {
            model: self.settings.model.clone(),
            prompt: prefix.to_string(),
            suffix: suffix.map(String::from),
            max_tokens: 100,
            temperature: 0.3,
            top_p: 0.95,
            stop: vec!["\n".to_string(), ";".to_string(), " ".to_string()],
        };

        let response = self
            .client
            .post(CODESTRAL_API_URL)
            .header("Authorization", format!("Bearer {}", self.settings.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| CodeStralError::Network(e.to_string()))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| CodeStralError::Network(e.to_string()))?;

        if !status.is_success() {
            return Err(CodeStralError::Api(status.as_u16(), body_text));
        }

        let completion: CodeStralResponse = serde_json::from_str(&body_text).map_err(|e| {
            CodeStralError::Parse(format!(
                "{e}; response_length={}",
                body_text.chars().count()
            ))
        })?;

        Ok(completion.choices.first().and_then(|c| {
            c.text.as_deref().map(|t| t.trim().to_string()).or_else(|| {
                c.message
                    .as_ref()?
                    .content
                    .as_ref()
                    .map(|c| c.trim().to_string())
            })
        }))
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum CodeStralError {
    Network(String),
    Api(u16, String),
    Parse(String),
}
