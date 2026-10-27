//! Lists the model ids a provider currently serves, for the studio forms.
//!
//! One GET per provider against its public models endpoint, with the key the
//! studio resolved. The list is a convenience, never a contract: a free text id
//! is always accepted, exactly like the CLI.

use std::time::Duration;

/// The model ids the provider serves today, sorted. `base_url` only matters for
/// `openai-compat` and `ollama`; the rest have fixed endpoints. CLI providers
/// have no list: the agent picks its own default.
pub async fn list_models(
    provider: &str,
    key: Option<&str>,
    base_url: Option<&str>,
) -> Result<Vec<String>, String> {
    let request_builder = match provider {
        "anthropic" => reqwest::Client::new()
            .get("https://api.anthropic.com/v1/models?limit=1000")
            .header("x-api-key", key.ok_or("ANTHROPIC_API_KEY is missing")?)
            .header("anthropic-version", "2023-06-01"),
        "openai" => bearer_get(
            "https://api.openai.com/v1/models",
            key.ok_or("OPENAI_API_KEY is missing")?,
        ),
        "openrouter" => bearer_get(
            "https://openrouter.ai/api/v1/models",
            key.ok_or("OPENROUTER_API_KEY is missing")?,
        ),
        "groq" => bearer_get(
            "https://api.groq.com/openai/v1/models",
            key.ok_or("GROQ_API_KEY is missing")?,
        ),
        "deepseek" => bearer_get(
            "https://api.deepseek.com/models",
            key.ok_or("DEEPSEEK_API_KEY is missing")?,
        ),
        "xai" => bearer_get(
            "https://api.x.ai/v1/models",
            key.ok_or("XAI_API_KEY is missing")?,
        ),
        "gemini" => reqwest::Client::new()
            .get("https://generativelanguage.googleapis.com/v1beta/models")
            .header("x-goog-api-key", key.ok_or("GEMINI_API_KEY is missing")?),
        "openai-compat" => {
            let base = base_url
                .map(str::trim)
                .filter(|base| !base.is_empty())
                .ok_or("the endpoint (base URL) is missing for openai-compat")?;
            check_base_url(base)?;
            bearer_get(
                format!("{}/models", base.trim_end_matches('/')),
                key.ok_or("MADS_API_KEY is missing")?,
            )
        }
        "ollama" => {
            let base = base_url
                .map(str::trim)
                .filter(|base| !base.is_empty())
                .unwrap_or("http://127.0.0.1:11434");
            check_base_url(base)?;
            reqwest::Client::new().get(format!("{}/api/tags", base.trim_end_matches('/')))
        }
        p if p.ends_with("-cli") => {
            return Err(format!(
                "{p} runs a local agent that picks its own model; leave the field empty or type one"
            ));
        }
        other => return Err(format!("unknown provider: {other}")),
    };
    let response = request_builder
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("could not reach the provider: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body: &str = body.split('\n').next().unwrap_or("");
        return Err(format!(
            "the provider answered HTTP {status}: {}",
            body.chars().take(200).collect::<String>()
        ));
    }
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("the provider answer is not JSON: {e}"))?;
    Ok(models_from_json(provider, &json))
}

/// The documented API root of each provider, for the studio endpoint field.
/// `openai-compat` has no default: the endpoint is the user's own service.
pub fn default_endpoint(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("https://api.anthropic.com"),
        "openai" => Some("https://api.openai.com/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "groq" => Some("https://api.groq.com/openai/v1"),
        "deepseek" => Some("https://api.deepseek.com"),
        "xai" => Some("https://api.x.ai/v1"),
        "gemini" => Some("https://generativelanguage.googleapis.com"),
        "ollama" => Some("http://127.0.0.1:11434"),
        _ => None,
    }
}

