use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{BusinessDraft, CatalogDraft, FetchedPage, InitState, SiteFetch};
use crate::{
    google::Issue,
    input::normalize_url,
    tools::{ToolHost, ToolOutput, ToolSpec, parse_args, schema::schema_for},
};

const SITEMAP_PAGE_MAX: usize = 200;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FetchPageArgs {
    /// Absolute URL on the business website.
    url: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FetchSitemapArgs {
    /// Sitemap URL. Empty looks in robots.txt, then /sitemap.xml.
    #[serde(default)]
    url: String,
    /// Case-insensitive text the URL must contain. Empty for all.
    #[serde(default)]
    contains: String,
    #[serde(default)]
    offset: usize,
    /// Page size, at most 200.
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddItemsArgs {
    /// Catalog items found on the site. URLs must come from fetched pages or the sitemap.
    items: Vec<CatalogDraft>,
}

/// Tools for the `init` mission: read the website, then draft `business.toml` and `catalog.csv`.
pub struct InitTools {
    state: Arc<Mutex<InitState>>,
    site: Arc<dyn SiteFetch>,
    page_limit: usize,
    call_budget: usize,
    calls: AtomicUsize,
    pages: AtomicUsize,
    finished: AtomicBool,
    sitemap: Mutex<Option<(String, Vec<String>)>>,
}

impl InitTools {
    pub fn new(
        state: Arc<Mutex<InitState>>,
        site: Arc<dyn SiteFetch>,
        max_turns: usize,
        page_limit: usize,
    ) -> Self {
        Self {
            state,
            site,
            page_limit,
            call_budget: max_turns * 4,
            calls: AtomicUsize::new(0),
            pages: AtomicUsize::new(0),
            finished: AtomicBool::new(false),
            sitemap: Mutex::new(None),
        }
    }

    fn host_ok(&self, url: &str) -> bool {
        normalize_url(url).is_some() && self.site.is_same_site(url)
    }

    async fn fetch_page(&self, a: FetchPageArgs) -> ToolOutput {
        if !self.host_ok(&a.url) {
            return ToolOutput::fail("HOST", format!("{} is not on the business website", a.url));
        }
        if self.pages.fetch_add(1, Ordering::SeqCst) >= self.page_limit {
            self.pages.fetch_sub(1, Ordering::SeqCst);
            return ToolOutput::fail(
                "LIMIT",
                format!("at most {} pages can be fetched", self.page_limit),
            );
        }
        let page: FetchedPage = match self.site.fetch_page(&a.url).await {
            Ok(p) => p,
            Err(e) => return ToolOutput::fail("FETCH", e),
        };
        let mut state = self.state.lock().await;
        state.note_seen(&a.url);
        state.note_seen(&page.url);
        page.links.iter().for_each(|l| state.note_seen(l));
        let summary = format!("fetch_page {} ({} links)", page.url, page.links.len());
        ToolOutput::ok(
            serde_json::to_value(&page).unwrap_or(Value::Null),
            &[],
            summary,
        )
    }

    async fn fetch_sitemap(&self, a: FetchSitemapArgs) -> ToolOutput {
        let requested = (!a.url.trim().is_empty()).then(|| a.url.trim().to_string());
        if let Some(u) = &requested
            && !self.host_ok(u)
        {
            return ToolOutput::fail("HOST", format!("{u} is not on the business website"));
        }
        let key = requested.clone().unwrap_or_default();
        let cached = self
            .sitemap
            .lock()
            .await
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| v.clone());
        let urls = match cached {
            Some(v) => v,
            None => match self.site.fetch_sitemap(requested.as_deref()).await {
                Ok(v) => {
                    *self.sitemap.lock().await = Some((key, v.clone()));
                    v
                }
                Err(e) => return ToolOutput::fail("FETCH", e),
            },
        };
        let needle = a.contains.trim().to_lowercase();
        let matching: Vec<&String> = urls
            .iter()
            .filter(|u| needle.is_empty() || u.to_lowercase().contains(&needle))
            .collect();
        let page: Vec<&String> = matching
            .iter()
            .skip(a.offset)
            .take(a.limit.clamp(1, SITEMAP_PAGE_MAX))
            .copied()
            .collect();
        let mut state = self.state.lock().await;
        page.iter().for_each(|u| state.note_seen(u));
        let summary = format!("fetch_sitemap {} of {} URLs", page.len(), matching.len());
        ToolOutput::ok(
            json!({"total": matching.len(), "offset": a.offset, "urls": page}),
            &[],
            summary,
        )
    }

    async fn write_business(&self, draft: BusinessDraft) -> ToolOutput {
        let name = draft.name.clone();
        match self.state.lock().await.set_business(draft) {
            Ok(()) => ToolOutput::ok(json!({"saved": true}), &[], format!("business {name}")),
            Err(issues) => ToolOutput::fail_issues(
                &issues,
                &[],
                format!("write_business: {} errors", issues.len()),
            ),
        }
    }

    async fn add_items(&self, a: AddItemsArgs) -> ToolOutput {
        let mut state = self.state.lock().await;
        match state.add_catalog(a.items) {
            Ok(added) => {
                let total = state.catalog_len();
                let summary = format!(
                    "catalog: +{} ({} skipped), {total} total",
                    added.added, added.skipped
                );
                ToolOutput::ok(
                    json!({"added": added.added, "skipped": added.skipped, "total": total}),
                    &[],
                    summary,
                )
            }
            Err(issues) => ToolOutput::fail_issues(
                &issues,
                &[],
                format!("add_catalog_items: {} errors", issues.len()),
            ),
        }
    }

    async fn finish_tool(&self) -> ToolOutput {
        let state = self.state.lock().await;
        if state.business().is_none() {
            let issue = Issue::error("E12", "business", "write_business was not called");
            return ToolOutput::fail_issues(&[issue], &[], "finish: no business");
        }
        self.finished.store(true, Ordering::SeqCst);
        ToolOutput::ok(
            json!({"catalog_items": state.catalog_len()}),
            &[],
            "init finished",
        )
    }
}

#[async_trait]
impl ToolHost for InitTools {
    fn specs(&self) -> Vec<ToolSpec> {
        let spec = |name: &str, description: &str, input_schema: Value| ToolSpec {
            name: name.into(),
            description: description.into(),
            input_schema,
        };
        vec![
            spec(
                "fetch_page",
                "Fetch one page of the business website: title, description, visible text and links. Start with the home page.",
                schema_for::<FetchPageArgs>(),
            ),
            spec(
                "fetch_sitemap",
                "List URLs from the sitemap, filtered and paginated. Use it to find the pages of catalog items.",
                schema_for::<FetchSitemapArgs>(),
            ),
            spec(
                "write_business",
                "Save the business profile. Replaces the previous one. Page URLs must come from fetched pages or the sitemap.",
                schema_for::<BusinessDraft>(),
            ),
            spec(
                "add_catalog_items",
                "Add catalog items (the entities people search for by name). Duplicates by URL are skipped.",
                schema_for::<AddItemsArgs>(),
            ),
            spec(
                "finish",
                "Finish the init mission. Needs write_business.",
                schema_for::<Empty>(),
            ),
        ]
    }

    async fn call(&self, name: &str, args: Value) -> ToolOutput {
        if self.calls.fetch_add(1, Ordering::SeqCst) + 1 > self.call_budget {
            return ToolOutput::fail("LIMIT", "tool call budget for this mission is exhausted");
        }
        macro_rules! with_args {
            ($t:ty, $f:expr) => {
                match parse_args::<$t>(args) {
                    Ok(a) => $f(a).await,
                    Err(e) => e,
                }
            };
        }
        match name {
            "fetch_page" => with_args!(FetchPageArgs, |a| self.fetch_page(a)),
            "fetch_sitemap" => with_args!(FetchSitemapArgs, |a| self.fetch_sitemap(a)),
            "write_business" => with_args!(BusinessDraft, |a| self.write_business(a)),
            "add_catalog_items" => with_args!(AddItemsArgs, |a| self.add_items(a)),
            "finish" => self.finish_tool().await,
            other => ToolOutput::fail(
                "UNKNOWN_TOOL",
                format!("'{other}' is not a tool of this mission"),
            ),
        }
    }

    fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;

    use super::*;
    use crate::{money::Cents, tools::ToolHost};

    struct FakeSite {
        pages: HashMap<String, FetchedPage>,
        sitemap: Vec<String>,
        calls: std::sync::atomic::AtomicUsize,
    }

    fn page(url: &str, links: &[&str]) -> FetchedPage {
        FetchedPage {
            url: url.into(),
            status: 200,
            title: "Vinellu".into(),
            description: "App de vinhos".into(),
            text: "App social de vinhos".into(),
            links: links.iter().map(|l| l.to_string()).collect(),
        }
    }

    #[async_trait]
    impl SiteFetch for FakeSite {
        fn is_same_site(&self, url: &str) -> bool {
            url.starts_with("https://vinellu.com")
        }
        async fn fetch_page(&self, url: &str) -> Result<FetchedPage, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| format!("HTTP 404 for {url}"))
        }
        async fn fetch_sitemap(&self, _url: Option<&str>) -> Result<Vec<String>, String> {
            if self.sitemap.is_empty() {
                Err("no sitemap".into())
            } else {
                Ok(self.sitemap.clone())
            }
        }
    }

    fn site() -> Arc<FakeSite> {
        let mut pages = HashMap::new();
        pages.insert(
            "https://vinellu.com".to_string(),
            page(
                "https://vinellu.com",
                &["https://vinellu.com/app", "https://vinellu.com/w/alamos"],
            ),
        );
        pages.insert(
            "https://vinellu.com/app".to_string(),
            page("https://vinellu.com/app", &[]),
        );
        let sitemap = (0..300)
            .map(|i| format!("https://vinellu.com/w/item-{i}"))
            .collect();
        Arc::new(FakeSite {
            pages,
            sitemap,
            calls: Default::default(),
        })
    }

    fn tools(site: Arc<FakeSite>) -> InitTools {
        let state = Arc::new(Mutex::new(InitState::new(
            "https://vinellu.com",
            Cents(5000),
            "BRL",
            5,
        )));
        InitTools::new(state, site, 40, 30)
    }

    fn codes(out: &ToolOutput) -> Vec<String> {
        out.content["errors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| e["code"].as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn business_args() -> Value {
        json!({"name": "Vinellu", "url": "https://vinellu.com", "language": "pt-BR", "locations": ["Brazil"], "goal": "cadastros no app",
               "description": "App social de vinhos com reviews, safras e harmonização.",
               "pages": [{"name": "app", "url": "https://vinellu.com/app"}]})
    }

    #[tokio::test]
    async fn toolset_is_the_documented_five() {
        let names: Vec<String> = tools(site()).specs().into_iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "fetch_page",
                "fetch_sitemap",
                "write_business",
                "add_catalog_items",
                "finish"
            ]
        );
    }

    #[tokio::test]
    async fn every_schema_is_portable() {
        for spec in tools(site()).specs() {
            let bad = crate::tools::schema::non_portable_keywords(&spec.input_schema);
            assert!(bad.is_empty(), "{}: {bad:?}", spec.name);
            assert_eq!(spec.input_schema["type"], "object");
        }
    }

    #[tokio::test]
    async fn fetching_a_page_returns_it_and_remembers_its_links() {
        let t = tools(site());
        let out = t
            .call("fetch_page", json!({"url": "https://vinellu.com"}))
            .await;
        assert!(!out.is_error, "{}", out.content);
        assert_eq!(out.content["result"]["title"], "Vinellu");
        let ok = t.call("write_business", business_args()).await;
        assert!(
            !ok.is_error,
            "the page link /app is now known: {}",
            ok.content
        );
    }

    #[tokio::test]
    async fn other_hosts_are_refused_without_a_request() {
        let s = site();
        let t = tools(s.clone());
        for url in [
            "https://evil.com/x",
            "http://169.254.169.254/latest",
            "file:///etc/passwd",
            "not a url",
        ] {
            let out = t.call("fetch_page", json!({"url": url})).await;
            assert!(out.is_error, "{url}");
            assert_eq!(codes(&out), ["HOST"], "{url}");
        }
        assert_eq!(s.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_errors_come_back_as_tool_errors() {
        let out = tools(site())
            .call("fetch_page", json!({"url": "https://vinellu.com/missing"}))
            .await;
        assert_eq!(codes(&out), ["FETCH"]);
        assert!(
            out.content["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains("404")
        );
    }

    #[tokio::test]
    async fn page_fetches_are_capped() {
        let state = Arc::new(Mutex::new(InitState::new(
            "https://vinellu.com",
            Cents(5000),
            "BRL",
            5,
        )));
        let t = InitTools::new(state, site(), 40, 2);
        for _ in 0..2 {
            assert!(
                !t.call("fetch_page", json!({"url": "https://vinellu.com"}))
                    .await
                    .is_error
            );
        }
        assert_eq!(
            codes(
                &t.call("fetch_page", json!({"url": "https://vinellu.com"}))
                    .await
            ),
            ["LIMIT"]
        );
    }

    #[tokio::test]
    async fn the_sitemap_is_paginated_filtered_and_its_urls_become_known() {
        let t = tools(site());
        let first = t.call("fetch_sitemap", json!({"limit": 5})).await;
        assert_eq!(first.content["result"]["total"], 300);
        assert_eq!(first.content["result"]["urls"].as_array().unwrap().len(), 5);
        let filtered = t
            .call(
                "fetch_sitemap",
                json!({"contains": "item-29", "limit": 200}),
            )
            .await;
        assert_eq!(
            filtered.content["result"]["total"], 11,
            "item-29 and item-290..299"
        );
        let capped = t.call("fetch_sitemap", json!({"limit": 5000})).await;
        assert_eq!(
            capped.content["result"]["urls"].as_array().unwrap().len(),
            200,
            "page size is capped"
        );
        let listed = "https://vinellu.com/w/item-3";
        let unseen = "https://vinellu.com/w/item-250";
        let ok = t
            .call(
                "add_catalog_items",
                json!({"items": [{"name": "Item", "url": listed}]}),
            )
            .await;
        assert!(!ok.is_error, "{}", ok.content);
        let bad = t
            .call(
                "add_catalog_items",
                json!({"items": [{"name": "Item 250", "url": unseen}]}),
            )
            .await;
        assert!(
            codes(&bad).contains(&"E07".to_string()),
            "only urls actually returned count as seen: {}",
            bad.content
        );
    }

    #[tokio::test]
    async fn a_sitemap_of_another_host_is_refused() {
        let out = tools(site())
            .call(
                "fetch_sitemap",
                json!({"url": "https://evil.com/sitemap.xml"}),
            )
            .await;
        assert_eq!(codes(&out), ["HOST"]);
    }

    #[tokio::test]
    async fn write_business_validates_and_reports_the_toml_key() {
        let t = tools(site());
        let mut bad = business_args();
        bad["locations"] = json!(["Brazil", "Chile"]);
        let out = t.call("write_business", bad).await;
        assert!(out.is_error);
        let paths: Vec<&str> = out.content["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["path"].as_str())
            .collect();
        assert!(paths.contains(&"business.locations"), "{paths:?}");
    }

    #[tokio::test]
    async fn catalog_items_are_added_deduplicated_and_limited() {
        let t = tools(site());
        t.call("fetch_sitemap", json!({"limit": 200})).await;
        let items: Vec<Value> = (0..3).map(|i| json!({"name": format!("Item {i}"), "url": format!("https://vinellu.com/w/item-{i}")})).collect();
        let out = t.call("add_catalog_items", json!({"items": items})).await;
        assert_eq!(
            (
                out.content["result"]["added"].as_u64(),
                out.content["result"]["total"].as_u64()
            ),
            (Some(3), Some(3))
        );
        let again = t
            .call(
                "add_catalog_items",
                json!({"items": [{"name": "Item 0 again", "url": "https://vinellu.com/w/item-0"}]}),
            )
            .await;
        assert_eq!(again.content["result"]["skipped"], 1);
        let too_many: Vec<Value> = (3..10).map(|i| json!({"name": format!("Item {i}"), "url": format!("https://vinellu.com/w/item-{i}")})).collect();
        let over = t
            .call("add_catalog_items", json!({"items": too_many}))
            .await;
        assert!(codes(&over).contains(&"E13".to_string()));
    }

    #[tokio::test]
    async fn finish_needs_a_business() {
        let t = tools(site());
        assert!(t.call("finish", json!({})).await.is_error);
        assert!(!t.finished());
        t.call("fetch_page", json!({"url": "https://vinellu.com"}))
            .await;
        t.call("write_business", business_args()).await;
        assert!(!t.call("finish", json!({})).await.is_error);
        assert!(t.finished());
    }

    #[tokio::test]
    async fn unknown_tools_and_bad_arguments_are_refused() {
        let t = tools(site());
        assert_eq!(codes(&t.call("shell", json!({})).await), ["UNKNOWN_TOOL"]);
        assert_eq!(
            codes(
                &t.call("fetch_page", json!({"url": "https://vinellu.com", "x": 1}))
                    .await
            ),
            ["ARGS"]
        );
    }

    #[tokio::test]
    async fn the_call_budget_is_enforced() {
        let state = Arc::new(Mutex::new(InitState::new(
            "https://vinellu.com",
            Cents(5000),
            "BRL",
            5,
        )));
        let t = InitTools::new(state, site(), 1, 30);
        for _ in 0..4 {
            assert!(!t.call("fetch_sitemap", json!({})).await.is_error);
        }
        assert_eq!(codes(&t.call("fetch_sitemap", json!({})).await), ["LIMIT"]);
    }
}
