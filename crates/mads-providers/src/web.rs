use std::time::Duration;

use async_trait::async_trait;
use mads_core::web::Web;
use reqwest::{Client, redirect::Policy};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REDIRECTS: usize = 5;

/// HTTP access for URL checks.
pub struct WebClient {
    client: Client,
}

impl WebClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_timeout(DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent(concat!("mads/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .redirect(Policy::limited(MAX_REDIRECTS))
            .build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Web for WebClient {
    /// GET, redirects followed, the body is never read.
    async fn check_url(&self, url: &str) -> Result<u16, String> {
        match self.client.get(url).send().await {
            Ok(resp) => Ok(resp.status().as_u16()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// GET a 2xx body, refusing anything over `limit` bytes before and while reading it.
    async fn fetch_bytes(&self, url: &str, limit: usize) -> Result<Vec<u8>, String> {
        let mut resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("{url}: HTTP {}", status.as_u16()));
        }
        if resp.content_length().is_some_and(|n| n > limit as u64) {
            return Err(format!("{url}: larger than {limit} bytes"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
            body.extend_from_slice(&chunk);
            if body.len() > limit {
                return Err(format!("{url}: larger than {limit} bytes"));
            }
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    async fn status(server: &MockServer, p: &str) -> Result<u16, String> {
        WebClient::new()
            .unwrap()
            .check_url(&format!("{}{p}", server.uri()))
            .await
    }

    #[tokio::test]
    async fn reports_the_status_code() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/gone"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/boom"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert_eq!(status(&server, "/ok").await, Ok(200));
        assert_eq!(status(&server, "/gone").await, Ok(404));
        assert_eq!(status(&server, "/boom").await, Ok(503));
    }

    #[tokio::test]
    async fn follows_redirects() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/old"))
            .respond_with(ResponseTemplate::new(301).insert_header("location", "/new"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/new"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        assert_eq!(status(&server, "/old").await, Ok(200));
    }

    #[tokio::test]
    async fn gives_up_after_five_redirects() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/loop"))
            .mount(&server)
            .await;
        assert!(status(&server, "/loop").await.is_err());
    }

    #[tokio::test]
    async fn fetch_bytes_reads_a_body_within_the_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/photo.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 100]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/gone.jpg"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let web = WebClient::new().unwrap();
        let ok = web
            .fetch_bytes(&format!("{}/photo.jpg", server.uri()), 1000)
            .await;
        assert_eq!(ok, Ok(vec![7u8; 100]));
        let big = web
            .fetch_bytes(&format!("{}/photo.jpg", server.uri()), 10)
            .await;
        assert!(big.unwrap_err().contains("larger than 10 bytes"));
        let gone = web
            .fetch_bytes(&format!("{}/gone.jpg", server.uri()), 1000)
            .await;
        assert!(gone.unwrap_err().contains("HTTP 404"));
    }

    #[tokio::test]
    async fn a_refused_connection_is_an_error() {
        let err = WebClient::new()
            .unwrap()
            .check_url("http://127.0.0.1:1/x")
            .await
            .unwrap_err();
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn a_slow_server_hits_the_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
            .mount(&server)
            .await;
        let client = WebClient::with_timeout(Duration::from_millis(100)).unwrap();
        assert!(
            client
                .check_url(&format!("{}/slow", server.uri()))
                .await
                .is_err()
        );
    }
}
