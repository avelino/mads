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

    #[tokio::test]
    async fn cli_providers_have_no_model_list() {
        let error = list_models("claude-cli", None, None).await.unwrap_err();
        assert!(error.contains("picks its own model"), "{error}");
    }

    #[tokio::test]
    async fn openai_compat_needs_an_endpoint_and_a_key() {
        let error = list_models("openai-compat", None, Some("http://localhost:8000/v1"))
            .await
            .unwrap_err();
        assert!(error.contains("MADS_API_KEY"), "{error}");
        let error = list_models("openai-compat", Some("k"), None)
            .await
            .unwrap_err();
        assert!(error.contains("endpoint"), "{error}");
    }
}
