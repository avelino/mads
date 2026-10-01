use std::collections::HashMap;

use async_trait::async_trait;
use genai::{
    Client, ModelIden, ServiceTarget,
    adapter::AdapterKind,
    chat::{
        ChatMessage, ChatOptions, ChatRequest as GenaiRequest, ChatResponse as GenaiResponse,
        ContentPart, MessageContent, Tool, ToolCall as GenaiCall, ToolResponse,
    },
    resolver::{AuthData, Endpoint},
    webc,
};
use mads_core::{
    agent::{ChatError, ChatModel, ChatRequest, ChatResponse, Message, ToolCall},
    usage::Usage,
};
use serde_json::json;

/// Output ceiling that every provider we target accepts (groq and deepseek stop at 8192).
const MAX_OUTPUT_TOKENS: u32 = 8192;
const MAX_ERROR_CHARS: usize = 600;

/// Chat completions through the `genai` crate: Anthropic, OpenAI, Gemini, OpenRouter, Groq,
/// DeepSeek, xAI, Ollama and any OpenAI-compatible endpoint.
pub struct GenaiChatModel {
    client: Client,
    model: String,
    label: String,
    options: ChatOptions,
}

fn options() -> ChatOptions {
    ChatOptions::default().with_max_tokens(MAX_OUTPUT_TOKENS)
}

/// (mads name, genai namespace, API key variable). The variable is resolved explicitly so the
/// documented name always works, even where genai reads a different one (OpenRouter).
const API_PROVIDERS: [(&str, &str, Option<&str>); 8] = [
    ("anthropic", "anthropic", Some("ANTHROPIC_API_KEY")),
    ("openai", "openai", Some("OPENAI_API_KEY")),
    ("gemini", "gemini", Some("GEMINI_API_KEY")),
    ("openrouter", "open_router", Some("OPENROUTER_API_KEY")),
    ("groq", "groq", Some("GROQ_API_KEY")),
    ("deepseek", "deepseek", Some("DEEPSEEK_API_KEY")),
    ("xai", "xai", Some("XAI_API_KEY")),
    ("ollama", "ollama", None),
];

pub(crate) fn api_provider(name: &str) -> Option<(&'static str, Option<&'static str>)> {
    API_PROVIDERS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, ns, env)| (*ns, *env))
}

impl GenaiChatModel {
    /// `provider` is the mads name (`anthropic`, `openrouter`...). None for unknown names.
    pub fn for_provider(provider: &str, model: &str) -> Option<Self> {
        let (namespace, env) = api_provider(provider)?;
        let mut builder = Client::builder();
        if let Some(var) = env {
            builder = builder
                .with_auth_resolver_fn(move |_: ModelIden| Ok(Some(AuthData::from_env(var))));
        }
        Some(Self {
            client: builder.build(),
            model: format!("{namespace}::{model}"),
            label: format!("{provider}/{model}"),
            options: options(),
        })
    }

    /// Any server that speaks the OpenAI chat completions API. `base_url` ends at the version
    /// segment, for example `http://localhost:8000/v1`.
    pub fn openai_compat(base_url: &str, model: &str, api_key: Option<String>) -> Self {
        let base = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{base_url}/")
        };
        let key = api_key.unwrap_or_else(|| "none".to_string());
        let client = Client::builder()
            .with_service_target_resolver_fn(move |target: ServiceTarget| {
                Ok(ServiceTarget {
                    endpoint: Endpoint::from_owned(base.clone()),
                    auth: AuthData::from_single(key.clone()),
                    model: ModelIden::new(AdapterKind::OpenAI, target.model.model_name),
                })
            })
            .build();
        Self {
            client,
            model: model.to_string(),
            label: format!("openai-compat/{model}"),
            options: options(),
        }
    }
}

#[async_trait]
impl ChatModel for GenaiChatModel {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, ChatError> {
        let resp = self
            .client
            .exec_chat(self.model.as_str(), to_genai(req), Some(&self.options))
            .await
            .map_err(classify)?;
        Ok(from_genai(resp))
    }
}

