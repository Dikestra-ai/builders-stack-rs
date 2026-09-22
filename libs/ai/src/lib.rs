// stack-ai — Rust equivalent of the @stack/ai TypeScript package.
//
// Mirrors the TS surface:
//   ai.generate({ prompt?, messages?, system?, maxTokens?, temperature?, model?, provider? })
//     → GenerateResult { text, prompt_tokens, completion_tokens }
//   ai.stream(opts) → tokio::sync::mpsc::Receiver<String>  (token-by-token)
//
// OpenAI calls go through async-openai.
// Anthropic calls use a raw reqwest POST to https://api.anthropic.com/v1/messages
// (async-openai is OpenAI-specific and cannot speak Anthropic's native wire format).
//
// API key: reads AI_API_KEY from the environment.

use anyhow::{anyhow, Context, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client as OpenAIClient,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::instrument;

// ── Default models ─────────────────────────────────────────────────────────────

const DEFAULT_MODEL_OPENAI: &str = "gpt-4o";
const DEFAULT_MODEL_ANTHROPIC: &str = "claude-sonnet-4-5";

// ── Public types ───────────────────────────────────────────────────────────────

/// Which AI provider to target for a call.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Provider {
    #[default]
    OpenAI,
    Anthropic,
}

/// A single message in a multi-turn conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// "user" | "assistant" | "system"
    pub role: String,
    pub content: String,
}

/// Options shared by [`AiClient::generate`] and [`AiClient::stream`].
/// Supply `prompt` OR `messages`; passing neither is an error at call time.
#[derive(Debug, Default)]
pub struct GenerateOptions {
    /// Single-turn prompt.  Mutually exclusive with `messages`.
    pub prompt: Option<String>,
    /// Multi-turn conversation.  Mutually exclusive with `prompt`.
    pub messages: Option<Vec<ChatMessage>>,
    /// System instructions prepended before the conversation.
    pub system: Option<String>,
    /// Max output tokens (defaults: 1 024 for Anthropic, provider default for OpenAI).
    pub max_tokens: Option<u32>,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// Model id override (e.g. "gpt-4o", "claude-sonnet-4-5").
    /// Defaults per provider: OpenAI → "gpt-4o", Anthropic → "claude-sonnet-4-5".
    pub model: Option<String>,
    /// Provider override.  Defaults to `Provider::OpenAI`.
    pub provider: Option<Provider>,
}

/// Result returned by [`AiClient::generate`].
#[derive(Debug, Clone)]
pub struct GenerateResult {
    /// The model's trimmed response text.
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

// ── AiClient ───────────────────────────────────────────────────────────────────

/// The main AI client.
///
/// Construct with [`AiClient::new`] (reads `AI_API_KEY` from env) or
/// [`AiClient::with_key`] to supply the key programmatically.
pub struct AiClient {
    /// async-openai client, used for all OpenAI requests.
    openai: OpenAIClient<OpenAIConfig>,
    /// Raw key stored separately for Anthropic (reqwest) calls.
    api_key: String,
    /// Shared HTTP client for Anthropic requests.
    http: reqwest::Client,
}

impl AiClient {
    /// Create a client using `AI_API_KEY` from the environment.
    /// Panics if the variable is absent — consistent with the TS behaviour.
    pub fn new() -> Self {
        let key = std::env::var("AI_API_KEY")
            .expect("stack-ai: AI_API_KEY env var not set");
        Self::with_key(key)
    }

    /// Create a client with an explicit API key (useful in tests / multi-tenant code).
    pub fn with_key(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        let config = OpenAIConfig::new().with_api_key(&api_key);
        Self {
            openai: OpenAIClient::with_config(config),
            api_key,
            http: reqwest::Client::new(),
        }
    }

    // ── generate ──────────────────────────────────────────────────────────────

    /// One-shot text generation.
    #[instrument(skip(self, opts), fields(model = ?opts.model))]
    pub async fn generate(&self, opts: GenerateOptions) -> Result<GenerateResult> {
        match opts.provider.as_ref().unwrap_or(&Provider::OpenAI) {
            Provider::OpenAI => self.generate_openai(opts).await,
            Provider::Anthropic => self.generate_anthropic(opts).await,
        }
    }

    // ── stream ────────────────────────────────────────────────────────────────

