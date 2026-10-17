//! Vision analysis — screenshot → multimodal LLM review.
//!
//! Ported from `tairitsu-mcp`'s vision tools during the 2026-10-17
//! browser-backbone consolidation: every browser-related facility lives
//! here, so the "analyze a screenshot with a vision model" flow moved with
//! it. Only OpenAI-compatible chat-completions endpoints are supported.
//!
//! # Registry resolution
//!
//! Models come from a `provider-registry` checkout (model TOMLs under
//! `models/<provider>/<model>.toml`, endpoint configs under
//! `entrypoint/<provider>/default.toml`). Resolution order:
//!
//! 1. `SHIRABE_VISION_REGISTRY` — explicit directory.
//! 2. `./provider-registry` — the current working directory.
//! 3. `../provider-registry` — the parent directory (the celestia
//!    workspace layout during development: siblings of this repo).
//!
//! An explicit model can be pinned with `SHIRABE_VISION_MODEL`; otherwise
//! the cheapest eligible vision model with an available API key wins. API
//! keys are resolved from the provider's own env var or the explicit
//! `SHIRABE_VISION_API_KEY` override — no generic cross-provider fallbacks.

use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

// ── Provider-registry data model ─────────────────────────────────────────

/// A vision-capable model entry loaded from provider-registry.
#[derive(Debug, Clone, Deserialize)]
pub struct VisionModel {
    #[serde(rename = "model")]
    pub inner: ModelInner,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelInner {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub context_window: u64,
    /// Chat models > 0; image/video generation models carry 0.
    #[serde(default)]
    pub max_output_tokens: u64,
    #[serde(default)]
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_per_million: f64,
    #[serde(default)]
    pub output_per_million: f64,
}

/// A provider's API entrypoint config.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEntry {
    #[serde(rename = "entrypoint")]
    pub inner: EntrypointInner,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntrypointInner {
    pub provider_id: String,
    #[serde(rename = "api")]
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub protocol: String,
    pub base_url: String,
    #[serde(default)]
    pub chat_endpoint: String,
    #[serde(default = "default_bearer")]
    pub auth_type: String,
    #[serde(default = "default_auth_header")]
    pub auth_header: String,
    #[serde(default)]
    pub env_var: String,
}

fn default_bearer() -> String {
    "bearer".to_string()
}

fn default_auth_header() -> String {
    "Authorization".to_string()
}

/// Combined model + API info for making a vision request.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub model_id: String,
    pub provider_id: String,
    pub base_url: String,
    pub chat_endpoint: String,
    pub protocol: String,
    pub auth_type: String,
    pub auth_header: String,
    pub env_var: String,
    pub input_price: f64,
}

impl ResolvedModel {
    /// Resolve the API key from the environment.
    ///
    /// Only the provider's own env var and the explicit
    /// `SHIRABE_VISION_API_KEY` override are consulted — never generic
    /// fallbacks. A stray `OPENAI_API_KEY` must not be sent to a different
    /// provider just because its model happened to be cheapest.
    pub fn api_key(&self) -> Option<String> {
        for var in [self.env_var.as_str(), "SHIRABE_VISION_API_KEY"] {
            if var.is_empty() {
                continue;
            }
            if let Ok(key) = std::env::var(var) {
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }
        None
    }

    /// Build the full chat completions URL.
    pub fn chat_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        let endpoint = if self.chat_endpoint.is_empty() {
            "/chat/completions"
        } else {
            &self.chat_endpoint
        };
        format!("{base}{endpoint}")
    }

    /// Build the auth header value.
    pub fn auth_value(&self, key: &str) -> String {
        if self.auth_type == "bearer" {
            format!("Bearer {key}")
        } else {
            key.to_string()
        }
    }
}

// ── Registry resolution ──────────────────────────────────────────────────