/// The endpoint URLs are caller supplied, so the studio must not become a
/// proxy into the network it runs on: http(s) only, and private, loopback and
/// link-local addresses are refused unless `MADS_ALLOW_PRIVATE_HOSTS` is set
/// (the same escape hatch the site fetcher honors). A public hostname that
/// resolves private is still possible; the single-operator threat model
/// accepts that.
fn check_base_url(base: &str) -> Result<(), String> {
    let url =
        reqwest::Url::parse(base).map_err(|_| "the endpoint must be a valid URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("the endpoint must be an http(s) URL".into());
    }
    let host = url.host_str().unwrap_or_default().trim_end_matches('.');
    if host.is_empty() {
        return Err("the endpoint has no host".into());
    }
    if std::env::var_os("MADS_ALLOW_PRIVATE_HOSTS").is_some_and(|v| !v.is_empty()) {
        return Ok(());
    }
    let blocked = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local()
            }
        });
    if blocked {
        return Err(
            "the endpoint cannot point to a private address; set MADS_ALLOW_PRIVATE_HOSTS to allow it"
                .into(),
        );
    }
    Ok(())
}

fn bearer_get(url: impl Into<String>, key: &str) -> reqwest::RequestBuilder {
    reqwest::Client::new().get(url.into()).bearer_auth(key)
}

/// Reads the id list from the three shapes providers use: OpenAI's `data[].id`,
/// Gemini's `models[].name` and Ollama's `models[].name`.
fn models_from_json(provider: &str, json: &serde_json::Value) -> Vec<String> {
    let items = match json.get("data").or_else(|| json.get("models")) {
        Some(items) => items.as_array().cloned().unwrap_or_default(),
        None => Vec::new(),
    };
    let mut models: Vec<String> = items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").or_else(|| item.get("name"))?.as_str()?;
            let id = if provider == "gemini" {
                id.strip_prefix("models/").unwrap_or(id)
            } else {
                id
            };
            (!id.is_empty()).then(|| id.to_string())
        })
        .collect();
    models.sort();
    models.dedup();
    models
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openai_shape_reads_data_ids() {
        let json = json!({"data": [{"id": "gpt-b"}, {"id": "gpt-a"}, {"id": "gpt-a"}]});
        assert_eq!(models_from_json("openai", &json), vec!["gpt-a", "gpt-b"]);
    }

    #[test]
    fn gemini_shape_strips_the_models_prefix() {
        let json = json!({"models": [{"name": "models/gemini-2.0-flash"}, {"name": "models/gemini-2.5-pro"}]});
        assert_eq!(
            models_from_json("gemini", &json),
            vec!["gemini-2.0-flash", "gemini-2.5-pro"]
        );
    }

    #[test]
    fn an_empty_or_foreign_shape_is_an_empty_list() {
        assert!(models_from_json("openai", &json!({"error": "x"})).is_empty());
        assert!(models_from_json("openai", &json!([])).is_empty());
    }

    #[test]
    fn known_providers_have_a_default_endpoint_and_compat_does_not() {
        assert_eq!(
            default_endpoint("openai"),
            Some("https://api.openai.com/v1")
        );
        assert_eq!(default_endpoint("openai-compat"), None);
        assert_eq!(default_endpoint("claude-cli"), None);
        assert_eq!(default_endpoint("nope"), None);
    }

    #[test]
    fn the_endpoint_guard_refuses_private_targets_and_bad_schemes() {
        assert!(check_base_url("http://localhost:8000/v1").is_err());
        assert!(check_base_url("http://127.0.0.1:8000/v1").is_err());
        assert!(check_base_url("http://10.0.0.5:8000/v1").is_err());
        assert!(check_base_url("http://192.168.1.10/v1").is_err());
        assert!(check_base_url("file:///etc/passwd").is_err());
        assert!(check_base_url("not a url").is_err());
        // Public endpoints pass; the request itself is what would fail offline.
        assert!(check_base_url("https://api.example.com/v1").is_ok());
    }

    #[tokio::test]
    async fn cli_providers_have_no_model_list() {
        let error = list_models("claude-cli", None, None).await.unwrap_err();
        assert!(error.contains("picks its own model"), "{error}");
    }

    #[tokio::test]
    async fn openai_compat_needs_an_endpoint_and_a_key() {
        // The endpoint guard runs before the key check, so the URL must be a
        // public one for the key error to surface.
        let error = list_models("openai-compat", None, Some("https://api.example.com/v1"))
            .await
            .unwrap_err();
        assert!(error.contains("MADS_API_KEY"), "{error}");
        let error = list_models("openai-compat", Some("k"), None)
            .await
            .unwrap_err();
        assert!(error.contains("endpoint"), "{error}");
    }
}
