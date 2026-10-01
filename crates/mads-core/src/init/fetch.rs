use async_trait::async_trait;
use serde::Serialize;

/// What an agent sees of a page: enough to understand the business, not the markup.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FetchedPage {
    pub url: String,
    pub status: u16,
    pub title: String,
    pub description: String,
    /// Visible text, at most 8000 characters.
    pub text: String,
    /// Absolute same-site links, at most 200.
    pub links: Vec<String>,
}

/// The URLs of a sitemap and the sitemap files that could not be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SitemapUrls {
    pub urls: Vec<String>,
    /// `url: reason` for every sitemap file that was skipped. A page missing from `urls` may be in one.
    pub skipped: Vec<String>,
}

/// Read-only access to one website. Implemented in `mads-providers`, faked in tests.
#[async_trait]
pub trait SiteFetch: Send + Sync {
    /// True for the start URL's host and its `www.` or apex sibling.
    fn is_same_site(&self, url: &str) -> bool;
    async fn fetch_page(&self, url: &str) -> Result<FetchedPage, String>;
    /// Every URL of the sitemap, indexes followed. `None` looks in robots.txt, then `/sitemap.xml`.
    async fn fetch_sitemap(&self, url: Option<&str>) -> Result<SitemapUrls, String>;
}
