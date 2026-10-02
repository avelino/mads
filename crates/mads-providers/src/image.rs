//! Image generation backends for the image step: Gemini and OpenAI over HTTP.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use mads_core::{
    google::AspectRatio,
    images::{ImageModel, ImageRequest, SolidImageModel},
};
use reqwest::{Client, multipart};
use serde_json::{Value, json};

use crate::select::{ProviderError, ProviderStatus};

/// Image models take far longer than a chat turn.
const TIMEOUT: Duration = Duration::from_secs(180);
const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com";
const OPENAI_BASE: &str = "https://api.openai.com";
pub const GEMINI_IMAGE_MODEL: &str = "gemini-2.5-flash-image";
pub const OPENAI_IMAGE_MODEL: &str = "gpt-image-1";
const ERROR_EXCERPT: usize = 300;

fn client() -> Result<Client, ProviderError> {
    Client::builder()
        .user_agent(concat!("mads/", env!("CARGO_PKG_VERSION")))
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| ProviderError::Config(format!("cannot start the HTTP client: {e}")))
}

/// Body of a response, or an error with its status and the start of its body.
async fn read_json(resp: reqwest::Response) -> Result<Value, String> {
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let excerpt: String = text.chars().take(ERROR_EXCERPT).collect();
        return Err(format!("HTTP {}: {excerpt}", status.as_u16()));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON from the image API: {e}"))
}

fn decode(b64: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(b64)
        .map_err(|e| format!("invalid base64 image: {e}"))
}

fn mime_of(bytes: &[u8]) -> &'static str {
    match mads_core::images::extension(bytes) {
        Some("jpg") => "image/jpeg",
        _ => "image/png",
    }
}

pub struct GeminiImageModel {
    client: Client,
    base: String,
    key: String,
    model: String,
}

impl GeminiImageModel {
    pub fn new(base: &str, key: &str, model: &str) -> Result<Self, ProviderError> {
        Ok(Self {
            client: client()?,
            base: base.trim_end_matches('/').to_string(),
            key: key.to_string(),
            model: model.to_string(),
        })
    }
}

/// Gemini has no 1.91:1: landscape comes from 16:9 and is cropped by the image step.
fn gemini_ratio(r: AspectRatio) -> &'static str {
    match r {
        AspectRatio::Landscape => "16:9",
        AspectRatio::Square => "1:1",
        AspectRatio::Portrait => "4:5",
        AspectRatio::Vertical => "9:16",
    }
}

fn gemini_body(req: &ImageRequest) -> Value {
    let mut parts = vec![json!({"text": req.prompt})];
    if let Some(r) = &req.reference {
        parts.push(json!({"inline_data": {"mime_type": mime_of(r), "data": STANDARD.encode(r)}}));
    }
    json!({
        "contents": [{"role": "user", "parts": parts}],
        "generationConfig": {
            "responseModalities": ["IMAGE"],
            "imageConfig": {"aspectRatio": gemini_ratio(req.ratio)},
        },
    })
}

/// The first image part of the first candidate. A text-only answer is the model's refusal.
fn gemini_image(v: &Value) -> Result<Vec<u8>, String> {
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let data = parts.iter().find_map(|p| {
        p.get("inlineData")
            .or_else(|| p.get("inline_data"))
            .and_then(|d| d["data"].as_str())
    });
    if let Some(d) = data {
        return decode(d);
    }
    let said: Vec<&str> = parts.iter().filter_map(|p| p["text"].as_str()).collect();
    let reason = v["candidates"][0]["finishReason"].as_str().unwrap_or("");
    Err(
        format!("no image in the answer {reason} {}", said.join(" "))
            .trim()
            .to_string(),
    )
}

#[async_trait]
impl ImageModel for GeminiImageModel {
    fn id(&self) -> String {
        format!("gemini:{}", self.model)
    }

    async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String> {
        let url = format!("{}/v1beta/models/{}:generateContent", self.base, self.model);
        let resp = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.key)
            .json(&gemini_body(req))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        gemini_image(&read_json(resp).await?)
    }
}

pub struct OpenAiImageModel {
    client: Client,
    base: String,
    key: String,
    model: String,
}

impl OpenAiImageModel {
    pub fn new(base: &str, key: &str, model: &str) -> Result<Self, ProviderError> {
        Ok(Self {
            client: client()?,
            base: base.trim_end_matches('/').to_string(),
            key: key.to_string(),
            model: model.to_string(),
        })
    }