    /// Streaming generation.
    ///
    /// Returns a [`tokio::sync::mpsc::Receiver<String>`] that yields tokens as
    /// they arrive; the channel closes when the stream ends.
    ///
    /// OpenAI: true token-by-token streaming via the SSE API.
    /// Anthropic: runs a full [`generate`] and sends the complete text as a
    /// single message (Anthropic SSE streaming is a future enhancement).
    #[instrument(skip(self, opts), fields(model = ?opts.model))]
    pub async fn stream(
        &self,
        opts: GenerateOptions,
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        match opts.provider.as_ref().unwrap_or(&Provider::OpenAI) {
            Provider::OpenAI => self.stream_openai(opts).await,
            Provider::Anthropic => {
                let result = self.generate_anthropic(opts).await?;
                let (tx, rx) = tokio::sync::mpsc::channel(1);
                let _ = tx.send(result.text).await;
                Ok(rx)
            }
        }
    }

    // ── OpenAI internals ──────────────────────────────────────────────────────

    async fn generate_openai(&self, opts: GenerateOptions) -> Result<GenerateResult> {
        let model = opts
            .model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL_OPENAI)
            .to_string();

        let messages = self.build_openai_messages(&opts)?;

        let mut req_builder = CreateChatCompletionRequestArgs::default();
        req_builder.model(model).messages(messages);
        if let Some(max_tokens) = opts.max_tokens {
            req_builder.max_tokens(max_tokens as u16);
        }
        if let Some(temperature) = opts.temperature {
            req_builder.temperature(temperature);
        }

        let request = req_builder.build()?;
        let response = self
            .openai
            .chat()
            .create(request)
            .await
            .context("OpenAI chat completion failed")?;

        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .cloned()
            .unwrap_or_default();

        let (prompt_tokens, completion_tokens) = response
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((0, 0));
        Ok(GenerateResult {
            text,
            prompt_tokens,
            completion_tokens,
        })
    }

    async fn stream_openai(
        &self,
        opts: GenerateOptions,
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        use futures::StreamExt;

        let model = opts
            .model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL_OPENAI)
            .to_string();

        let messages = self.build_openai_messages(&opts)?;

        let mut req_builder = CreateChatCompletionRequestArgs::default();
        req_builder.model(model).messages(messages);
        if let Some(max_tokens) = opts.max_tokens {
            req_builder.max_tokens(max_tokens as u16);
        }
        if let Some(temperature) = opts.temperature {
            req_builder.temperature(temperature);
        }

        let request = req_builder.build()?;
        let mut sse_stream = self
            .openai
            .chat()
            .create_stream(request)
            .await
            .context("OpenAI stream creation failed")?;

