//! AI-powered SQL completion with multi-provider fallback.
//!
//! Provides a unified completion interface that tries providers in order:
//! 1. CodeStral (Mistral FIM API) — fastest, purpose-built for code completion
//! 2. DeepSeek (chat API) — broader context understanding
//!
//! Each provider receives the same prompt: schema context + prefix + suffix
//! with explicit SQL completion instructions.

use models::{AppUiSettings, CodeStralSettings, DeepSeekSettings};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

// ── Shared HTTP client ──────────────────────────────────────────────

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn http_client() -> Client {
    HTTP_CLIENT.get_or_init(Client::new).clone()
}

// ── Completion result ───────────────────────────────────────────────

/// The result of a completion request — either a completion string or nothing.
pub type CompletionResult = Result<Option<String>, String>;

// ── Completion service ──────────────────────────────────────────────

enum CompletionProvider {
    CodeStral(CodeStralProvider),
    DeepSeek(DeepSeekProvider),
}

/// Tries completion providers in order, returning the first successful result.
pub struct CompletionService {
    providers: Vec<CompletionProvider>,
}

impl CompletionService {
    pub fn new(settings: &AppUiSettings) -> Self {
        let mut providers: Vec<CompletionProvider> = Vec::new();

        eprintln!(
            "[completion] init: codestral enabled={}, has_key={}, deepseek enabled={}, has_key={}",
            settings.codestral.enabled,
            !settings.codestral.api_key.is_empty(),
            settings.deepseek.enabled,
            !settings.deepseek.api_key.is_empty(),
        );

        if settings.codestral.enabled && !settings.codestral.api_key.is_empty() {
            providers.push(CompletionProvider::CodeStral(CodeStralProvider::new(
                settings.codestral.clone(),
            )));
            eprintln!("[completion] added CodeStral provider");
        }

        if settings.deepseek.enabled && !settings.deepseek.api_key.is_empty() {
            providers.push(CompletionProvider::DeepSeek(DeepSeekProvider::new(
                settings.deepseek.clone(),
            )));
            eprintln!("[completion] added DeepSeek provider");
        }

        if providers.is_empty() {
            eprintln!("[completion] no providers configured");
        }

        Self { providers }
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Request a completion, trying each provider in order.
    pub async fn get_completion(
        &self,
        prefix: &str,
        suffix: Option<&str>,
        schema_context: &str,
    ) -> CompletionResult {
        if self.providers.is_empty() {
            return Ok(None);
        }

        for provider in &self.providers {
            let provider_name = match provider {
                CompletionProvider::CodeStral(_) => "CodeStral",
                CompletionProvider::DeepSeek(_) => "DeepSeek",
            };
            eprintln!("[completion] trying provider: {provider_name}");

            let result = match provider {
                CompletionProvider::CodeStral(p) => {
                    p.complete(prefix, suffix, schema_context).await
                }
                CompletionProvider::DeepSeek(p) => p.complete(prefix, suffix, schema_context).await,
            };

            match result {
                Ok(Some(completion)) => return Ok(Some(completion)),
                Ok(None) => {
                    eprintln!("[completion] provider returned empty");
                    continue;
                }
                Err(e) => {
                    eprintln!("[completion] provider error: {e}");
                    continue;
                }
            }
        }

        Ok(None)
    }
}

// ── Prompt builder ──────────────────────────────────────────────────

fn build_completion_prompt(prefix: &str, schema_context: &str) -> String {
    if schema_context.is_empty() {
        return prefix.to_string();
    }
    format!("-- Database schema:\n{schema_context}\n\n{prefix}")
}

// ── CodeStral provider ──────────────────────────────────────────────

const CODESTRAL_API_URL: &str = "https://codestral.mistral.ai/v1/fim/completions";

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
    text: Option<String>,
    message: Option<CodeStralMessage>,
}

#[derive(Debug, Deserialize)]
struct CodeStralMessage {
    content: Option<String>,
}

struct CodeStralProvider {
    client: Client,
    settings: CodeStralSettings,
}

impl CodeStralProvider {
    fn new(settings: CodeStralSettings) -> Self {
        Self {
            client: http_client(),
            settings,
        }
    }