/// Locate the `provider-registry` directory.
pub fn find_registry() -> Option<std::path::PathBuf> {
    if let Ok(explicit) = std::env::var("SHIRABE_VISION_REGISTRY") {
        if !explicit.is_empty() {
            // Returned as-is so a bad path surfaces in the "no models in
            // <path>" error instead of silently falling through.
            return Some(std::path::PathBuf::from(explicit));
        }
    }
    let cwd = std::env::current_dir().ok()?;
    [
        cwd.join("provider-registry"),
        cwd.join("../provider-registry"),
    ]
    .into_iter()
    .find(|candidate| candidate.join("models").is_dir() && candidate.join("entrypoint").is_dir())
}

/// Scan a provider-registry for vision models and resolve their API configs.
///
/// Returns a list of resolved models sorted by input price (cheapest first).
pub fn load_vision_models(registry_path: &Path) -> Vec<ResolvedModel> {
    let models_dir = registry_path.join("models");
    let entrypoint_dir = registry_path.join("entrypoint");

    // Load all entrypoints into a map: provider_id → ApiConfig.
    let mut entrypoints: std::collections::HashMap<String, ApiConfig> =
        std::collections::HashMap::new();
    if entrypoint_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&entrypoint_dir) {
            for entry in entries.flatten() {
                let toml_path = entry.path().join("default.toml");
                if toml_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&toml_path) {
                        if let Ok(ProviderEntry { inner }) =
                            toml::from_str::<ProviderEntry>(&content)
                        {
                            // Only OpenAI-compatible chat endpoints are
                            // supported (the request shape is fixed).
                            if inner.api.protocol == "openai_chat_v1" {
                                entrypoints.insert(inner.provider_id.clone(), inner.api);
                            }
                        }
                    }
                }
            }
        }
    }

    // Scan all model TOMLs for supports_vision = true.
    let mut models: Vec<ResolvedModel> = Vec::new();
    if models_dir.is_dir() {
        scan_models_dir(&models_dir, &entrypoints, &mut models);
    }

    // Sort by input price (cheapest first).
    models.sort_by(|a, b| {
        a.input_price
            .partial_cmp(&b.input_price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    models
}

fn scan_models_dir(
    dir: &Path,
    entrypoints: &std::collections::HashMap<String, ApiConfig>,
    out: &mut Vec<ResolvedModel>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_models_dir(&path, entrypoints, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(VisionModel { inner: m }) = toml::from_str::<VisionModel>(&content) {
                    if !m.supports_vision {
                        continue;
                    }
                    // Chat-review models: `max_output_tokens = 0` marks
                    // image/video GENERATION models (grok-imagine-*,
                    // gpt-image-*), which also accept image input but
                    // cannot answer a review prompt.
                    if m.max_output_tokens == 0 {
                        continue;
                    }
                    if let Some(api) = entrypoints.get(&m.provider_id) {
                        out.push(ResolvedModel {
                            model_id: m.id,
                            provider_id: m.provider_id,
                            base_url: api.base_url.clone(),
                            chat_endpoint: api.chat_endpoint.clone(),
                            protocol: api.protocol.clone(),
                            auth_type: api.auth_type.clone(),
                            auth_header: api.auth_header.clone(),
                            env_var: api.env_var.clone(),
                            input_price: m.pricing.input_per_million,
                        });
                    }
                }
            }
        }
    }
}

/// Pick the best model: explicit override (arg or `SHIRABE_VISION_MODEL`)
/// → cheapest with an available API key → first entry.
pub fn pick_model<'a>(
    models: &'a [ResolvedModel],
    explicit: Option<&str>,
) -> anyhow::Result<&'a ResolvedModel> {
    let pinned = explicit
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("SHIRABE_VISION_MODEL")
                .ok()
                .filter(|s| !s.is_empty())
        });
    if let Some(model_id) = pinned {
        return models
            .iter()
            .find(|m| m.model_id == model_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "vision model '{model_id}' not found in the provider registry \
                 (an explicit selection never falls back to another model)"
                )
            });
    }
    models
        .iter()
        .find(|m| m.api_key().is_some())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no vision model with an available API key — set the provider's \
             env var or SHIRABE_VISION_API_KEY, or pin SHIRABE_VISION_MODEL"
            )
        })
}

// ── Request ─────────────────────────────────────────────────────────────