    async fn edit(&self, req: &ImageRequest, reference: &[u8]) -> Result<Value, String> {
        let mime = mime_of(reference);
        let file = multipart::Part::bytes(reference.to_vec())
            .file_name(if mime == "image/jpeg" {
                "reference.jpg"
            } else {
                "reference.png"
            })
            .mime_str(mime)
            .map_err(|e| e.to_string())?;
        let form = multipart::Form::new()
            .text("model", self.model.clone())
            .text("prompt", req.prompt.clone())
            .text("size", openai_size(req.ratio))
            .part("image[]", file);
        let resp = self
            .client
            .post(format!("{}/v1/images/edits", self.base))
            .bearer_auth(&self.key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        read_json(resp).await
    }

    async fn create(&self, req: &ImageRequest) -> Result<Value, String> {
        let body = json!({"model": self.model, "prompt": req.prompt, "size": openai_size(req.ratio), "n": 1});
        let resp = self
            .client
            .post(format!("{}/v1/images/generations", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        read_json(resp).await
    }
}

/// gpt-image sizes closest to each ratio; the image step crops the rest.
fn openai_size(r: AspectRatio) -> &'static str {
    match r {
        AspectRatio::Landscape => "1536x1024",
        AspectRatio::Square => "1024x1024",
        AspectRatio::Portrait | AspectRatio::Vertical => "1024x1536",
    }
}

#[async_trait]
impl ImageModel for OpenAiImageModel {
    fn id(&self) -> String {
        format!("openai:{}", self.model)
    }

    async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String> {
        let v = match &req.reference {
            Some(r) => self.edit(req, r).await?,
            None => self.create(req).await?,
        };
        let b64 = v["data"][0]["b64_json"]
            .as_str()
            .ok_or("no b64_json in the answer")?;
        decode(b64)
    }
}

/// Image providers `--image-provider` accepts, with the key each one needs.
const IMAGE_PROVIDERS: [(&str, Option<&str>); 3] = [
    ("gemini", Some("GEMINI_API_KEY")),
    ("openai", Some("OPENAI_API_KEY")),
    ("none", None),
];

/// Readiness of each image provider, for `mads providers`.
pub fn list_image_providers() -> Vec<(&'static str, ProviderStatus)> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    IMAGE_PROVIDERS
        .iter()
        .map(|(name, key)| {
            let status = match key {
                Some(k) if env(k).is_none() => ProviderStatus::MissingEnv((*k).into()),
                _ => ProviderStatus::Ready,
            };
            (*name, status)
        })
        .collect()
}

/// The image model for `--image-provider`. `auto` picks the first provider with a key, or none.
pub fn build_image_model(
    provider: &str,
    model: Option<&str>,
) -> Result<Option<Arc<dyn ImageModel>>, ProviderError> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    image_model_with(provider, model, &env)
}

pub(crate) fn image_model_with(
    provider: &str,
    model: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Arc<dyn ImageModel>>, ProviderError> {
    let key = |var: &str| {
        env(var).ok_or_else(|| {
            ProviderError::Config(format!("--image-provider {provider} needs {var}"))
        })
    };
    let base = |var: &str, default: &str| env(var).unwrap_or_else(|| default.to_string());
    let model_or = |default: &str| model.unwrap_or(default).to_string();
    let built: Arc<dyn ImageModel> = match provider {
        "" | "auto" => {
            return match (env("GEMINI_API_KEY"), env("OPENAI_API_KEY")) {
                (Some(_), _) => image_model_with("gemini", model, env),
                (None, Some(_)) => image_model_with("openai", model, env),
                (None, None) => Ok(None),
            };
        }
        "none" => return Ok(None),
        "solid" => Arc::new(SolidImageModel),
        "gemini" => Arc::new(GeminiImageModel::new(
            &base("MADS_GEMINI_BASE_URL", GEMINI_BASE),
            &key("GEMINI_API_KEY")?,
            &model_or(GEMINI_IMAGE_MODEL),
        )?),
        "openai" => Arc::new(OpenAiImageModel::new(
            &base("MADS_OPENAI_BASE_URL", OPENAI_BASE),
            &key("OPENAI_API_KEY")?,
            &model_or(OPENAI_IMAGE_MODEL),
        )?),
        other => {
            return Err(ProviderError::Config(format!(
                "unknown image provider '{other}': use auto, gemini, openai or none"
            )));
        }
    };
    Ok(Some(built))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use mads_core::images::solid_png;
    use wiremock::{
        Mock, MockServer, Request, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;

    fn req(reference: bool) -> ImageRequest {
        ImageRequest {
            prompt: "a table".into(),
            ratio: AspectRatio::Landscape,
            reference: reference.then(|| solid_png(AspectRatio::Square, [1, 1, 1])),
        }
    }

    fn png_b64() -> String {
        STANDARD.encode(solid_png(AspectRatio::Square, [5, 5, 5]))
    }

    fn body_of(r: &Request) -> Value {
        serde_json::from_slice(&r.body).unwrap()
    }

    #[tokio::test]
    async fn gemini_sends_prompt_ratio_reference_and_reads_the_image() {
        let server = MockServer::start().await;
        let answer = json!({"candidates": [{"content": {"parts": [{"text": "here"}, {"inlineData": {"mimeType": "image/png", "data": png_b64()}}]}}]});
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-x:generateContent"))
            .and(header("x-goog-api-key", "k"))
            .respond_with(ResponseTemplate::new(200).set_body_json(answer))
            .mount(&server)
            .await;
        let m = GeminiImageModel::new(&server.uri(), "k", "gemini-x").unwrap();
        let bytes = m.generate(&req(true)).await.unwrap();
        assert_eq!(mads_core::images::extension(&bytes), Some("png"));
        let sent = body_of(&server.received_requests().await.unwrap()[0]);
        assert_eq!(
            sent["generationConfig"]["imageConfig"]["aspectRatio"],
            "16:9"
        );
        assert_eq!(sent["contents"][0]["parts"][0]["text"], "a table");
        assert_eq!(
            sent["contents"][0]["parts"][1]["inline_data"]["mime_type"],
            "image/png"
        );
    }

    #[tokio::test]
    async fn gemini_without_an_image_reports_the_refusal() {
        let server = MockServer::start().await;
        let answer = json!({"candidates": [{"finishReason": "SAFETY", "content": {"parts": [{"text": "cannot draw that"}]}}]});
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(answer))
            .mount(&server)
            .await;
        let m = GeminiImageModel::new(&server.uri(), "k", "g").unwrap();
        let err = m.generate(&req(false)).await.unwrap_err();
        assert!(
            err.contains("SAFETY") && err.contains("cannot draw that"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn http_errors_carry_status_and_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(429).set_body_string("quota exceeded"))
            .mount(&server)
            .await;
        let m = OpenAiImageModel::new(&server.uri(), "k", "gpt-image-1").unwrap();
        let err = m.generate(&req(false)).await.unwrap_err();
        assert_eq!(err, "HTTP 429: quota exceeded");
    }

    #[tokio::test]
    async fn openai_creates_without_reference_and_edits_with_one() {
        let server = MockServer::start().await;
        let answer = json!({"data": [{"b64_json": png_b64()}]});
        for p in ["/v1/images/generations", "/v1/images/edits"] {
            Mock::given(method("POST"))
                .and(path(p))
                .and(header("authorization", "Bearer k"))
                .respond_with(ResponseTemplate::new(200).set_body_json(answer.clone()))
                .mount(&server)
                .await;
        }
        let m = OpenAiImageModel::new(&server.uri(), "k", "gpt-image-1").unwrap();
        m.generate(&req(false)).await.unwrap();
        m.generate(&req(true)).await.unwrap();
        let got = server.received_requests().await.unwrap();
        assert_eq!(got[0].url.path(), "/v1/images/generations");
        assert_eq!(body_of(&got[0])["size"], "1536x1024");
        assert_eq!(got[1].url.path(), "/v1/images/edits");
        let form = String::from_utf8_lossy(&got[1].body);
        assert!(form.contains("name=\"image[]\"") && form.contains("1536x1024"));
    }

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn id(provider: &str, vars: &[(&str, &str)]) -> Option<String> {
        image_model_with(provider, None, &env(vars))
            .unwrap()
            .map(|m| m.id())
    }

    #[test]
    fn auto_prefers_gemini_then_openai_then_none() {
        let both = [("GEMINI_API_KEY", "g"), ("OPENAI_API_KEY", "o")];
        assert_eq!(
            id("auto", &both).as_deref(),
            Some("gemini:gemini-2.5-flash-image")
        );
        assert_eq!(id("", &both[1..]).as_deref(), Some("openai:gpt-image-1"));
        assert_eq!(id("auto", &[]), None);
        assert_eq!(id("none", &both), None);
        assert_eq!(id("solid", &[]).as_deref(), Some("solid"));
    }

    #[test]
    fn an_explicit_provider_needs_its_key_and_a_known_name() {
        let err = image_model_with("openai", None, &env(&[])).err().unwrap();
        assert!(err.to_string().contains("OPENAI_API_KEY"));
        assert!(image_model_with("dalle", None, &env(&[])).is_err());
        let custom = image_model_with(
            "gemini",
            Some("gemini-3-pro-image"),
            &env(&[("GEMINI_API_KEY", "g")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(custom.id(), "gemini:gemini-3-pro-image");
    }
}