pub(crate) fn to_genai(req: &ChatRequest) -> GenaiRequest {
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut messages = Vec::with_capacity(req.messages.len());
    for m in &req.messages {
        match m {
            Message::User(text) => messages.push(ChatMessage::user(text.clone())),
            Message::Assistant { text, tool_calls } => {
                tool_calls.iter().for_each(|c| {
                    names.insert(c.id.as_str(), c.name.as_str());
                });
                messages.push(assistant_message(text.as_deref(), tool_calls));
            }
            Message::ToolResults(results) => {
                let responses: Vec<ToolResponse> = results
                    .iter()
                    .map(|r| {
                        let response = ToolResponse::new(r.call_id.clone(), r.content.clone());
                        match names.get(r.call_id.as_str()) {
                            Some(name) => response.with_fn_name(*name),
                            None => response,
                        }
                    })
                    .collect();
                messages.push(ChatMessage::from(responses));
            }
        }
    }
    let mut out = GenaiRequest::from_messages(messages);
    if !req.system.is_empty() {
        out = out.with_system(req.system.clone());
    }
    if !req.tools.is_empty() {
        let tools = req.tools.iter().map(|t| {
            Tool::new(t.name.clone())
                .with_description(t.description.clone())
                .with_schema(t.input_schema.clone())
        });
        out = out.with_tools(tools);
    }
    out
}

/// Text first, then Gemini thought signatures (they must precede the calls), then the calls.
fn assistant_message(text: Option<&str>, calls: &[ToolCall]) -> ChatMessage {
    let mut parts: Vec<ContentPart> = Vec::new();
    if let Some(t) = text.filter(|t| !t.is_empty()) {
        parts.push(ContentPart::Text(t.to_string()));
    }
    let signatures = calls
        .first()
        .and_then(|c| c.extra.as_ref())
        .and_then(|e| e.get("thought_signatures"))
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    parts.extend(signatures.into_iter().map(ContentPart::ThoughtSignature));
    parts.extend(calls.iter().map(|c| {
        ContentPart::ToolCall(GenaiCall {
            call_id: c.id.clone(),
            fn_name: c.name.clone(),
            fn_arguments: c.arguments.clone(),
            thought_signatures: None,
        })
    }));
    if parts.is_empty() {
        parts.push(ContentPart::Text(String::new()));
    }
    ChatMessage::assistant(MessageContent::from_parts(parts))
}

fn from_genai(resp: GenaiResponse) -> ChatResponse {
    let usage = usage_of(&resp.usage);
    let text = resp.content.joined_texts().filter(|t| !t.trim().is_empty());
    let tool_calls = resp
        .content
        .tool_calls()
        .into_iter()
        .map(|c| ToolCall {
            id: c.call_id.clone(),
            name: c.fn_name.clone(),
            arguments: c.fn_arguments.clone(),
            extra: c
                .thought_signatures
                .as_ref()
                .map(|s| json!({"thought_signatures": s})),
        })
        .collect();
    ChatResponse {
        text,
        tool_calls,
        usage,
    }
}

pub(crate) fn usage_of(u: &genai::chat::Usage) -> Usage {
    let clamp = |v: Option<i32>| u64::try_from(v.unwrap_or(0)).unwrap_or(0);
    Usage {
        input_tokens: clamp(u.prompt_tokens),
        output_tokens: clamp(u.completion_tokens),
        cost_usd: None,
    }
}

fn status_of(e: &genai::Error) -> Option<u16> {
    match e {
        genai::Error::WebModelCall {
            webc_error: webc::Error::ResponseFailedStatus { status, .. },
            ..
        }
        | genai::Error::WebAdapterCall {
            webc_error: webc::Error::ResponseFailedStatus { status, .. },
            ..
        } => Some(status.as_u16()),
        genai::Error::HttpError { status, .. } => Some(status.as_u16()),
        _ => None,
    }
}

fn is_network(e: &genai::Error) -> bool {
    matches!(
        e,
        genai::Error::WebModelCall {
            webc_error: webc::Error::Reqwest(_),
            ..
        } | genai::Error::WebAdapterCall {
            webc_error: webc::Error::Reqwest(_),
            ..
        } | genai::Error::WebStream { .. }
    )
}

/// 408, 429, 5xx and network failures are worth retrying. Everything else is the caller's problem.
pub(crate) fn classify(e: genai::Error) -> ChatError {
    let mut message = e.to_string();
    if message.chars().count() > MAX_ERROR_CHARS {
        message = format!(
            "{}…",
            message.chars().take(MAX_ERROR_CHARS).collect::<String>()
        );
    }
    let transient = match status_of(&e) {
        Some(code) => code == 408 || code == 429 || code >= 500,
        None => is_network(&e),
    };
    if transient {
        ChatError::Transient(message)
    } else {
        ChatError::Fatal(message)
    }
}

#[cfg(test)]
mod tests {
    use genai::{
        ModelIden,
        adapter::AdapterKind,
        chat::{ChatRole, ContentPart},
        webc,
    };
    use mads_core::{
        agent::{ChatError, ChatModel, ChatRequest, Message, ToolCall, ToolResult},
        tools::ToolSpec,
    };
    use reqwest::{StatusCode, header::HeaderMap};
    use serde_json::{Value, json};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    fn spec() -> ToolSpec {
        ToolSpec {
            name: "get_business".into(),
            description: "Business profile".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    fn call(id: &str, extra: Option<Value>) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: "get_business".into(),
            arguments: json!({}),
            extra,
        }
    }