/// Truncate at a char boundary — provider error bodies are frequently CJK,
/// and a raw byte slice at 500 would panic on a multi-byte boundary.
fn truncate_snippet(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Send a vision analysis request and return the model's text response.
pub async fn analyze_image(
    model: &ResolvedModel,
    prompt: &str,
    image_base64: &str,
    image_mime: &str,
) -> anyhow::Result<String> {
    let api_key = model.api_key().ok_or_else(|| {
        let hint = if model.env_var.is_empty() {
            "SHIRABE_VISION_API_KEY".to_string()
        } else {
            format!("{} or SHIRABE_VISION_API_KEY", model.env_var)
        };
        anyhow::anyhow!("no API key found. Set {hint}")
    })?;

    let data_url = format!("data:{image_mime};base64,{image_base64}");
    let body = serde_json::json!({
        "model": model.model_id,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": data_url}}
            ]
        }],
        "max_tokens": 4096,
        "temperature": 0.3
    });

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    let resp = client
        .post(model.chat_url())
        .header(&model.auth_header, model.auth_value(&api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("API error {status}: {}", truncate_snippet(&text, 500));
    }

    let chat_resp: ChatResponse = serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "failed to parse response: {e}\nraw: {}",
            truncate_snippet(&text, 500)
        )
    })?;

    chat_resp
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("no choices in response"))
}

// ── Response types (only what we need to read) ──────────────────────────

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_toml() -> &'static str {
        r#"
[model]
id = "glm-5v-turbo"
name = "GLM-5V-Turbo"
provider_id = "zhipu_glm"
supports_vision = true

