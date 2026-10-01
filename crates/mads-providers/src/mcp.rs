use std::{io, sync::Arc};

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use mads_core::tools::ToolHost;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

/// Exposes one mission's tools to an agent CLI over MCP, on a loopback port that only answers
/// requests carrying this endpoint's bearer token. Dropping it closes the port.
pub struct McpEndpoint {
    url: String,
    token: String,
    shutdown: CancellationToken,
}

impl McpEndpoint {
    pub async fn start(tools: Arc<dyn ToolHost>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let token = new_token();
        let shutdown = CancellationToken::new();

        let config =
            StreamableHttpServerConfig::default().with_cancellation_token(shutdown.child_token());
        let service = StreamableHttpService::new(
            move || {
                Ok(ToolsHandler {
                    tools: tools.clone(),
                })
            },
            Arc::new(LocalSessionManager::default()),
            config,
        );
        let app =
            Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn_with_state(
                    Arc::<str>::from(token.as_str()),
                    require_bearer,
                ));

        let stop = shutdown.clone();
        tokio::spawn(async move {
            let server = axum::serve(listener, app)
                .with_graceful_shutdown(async move { stop.cancelled().await });
            let _ = server.await;
        });
        Ok(Self {
            url: format!("http://127.0.0.1:{port}/mcp"),
            token,
            shutdown,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

impl Drop for McpEndpoint {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

/// 64 hex characters from two random UUIDs.
fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn require_bearer(State(token): State<Arc<str>>, req: Request, next: Next) -> Response {
    let provided = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match provided {
        Some(t) if constant_time_eq(t.as_bytes(), token.as_bytes()) => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response(),
    }
}

#[derive(Clone)]
struct ToolsHandler {
    tools: Arc<dyn ToolHost>,
}

impl ServerHandler for ToolsHandler {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Tools of one mads mission. Call them to do the work, then call finish.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self
            .tools
            .specs()
            .into_iter()
            .map(|s| {
                let schema = s.input_schema.as_object().cloned().unwrap_or_default();
                Tool::new(s.name, s.description, Arc::new(schema))
            })
            .collect();
        // Claude Code negotiates protocol 2026-07-28, which rejects a list without cache hints.
        // The tools of a mission never change, but a zero TTL keeps the client from caching across missions.
        Ok(ListToolsResult::with_all_items(tools)
            .with_ttl_ms(0)
            .with_cache_scope(CacheScope::Private))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = Value::Object(request.arguments.unwrap_or_default());
        let out = self.tools.call(&request.name, args).await;
        let content = vec![ContentBlock::text(out.text())];
        let result = if out.is_error {
            CallToolResult::error(content)
        } else {
            CallToolResult::success(content)
        };
        Ok(CallToolResponse::Complete(result))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use async_trait::async_trait;
    use mads_core::tools::{ToolHost, ToolOutput, ToolSpec};
    use reqwest::StatusCode;
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    use serde_json::{Value, json};

    use super::*;

    struct FakeHost {
        finished: AtomicBool,
    }

    impl FakeHost {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                finished: AtomicBool::new(false),
            })
        }
    }

    #[async_trait]
    impl ToolHost for FakeHost {
        fn specs(&self) -> Vec<ToolSpec> {
            vec![
                ToolSpec {
                    name: "echo".into(),
                    description: "Echo the arguments".into(),
                    input_schema: json!({"type": "object", "properties": {"text": {"type": "string"}}}),
                },
                ToolSpec {
                    name: "finish".into(),
                    description: "Finish".into(),
                    input_schema: json!({"type": "object"}),
                },
            ]
        }

        async fn call(&self, name: &str, args: Value) -> ToolOutput {
            match name {
                "echo" => ToolOutput::ok(json!({"echo": args}), &[], "echo"),
                "finish" => {
                    self.finished.store(true, Ordering::SeqCst);
                    ToolOutput::ok(json!({}), &[], "finish")
                }
                other => ToolOutput::fail("UNKNOWN_TOOL", other),
            }
        }

        fn finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }
    }

    const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;