    #[test]
    fn request_carries_system_user_message_and_tools() {
        let req = ChatRequest {
            system: "sys".into(),
            messages: vec![Message::User("go".into())],
            tools: vec![spec()],
        };
        let g = to_genai(&req);
        assert_eq!(g.system.as_deref(), Some("sys"));
        assert_eq!(g.messages.len(), 1);
        assert!(matches!(g.messages[0].role, ChatRole::User));
        let tools = g.tools.expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].description.as_deref(), Some("Business profile"));
        assert_eq!(
            tools[0].schema,
            Some(json!({"type": "object", "properties": {}}))
        );
    }

    #[test]
    fn no_tools_means_no_tools_field() {
        let req = ChatRequest {
            system: String::new(),
            messages: vec![Message::User("go".into())],
            tools: vec![],
        };
        assert!(to_genai(&req).tools.is_none());
    }

    #[test]
    fn assistant_turn_keeps_text_then_tool_calls() {
        let req = ChatRequest {
            system: String::new(),
            messages: vec![Message::Assistant {
                text: Some("let me look".into()),
                tool_calls: vec![call("c1", None)],
            }],
            tools: vec![],
        };
        let g = to_genai(&req);
        assert!(matches!(g.messages[0].role, ChatRole::Assistant));
        let parts = g.messages[0].content.parts();
        assert!(matches!(&parts[0], ContentPart::Text(t) if t == "let me look"));
        assert!(
            matches!(&parts[1], ContentPart::ToolCall(c) if c.call_id == "c1" && c.fn_name == "get_business")
        );
    }

    #[test]
    fn gemini_thought_signatures_travel_with_the_first_call() {
        let extra = json!({"thought_signatures": ["sig-1"]});
        let req = ChatRequest {
            system: String::new(),
            messages: vec![Message::Assistant {
                text: None,
                tool_calls: vec![call("c1", Some(extra))],
            }],
            tools: vec![],
        };
        let parts = to_genai(&req).messages.remove(0).content.into_parts();
        assert!(matches!(&parts[0], ContentPart::ThoughtSignature(s) if s == "sig-1"));
        assert!(matches!(&parts[1], ContentPart::ToolCall(_)));
    }

    #[test]
    fn tool_results_become_tool_responses_with_the_function_name() {
        let req = ChatRequest {
            system: String::new(),
            messages: vec![
                Message::Assistant {
                    text: None,
                    tool_calls: vec![call("c1", None)],
                },
                Message::ToolResults(vec![ToolResult {
                    call_id: "c1".into(),
                    content: "{\"ok\":true}".into(),
                    is_error: false,
                }]),
            ],
            tools: vec![],
        };
        let g = to_genai(&req);
        assert!(matches!(g.messages[1].role, ChatRole::Tool));
        let responses = g.messages[1].content.tool_responses();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].call_id, "c1");
        assert_eq!(responses[0].fn_name.as_deref(), Some("get_business"));
        assert_eq!(responses[0].content, "{\"ok\":true}");
    }

    #[test]
    fn usage_clamps_missing_and_negative_values() {
        let u = genai::chat::Usage {
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
            total_tokens: Some(15),
            ..Default::default()
        };
        let m = usage_of(&u);
        assert_eq!((m.input_tokens, m.output_tokens, m.cost_usd), (10, 5, None));
        let none = usage_of(&genai::chat::Usage::default());
        assert_eq!((none.input_tokens, none.output_tokens), (0, 0));
        let neg = usage_of(&genai::chat::Usage {
            prompt_tokens: Some(-1),
            ..Default::default()
        });
        assert_eq!(neg.input_tokens, 0);
    }

    fn status_error(code: StatusCode) -> genai::Error {
        genai::Error::WebModelCall {
            model_iden: ModelIden::from_static(AdapterKind::OpenAI, "m"),
            webc_error: webc::Error::ResponseFailedStatus {
                status: code,
                body: "nope".into(),
                headers: Box::new(HeaderMap::new()),
            },
        }
    }

    #[test]
    fn rate_limits_timeouts_and_server_errors_are_transient() {
        for code in [
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(
                matches!(classify(status_error(code)), ChatError::Transient(_)),
                "{code}"
            );
        }
    }

    #[test]
    fn client_errors_are_fatal_and_keep_the_status_in_the_message() {
        for code in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ] {
            match classify(status_error(code)) {
                ChatError::Fatal(m) => assert!(m.contains(code.as_str()), "{m}"),
                other => panic!("{code}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_missing_api_key_is_fatal() {
        let e = genai::Error::RequiresApiKey {
            model_iden: ModelIden::from_static(AdapterKind::OpenAI, "m"),
        };
        assert!(matches!(classify(e), ChatError::Fatal(_)));
    }

    fn completion(message: Value) -> Value {
        json!({"id": "chatcmpl-1", "object": "chat.completion", "created": 1, "model": "m",
               "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
               "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}})
    }

    async fn model(server: &MockServer) -> GenaiChatModel {
        GenaiChatModel::openai_compat(&format!("{}/v1", server.uri()), "m", None)
    }

    fn request() -> ChatRequest {
        ChatRequest {
            system: "sys".into(),
            messages: vec![
                Message::User("go".into()),
                Message::Assistant {
                    text: None,
                    tool_calls: vec![call("call_0", None)],
                },
                Message::ToolResults(vec![ToolResult {
                    call_id: "call_0".into(),
                    content: "{\"ok\":true}".into(),
                    is_error: false,
                }]),
            ],
            tools: vec![spec()],
        }
    }

    #[tokio::test]
    async fn an_openai_compatible_endpoint_returns_tool_calls_and_usage() {
        let server = MockServer::start().await;
        let body = completion(
            json!({"role": "assistant", "content": null, "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_business", "arguments": "{\"x\":1}"}}]}),
        );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let resp = model(&server).await.complete(&request()).await.unwrap();
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(
            (
                resp.tool_calls[0].id.as_str(),
                resp.tool_calls[0].name.as_str()
            ),
            ("call_1", "get_business")
        );
        assert_eq!(resp.tool_calls[0].arguments, json!({"x": 1}));
        assert_eq!((resp.usage.input_tokens, resp.usage.output_tokens), (10, 5));
    }

    #[tokio::test]
    async fn the_wire_request_has_system_tools_and_the_tool_result_turn() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(completion(json!({"role": "assistant", "content": "done"}))),
            )
            .mount(&server)
            .await;
        let resp = model(&server).await.complete(&request()).await.unwrap();
        assert_eq!(resp.text.as_deref(), Some("done"));
        assert!(resp.tool_calls.is_empty());
        let received = server.received_requests().await.unwrap();
        let sent: Value = serde_json::from_slice(&received[0].body).unwrap();
        assert_eq!(sent["model"], "m");
        let roles: Vec<&str> = sent["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool"]);
        assert_eq!(sent["tools"][0]["function"]["name"], "get_business");
        assert_eq!(sent["messages"][3]["tool_call_id"], "call_0");
        assert!(
            sent["max_tokens"].is_number() || sent["max_completion_tokens"].is_number(),
            "output limit is set: {sent}"
        );
    }

    #[tokio::test]
    async fn http_errors_are_classified() {
        for (status, transient) in [
            (429, true),
            (500, true),
            (503, true),
            (401, false),
            (400, false),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(status).set_body_string("{\"error\":\"x\"}"))
                .mount(&server)
                .await;
            let err = model(&server).await.complete(&request()).await.unwrap_err();
            assert_eq!(
                matches!(err, ChatError::Transient(_)),
                transient,
                "{status}: {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_api_key_is_sent_as_a_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(completion(json!({"role": "assistant", "content": "ok"}))),
            )
            .mount(&server)
            .await;
        let m = GenaiChatModel::openai_compat(
            &format!("{}/v1", server.uri()),
            "m",
            Some("secret".into()),
        );
        m.complete(&request()).await.unwrap();
        let received = server.received_requests().await.unwrap();
        let auth = received[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(auth, "Bearer secret");
    }

    #[test]
    fn labels_name_the_provider_and_model() {
        assert_eq!(
            GenaiChatModel::openai_compat("http://x/v1", "llama", None).label(),
            "openai-compat/llama"
        );
        assert_eq!(
            GenaiChatModel::for_provider("anthropic", "claude-sonnet-5-5")
                .unwrap()
                .label(),
            "anthropic/claude-sonnet-5-5"
        );
    }

    #[test]
    fn provider_table_maps_names_namespaces_and_documented_env_vars() {
        assert_eq!(
            api_provider("anthropic"),
            Some(("anthropic", Some("ANTHROPIC_API_KEY")))
        );
        assert_eq!(
            api_provider("openrouter"),
            Some(("open_router", Some("OPENROUTER_API_KEY"))),
            "genai reads OPEN_ROUTER_API_KEY, mads must not"
        );
        assert_eq!(api_provider("ollama"), Some(("ollama", None)));
        assert_eq!(api_provider("claude-cli"), None);
        assert!(GenaiChatModel::for_provider("nope", "m").is_none());
    }
}