[model.pricing]
input_per_million = 1.2
output_per_million = 4.0
"#
    }

    #[test]
    fn test_deserialize_model() {
        let vm: VisionModel = toml::from_str(model_toml()).unwrap();
        assert_eq!(vm.inner.id, "glm-5v-turbo");
        assert!(vm.inner.supports_vision);
        assert_eq!(vm.inner.pricing.input_per_million, 1.2);
    }

    #[test]
    fn test_resolved_model_urls() {
        let m = ResolvedModel {
            model_id: "m".into(),
            provider_id: "p".into(),
            base_url: "https://api.example.com/".into(),
            chat_endpoint: String::new(),
            protocol: "openai".into(),
            auth_type: "bearer".into(),
            auth_header: "Authorization".into(),
            env_var: "EXAMPLE_API_KEY".into(),
            input_price: 1.0,
        };
        assert_eq!(m.chat_url(), "https://api.example.com/chat/completions");
        assert_eq!(m.auth_value("k"), "Bearer k");
    }

    #[test]
    fn test_truncate_snippet_respects_char_boundaries() {
        let cjk = "错误".repeat(200); // 6 bytes/char, len 1200
        let out = truncate_snippet(&cjk, 500);
        assert!(out.len() <= 500);
        assert!(cjk.starts_with(out));
        // Slicing mid-codepoint would have panicked above.
        assert_eq!(truncate_snippet("short", 500), "short");
    }

    #[test]
    #[serial_test::serial]
    fn test_pick_model_explicit_is_hard() {
        // Host env must not leak into the selection.
        unsafe {
            std::env::remove_var("SHIRABE_VISION_API_KEY");
            std::env::remove_var("SHIRABE_VISION_MODEL");
        }
        let make = |id: &str, price: f64| ResolvedModel {
            model_id: id.to_string(),
            provider_id: "p".into(),
            base_url: "b".into(),
            chat_endpoint: String::new(),
            protocol: "openai_chat_v1".into(),
            auth_type: "bearer".into(),
            auth_header: "Authorization".into(),
            env_var: String::new(),
            input_price: price,
        };
        let models = vec![make("a", 0.1), make("b", 9.0)];
        assert_eq!(pick_model(&models, Some("b")).unwrap().model_id, "b");
        // An explicit typo is an error — never a silent fallback to another
        // provider's model.
        assert!(pick_model(&models, Some("does-not-exist")).is_err());
        // No keys anywhere and no pin → error (not "first entry").
        assert!(pick_model(&models, None).is_err());
    }

    #[test]
    #[serial_test::serial]
    fn test_api_key_only_provider_or_explicit() {
        let m = ResolvedModel {
            model_id: "m".into(),
            provider_id: "p".into(),
            base_url: "b".into(),
            chat_endpoint: String::new(),
            protocol: "openai_chat_v1".into(),
            auth_type: "bearer".into(),
            auth_header: "Authorization".into(),
            env_var: "SHIRABE_TEST_PROVIDER_KEY".into(),
            input_price: 1.0,
        };
        unsafe {
            std::env::remove_var("SHIRABE_TEST_PROVIDER_KEY");
            std::env::remove_var("SHIRABE_VISION_API_KEY");
            std::env::set_var("OPENAI_API_KEY", "must-not-be-used");
        }
        // A generic provider key must NOT be borrowed.
        assert!(m.api_key().is_none());
        unsafe {
            std::env::set_var("SHIRABE_VISION_API_KEY", "explicit-override");
        }
        assert_eq!(m.api_key().as_deref(), Some("explicit-override"));
        unsafe {
            std::env::remove_var("SHIRABE_VISION_API_KEY");
            std::env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn test_load_vision_models_filters_protocol_and_generation() {
        let dir =
            std::env::temp_dir().join(format!("shirabe-vision-registry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("models/xai")).unwrap();
        std::fs::create_dir_all(dir.join("models/openai")).unwrap();
        std::fs::create_dir_all(dir.join("entrypoint/xai")).unwrap();
        std::fs::create_dir_all(dir.join("entrypoint/openai")).unwrap();
        std::fs::create_dir_all(dir.join("entrypoint/anthropic")).unwrap();

        // OpenAI-compatible provider with a chat model + a generation model.
        std::fs::write(
            dir.join("entrypoint/xai/default.toml"),
            "[entrypoint]\nprovider_id = \"xai\"\n[entrypoint.api]\nprotocol = \"openai_chat_v1\"\nbase_url = \"https://api.x.ai/v1\"\nenv_var = \"XAI_API_KEY\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("entrypoint/openai/default.toml"),
            "[entrypoint]\nprovider_id = \"openai\"\n[entrypoint.api]\nprotocol = \"openai_chat_v1\"\nbase_url = \"https://api.openai.com/v1\"\nenv_var = \"OPENAI_API_KEY\"\n",
        )
        .unwrap();
        // Non-OpenAI protocol → must never be selectable.
        std::fs::write(
            dir.join("entrypoint/anthropic/default.toml"),
            "[entrypoint]\nprovider_id = \"anthropic\"\n[entrypoint.api]\nprotocol = \"anthropic_messages_v1\"\nbase_url = \"https://api.anthropic.com/v1\"\nenv_var = \"ANTHROPIC_API_KEY\"\n",
        )
        .unwrap();

        let model = |provider: &str, id: &str, out: u64, price: f64| {
            format!(
                "[model]\nid = \"{id}\"\nname = \"{id}\"\nprovider_id = \"{provider}\"\nsupports_vision = true\nmax_output_tokens = {out}\n\n[model.pricing]\ninput_per_million = {price}\noutput_per_million = 0.0\n"
            )
        };
        std::fs::write(
            dir.join("models/xai/chat-model.toml"),
            model("xai", "chat-model", 16384, 1.0),
        )
        .unwrap();
        std::fs::write(
            dir.join("models/xai/grok-imagine-video.toml"),
            model("xai", "grok-imagine-video", 0, 0.0),
        )
        .unwrap();
        // anthropic model: provider has no openai-compatible entrypoint → excluded.
        std::fs::write(
            dir.join("models/openai/claude.toml"),
            model("anthropic", "claude-x", 8192, 0.5),
        )
        .unwrap();

        let models = load_vision_models(&dir);
        let ids: Vec<&str> = models.iter().map(|m| m.model_id.as_str()).collect();
        // Chat model kept; generation model (out=0) dropped; non-openai
        // provider dropped.
        assert_eq!(ids, vec!["chat-model"]);
        assert_eq!(models[0].protocol, "openai_chat_v1");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