    async fn complete(
        &self,
        prefix: &str,
        suffix: Option<&str>,
        schema_context: &str,
    ) -> CompletionResult {
        let prompt = build_completion_prompt(prefix, schema_context);

        let request = CodeStralRequest {
            model: self.settings.model.clone(),
            prompt,
            suffix: suffix.map(String::from),
            max_tokens: 80,
            temperature: 0.2,
            top_p: 0.95,
            stop: vec!["\n\n".to_string(), ";".to_string()],
        };

        let response = self
            .client
            .post(CODESTRAL_API_URL)
            .header("Authorization", format!("Bearer {}", self.settings.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("CodeStral network error: {e}"))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| format!("CodeStral response error: {e}"))?;

        if !status.is_success() {
            return Err(format!("CodeStral API {}: {}", status.as_u16(), body_text));
        }

        let completion: CodeStralResponse =
            serde_json::from_str(&body_text).map_err(|e| format!("CodeStral parse error: {e}"))?;

        Ok(completion.choices.first().and_then(|c| {
            c.text.as_deref().map(normalize_text).or_else(|| {
                c.message
                    .as_ref()?
                    .content
                    .as_ref()
                    .map(|c| normalize_text(c))
            })
        }))
    }
}

// ── DeepSeek provider ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct DeepSeekRequest {
    model: String,
    messages: Vec<DeepSeekMessage>,
    max_tokens: usize,
    temperature: f32,
    stop: Vec<String>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct DeepSeekMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoice {
    message: DeepSeekResponseMessage,
}

#[derive(Debug, Deserialize)]
struct DeepSeekResponseMessage {
    content: String,
}

struct DeepSeekProvider {
    client: Client,
    settings: DeepSeekSettings,
}

impl DeepSeekProvider {
    fn new(settings: DeepSeekSettings) -> Self {
        Self {
            client: http_client(),
            settings,
        }
    }

    async fn complete(
        &self,
        prefix: &str,
        suffix: Option<&str>,
        schema_context: &str,
    ) -> CompletionResult {
        eprintln!(
            "[completion] DeepSeek request: prefix_len={}, model={}",
            prefix.len(),
            self.settings.model,
        );

        let schema_part = if schema_context.is_empty() {
            String::new()
        } else {
            format!("Database schema:\n{schema_context}\n\n")
        };

        let suffix_part = suffix
            .map(|s| format!("\n\nThe SQL after the cursor is:\n```sql\n{s}\n```"))
            .unwrap_or_default();

        let system_prompt = format!(
            "You are a SQL completion engine. Complete the SQL statement at the cursor position.\n\
             Return ONLY the completion text — no explanations, no markdown, no backticks.\n\
             Match the existing style (uppercase keywords, indentation).\n\
             Do NOT repeat text that already appears before or after the cursor.\n\
             If the statement is already complete, return an empty response.\n\
             {schema_part}",
        );

        let user_prompt = format!(
            "Complete the SQL at the [CURSOR]:\n\
             ```sql\n{prefix}[CURSOR]{}\n```{suffix_part}",
            suffix.unwrap_or("")
        );

        let request = DeepSeekRequest {
            model: self.settings.model.clone(),
            messages: vec![
                DeepSeekMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                DeepSeekMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            max_tokens: 80,
            temperature: 0.1,
            stop: vec!["\n\n".to_string(), ";".to_string(), "```".to_string()],
            stream: false,
        };

        let api_url = format!("{}/chat/completions", self.settings.base_url);

        let response = self
            .client
            .post(&api_url)
            .header("Authorization", format!("Bearer {}", self.settings.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("DeepSeek network error: {e}"))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| format!("DeepSeek response error: {e}"))?;

        if !status.is_success() {
            eprintln!(
                "[completion] DeepSeek API error: status={}, body={}",
                status.as_u16(),
                body_text
            );
            return Err(format!("DeepSeek API {}: {}", status.as_u16(), body_text));
        }

        let completion: DeepSeekResponse =
            serde_json::from_str(&body_text).map_err(|e| format!("DeepSeek parse error: {e}"))?;

        let text = completion
            .choices
            .first()
            .map(|c| normalize_text(&c.message.content))
            .filter(|t| !t.is_empty());

        eprintln!(
            "[completion] DeepSeek response: text_len={}",
            text.as_ref().map_or(0, |t| t.len())
        );
        Ok(text)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn normalize_text(text: &str) -> String {
    text.trim_matches(|ch| matches!(ch, '\r' | '\n'))
        .trim()
        .to_string()
}