    async fn post(url: &str, auth: Option<&str>, host: Option<&str>) -> StatusCode {
        let mut req = reqwest::Client::new()
            .post(url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(INITIALIZE);
        if let Some(a) = auth {
            req = req.header("authorization", a);
        }
        if let Some(h) = host {
            req = req.header("host", h);
        }
        req.send().await.unwrap().status()
    }

    async fn start() -> (McpEndpoint, Arc<FakeHost>) {
        let host = FakeHost::new();
        (McpEndpoint::start(host.clone()).await.unwrap(), host)
    }

    #[tokio::test]
    async fn endpoint_listens_on_loopback_with_a_long_random_token() {
        let (a, _) = start().await;
        let (b, _) = start().await;
        assert!(
            a.url().starts_with("http://127.0.0.1:") && a.url().ends_with("/mcp"),
            "{}",
            a.url()
        );
        assert_eq!(a.token().len(), 64);
        assert!(a.token().chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a.token(), b.token());
        assert_ne!(a.url(), b.url());
    }

    #[tokio::test]
    async fn requests_without_a_valid_bearer_token_are_rejected() {
        let (e, _) = start().await;
        let good = format!("Bearer {}", e.token());
        let cases: Vec<(&str, Option<String>)> = vec![
            ("no header", None),
            ("scheme only", Some("Bearer".into())),
            ("empty token", Some("Bearer ".into())),
            ("basic scheme", Some(format!("Basic {}", e.token()))),
            ("lowercase scheme", Some(format!("bearer {}", e.token()))),
            ("raw token", Some(e.token().to_string())),
            ("wrong token", Some(format!("Bearer {}", "0".repeat(64)))),
            (
                "space inside the token",
                Some(format!("Bearer {} {}", &e.token()[..32], &e.token()[32..])),
            ),
            (
                "truncated token",
                Some(format!("Bearer {}", &e.token()[..63])),
            ),
            ("extra token", Some(format!("{good}x"))),
        ];
        for (label, header) in cases {
            let status = post(e.url(), header.as_deref(), None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{label}");
        }
    }

    #[tokio::test]
    async fn a_token_from_another_endpoint_is_rejected() {
        let (a, _) = start().await;
        let (b, _) = start().await;
        assert_eq!(
            post(a.url(), Some(&format!("Bearer {}", b.token())), None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn the_token_in_the_query_string_is_not_enough() {
        let (e, _) = start().await;
        let url = format!("{}?token={}", e.url(), e.token());
        assert_eq!(post(&url, None, None).await, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn other_paths_need_the_token_too() {
        let (e, _) = start().await;
        let base = e.url().trim_end_matches("/mcp").to_string();
        for path in ["/", "/admin", "/mcp/../admin"] {
            let status = reqwest::Client::new()
                .get(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .status();
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        }
    }

    #[tokio::test]
    async fn a_forged_host_header_is_refused_even_with_the_right_token() {
        let (e, _) = start().await;
        let status = post(
            e.url(),
            Some(&format!("Bearer {}", e.token())),
            Some("evil.example"),
        )
        .await;
        assert!(status.is_client_error(), "{status}");
        assert_ne!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_valid_token_reaches_the_mcp_service() {
        let (e, _) = start().await;
        let status = post(e.url(), Some(&format!("Bearer {}", e.token())), None).await;
        assert!(status.is_success(), "{status}");
    }

    async fn client(e: &McpEndpoint) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
        let config = StreamableHttpClientTransportConfig::with_uri(e.url().to_string())
            .auth_header(e.token().to_string());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        ().serve(transport).await.expect("mcp handshake")
    }

    fn object(v: Value) -> serde_json::Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[tokio::test]
    async fn an_mcp_client_lists_the_tools_with_their_schemas() {
        let (e, _) = start().await;
        let c = client(&e).await;
        let tools = c.list_all_tools().await.unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names, ["echo", "finish"]);
        assert_eq!(tools[0].description.as_deref(), Some("Echo the arguments"));
        assert_eq!(tools[0].input_schema.get("type"), Some(&json!("object")));
    }

    #[tokio::test]
    async fn calling_a_tool_runs_it_on_the_host_and_returns_its_json() {
        let (e, host) = start().await;
        let c = client(&e).await;
        let out = c
            .call_tool(
                CallToolRequestParams::new("echo").with_arguments(object(json!({"text": "hi"}))),
            )
            .await
            .unwrap();
        assert_ne!(out.is_error, Some(true));
        let text = out.content[0].as_text().unwrap().text.clone();
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["result"]["echo"]["text"], "hi");
        assert!(!host.finished());
        c.call_tool(CallToolRequestParams::new("finish"))
            .await
            .unwrap();
        assert!(host.finished());
    }

    #[tokio::test]
    async fn tool_errors_come_back_flagged_as_errors() {
        let (e, _) = start().await;
        let c = client(&e).await;
        let out = c
            .call_tool(CallToolRequestParams::new("nope"))
            .await
            .unwrap();
        assert_eq!(out.is_error, Some(true));
        assert!(
            out.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("UNKNOWN_TOOL")
        );
    }

    #[tokio::test]
    async fn dropping_the_endpoint_closes_the_port() {
        let (e, _) = start().await;
        let url = e.url().to_string();
        drop(e);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let res = reqwest::Client::new()
            .post(&url)
            .body(INITIALIZE)
            .send()
            .await;
        assert!(res.is_err(), "the listener should be gone");
    }

    /// Claude Code speaks protocol 2026-07-28, which rejects a tool list without cache hints.
    /// The rmcp client is lenient about this, so the shape is checked on the raw JSON.
    #[tokio::test]
    async fn the_modern_protocol_tool_list_carries_cache_hints() {
        let (e, _) = start().await;
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{},"io.modelcontextprotocol/clientInfo":{"name":"t","version":"0"}}}}"#;
        let text = reqwest::Client::new()
            .post(e.url())
            .header("authorization", format!("Bearer {}", e.token()))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2026-07-28")
            .header("mcp-method", "tools/list")
            .body(body)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let data = text
            .lines()
            .find_map(|l| l.strip_prefix("data: ").filter(|d| d.starts_with('{')))
            .expect("a data line");
        let v: Value = serde_json::from_str(data).unwrap();
        let result = &v["result"];
        assert!(result["ttlMs"].is_u64(), "ttlMs is required: {result}");
        assert!(
            matches!(result["cacheScope"].as_str(), Some("public" | "private")),
            "cacheScope is required: {result}"
        );
        assert_eq!(result["tools"][0]["name"], "echo");
    }
}
