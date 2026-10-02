use async_trait::async_trait;

/// Network access mads needs. Implemented in `mads-providers`, faked in tests.
#[async_trait]
pub trait Web: Send + Sync {
    /// HTTP status after following redirects, or a network error message.
    async fn check_url(&self, url: &str) -> Result<u16, String>;

    /// Body of a 2xx response of at most `limit` bytes. Used for reference photos.
    async fn fetch_bytes(&self, url: &str, _limit: usize) -> Result<Vec<u8>, String> {
        Err(format!("cannot download {url}: no HTTP client"))
    }
}