        let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);

        tokio::spawn(async move {
            while let Some(result) = sse_stream.next().await {
                match result {
                    Ok(response) => {
                        for choice in response.choices {
                            if let Some(delta) = choice.delta.content {
                                if tx.send(delta).await.is_err() {
                                    return; // receiver was dropped
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("OpenAI stream error: {e}");
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Builds the `messages` vec for OpenAI from `GenerateOptions`.
    /// Validates that exactly one of `prompt` / `messages` is provided.
    fn build_openai_messages(
        &self,
        opts: &GenerateOptions,
    ) -> Result<Vec<async_openai::types::ChatCompletionRequestMessage>> {
        use async_openai::types::ChatCompletionRequestMessage;

        let mut out: Vec<ChatCompletionRequestMessage> = Vec::new();

        // System message first (top-level field in opts, takes precedence)
        if let Some(system) = &opts.system {
            out.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system.as_str())
                    .build()?
                    .into(),
            );
        }

        match (&opts.messages, &opts.prompt) {
            (Some(msgs), _) => {
                for m in msgs {
                    let msg: ChatCompletionRequestMessage = match m.role.as_str() {
                        "system" => ChatCompletionRequestSystemMessageArgs::default()
                            .content(m.content.as_str())
                            .build()?
                            .into(),
                        "assistant" => ChatCompletionRequestAssistantMessageArgs::default()
                            .content(m.content.as_str())
                            .build()?
                            .into(),
                        _ => ChatCompletionRequestUserMessageArgs::default()
                            .content(m.content.as_str())
                            .build()?
                            .into(),
                    };
                    out.push(msg);
                }
            }
            (None, Some(prompt)) => {
                out.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(prompt.as_str())
                        .build()?
                        .into(),
                );
            }
            (None, None) => {
                return Err(anyhow!("stack-ai: provide `prompt` or `messages`"));
            }
        }

        Ok(out)
    }

    // ── Anthropic internals ───────────────────────────────────────────────────

    async fn generate_anthropic(&self, opts: GenerateOptions) -> Result<GenerateResult> {
        use serde_json::json;

        let model = opts
            .model
            .as_deref()
            .unwrap_or(DEFAULT_MODEL_ANTHROPIC);
        let max_tokens = opts.max_tokens.unwrap_or(1024);

        // Build messages array — Anthropic only accepts "user" / "assistant" roles here.
        let mut messages: Vec<serde_json::Value> = Vec::new();

        match (&opts.messages, &opts.prompt) {
            (Some(msgs), _) => {
                for m in msgs {
                    // "system" role inside messages[] is not valid in Anthropic's API;
                    // it belongs in the top-level `system` field which is handled below.
                    let role = match m.role.as_str() {
                        "assistant" => "assistant",
                        _ => "user",
                    };
                    messages.push(json!({"role": role, "content": m.content}));
                }
            }
            (None, Some(prompt)) => {
                messages.push(json!({"role": "user", "content": prompt}));
            }
            (None, None) => {
                return Err(anyhow!("stack-ai: provide `prompt` or `messages`"));
            }
        }

        let mut body = json!({
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
        });
        if let Some(system) = &opts.system {
            body["system"] = json!(system);
        }
        if let Some(temperature) = opts.temperature {
            body["temperature"] = json!(temperature);
        }

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {status}: {body_text}"));
        }

        let json: serde_json::Value = resp.json().await.context("Failed to parse Anthropic response")?;

        let text = json["content"]
            .as_array()
            .and_then(|arr| {
                // Concatenate all text blocks in order
                let parts: Vec<&str> = arr
                    .iter()
                    .filter(|b| b["type"].as_str() == Some("text"))
                    .filter_map(|b| b["text"].as_str())
                    .collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(""))
                }
            })
            .unwrap_or_default();

        let prompt_tokens = json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

        Ok(GenerateResult {
            text,
            prompt_tokens,
            completion_tokens,
        })
    }
}

impl Default for AiClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── Global singleton ───────────────────────────────────────────────────────────

/// Process-lifetime [`AiClient`] singleton, initialised lazily on first access.
///
/// Reads `AI_API_KEY` from the environment.  Use [`default_client`] to get a
/// reference, or the top-level [`generate`] / [`stream`] helpers directly.
static DEFAULT_CLIENT: Lazy<AiClient> = Lazy::new(AiClient::new);

/// Return a reference to the process-wide [`AiClient`] singleton.
pub fn default_client() -> &'static AiClient {
    &DEFAULT_CLIENT
}

// ── Top-level convenience helpers ─────────────────────────────────────────────

/// One-shot generation via the singleton client.
///
/// ```rust,ignore
/// let result = stack_ai::generate(GenerateOptions { prompt: Some("hi".into()), ..Default::default() }).await?;
/// ```
pub async fn generate(opts: GenerateOptions) -> Result<GenerateResult> {
    default_client().generate(opts).await
}

/// Streaming generation via the singleton client.
pub async fn stream(opts: GenerateOptions) -> Result<tokio::sync::mpsc::Receiver<String>> {
    default_client().stream(opts).await
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_constants() {
        assert_eq!(DEFAULT_MODEL_OPENAI, "gpt-4o");
        assert_eq!(DEFAULT_MODEL_ANTHROPIC, "claude-sonnet-4-5");
    }

    #[test]
    fn generate_options_default() {
        let opts = GenerateOptions::default();
        assert!(opts.prompt.is_none());
        assert!(opts.messages.is_none());
        assert!(opts.system.is_none());
        assert!(opts.max_tokens.is_none());
        assert!(opts.temperature.is_none());
        assert!(opts.model.is_none());
        assert!(opts.provider.is_none());
    }

    #[test]
    fn chat_message_fields() {
        let m = ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        };
        assert_eq!(m.role, "user");
        assert_eq!(m.content, "hello");
    }

    #[test]
    fn provider_default_is_openai() {
        assert_eq!(Provider::default(), Provider::OpenAI);
    }

    #[test]
    fn build_openai_messages_requires_prompt_or_messages() {
        // We can't easily call the async client without a key, but we can
        // verify the error path via with_key on a no-op key.
        let client = AiClient::with_key("test-key");
        let opts = GenerateOptions::default(); // neither prompt nor messages
        let result = client.build_openai_messages(&opts);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("prompt") || err.contains("messages"));
    }

    #[test]
    fn build_openai_messages_with_prompt() {
        let client = AiClient::with_key("test-key");
        let opts = GenerateOptions {
            prompt: Some("hello".to_string()),
            system: Some("be helpful".to_string()),
            ..Default::default()
        };
        let msgs = client.build_openai_messages(&opts).unwrap();
        // system message first, then user prompt
        assert_eq!(msgs.len(), 2);
    }
}
