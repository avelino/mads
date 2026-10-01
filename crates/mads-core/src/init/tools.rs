use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{
    BusinessDraft, CatalogDraft, FetchedPage, InitState, ResearchDraft, SiteFetch, SitemapUrls,
};
use crate::{
    google::Issue,
    input::{normalize_url, slugify},
    tools::{ToolHost, ToolOutput, ToolSpec, parse_args, schema::schema_for},
};

const SITEMAP_PAGE_MAX: usize = 200;
const SEARCH_URLS_MAX: usize = 20;
const SEARCH_NAMES_MAX: usize = 50;

/// `(path slug, url)` for every sitemap URL.
type SlugIndex = Arc<Vec<(String, String)>>;

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
struct SearchSiteArgs {
    /// The names to look for, as people write them. Accents and case do not matter. 1 to 50 names.
    names: Vec<String>,
    /// URLs returned per name, at most 20.
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize {
    5
}

fn name_list_issues(names: &[String]) -> Option<Vec<Issue>> {
    if names.is_empty() || names.len() > SEARCH_NAMES_MAX {
        let msg = format!("needs 1 to {SEARCH_NAMES_MAX} names, got {}", names.len());
        return Some(vec![Issue::error("QUERY", "names", msg)]);
    }
    let issues: Vec<Issue> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| slugify(n).is_empty())
        .map(|(i, _)| Issue::error("QUERY", format!("names[{i}]"), "no letters or digits"))
        .collect();
    (!issues.is_empty()).then_some(issues)
}

/// URLs whose path slug holds every word of `name`, the shortest path first.
fn matching_urls<'a>(index: &'a [(String, String)], name: &str) -> Vec<&'a String> {
    let slug = slugify(name);
    let words: Vec<&str> = slug.split('-').collect();
    let mut hits: Vec<(usize, &String)> = index
        .iter()
        .filter(|(s, _)| words.iter().all(|w| s.contains(w)))
        .map(|(s, u)| (s.len(), u))
        .collect();
    hits.sort_by_key(|(len, _)| *len);
    hits.into_iter().map(|(_, u)| u).collect()
}

/// `%C3%A9` back to `é`, so an encoded slug matches the name. Bad escapes stay as they are.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The slug of everything after the host: what a page name looks like inside its URL.
fn path_slug(url: &str) -> String {
    let path = url::Url::parse(url)
        .map(|u| format!("{}?{}", u.path(), u.query().unwrap_or_default()))
        .unwrap_or_default();
    slugify(&percent_decode(&path))
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
    sitemap: Mutex<Option<(String, Arc<SitemapUrls>)>>,
    /// The default sitemap's slugs, built on the first `search_site`.
    slugs: Mutex<Option<SlugIndex>>,
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
            slugs: Mutex::new(None),
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

    /// The sitemap's URLs, fetched once per sitemap and kept for the next calls.
    async fn sitemap_urls(&self, requested: Option<&str>) -> Result<Arc<SitemapUrls>, String> {
        let key = requested.unwrap_or_default().to_string();
        let cached = self
            .sitemap
            .lock()
            .await
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| v.clone());
        if let Some(v) = cached {
            return Ok(v);
        }
        let v = Arc::new(self.site.fetch_sitemap(requested).await?);
        *self.sitemap.lock().await = Some((key, v.clone()));
        Ok(v)
    }

    async fn fetch_sitemap(&self, a: FetchSitemapArgs) -> ToolOutput {
        let requested = (!a.url.trim().is_empty()).then(|| a.url.trim().to_string());
        if let Some(u) = &requested
            && !self.host_ok(u)
        {
            return ToolOutput::fail("HOST", format!("{u} is not on the business website"));
        }
        let read = match self.sitemap_urls(requested.as_deref()).await {
            Ok(v) => v,
            Err(e) => return ToolOutput::fail("FETCH", e),
        };
        let needle = a.contains.trim().to_lowercase();
        let matching: Vec<&String> = read
            .urls
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
            json!({"total": matching.len(), "offset": a.offset, "urls": page, "skipped": read.skipped}),
            &[],
            summary,
        )
    }

    /// Slugs of the default sitemap, computed once: a large site has hundreds of thousands of URLs.
    async fn slug_index(&self) -> Result<SlugIndex, String> {
        if let Some(index) = self.slugs.lock().await.clone() {
            return Ok(index);
        }
        let read = self.sitemap_urls(None).await?;
        let index: SlugIndex = Arc::new(
            read.urls
                .iter()
                .map(|u| (path_slug(u), u.clone()))
                .collect(),
        );
        *self.slugs.lock().await = Some(index.clone());
        Ok(index)
    }

    /// Finds the pages of each name in the sitemap: every word of a name must be in the URL path.
    async fn search_site(&self, a: SearchSiteArgs) -> ToolOutput {
        if let Some(issues) = name_list_issues(&a.names) {
            return ToolOutput::fail_issues(&issues, &[], "search_site: bad names");
        }
        let index = match self.slug_index().await {
            Ok(v) => v,
            Err(e) => return ToolOutput::fail("FETCH", e),
        };
        let skipped = match self.sitemap_urls(None).await {
            Ok(read) => read.skipped.clone(),
            Err(e) => return ToolOutput::fail("FETCH", e),
        };
        let per_name = a.limit.clamp(1, SEARCH_URLS_MAX);
        let mut state = self.state.lock().await;
        let results: Vec<Value> = a
            .names
            .iter()
            .map(|name| {
                let hits = matching_urls(&index, name);
                let urls: Vec<&String> = hits.iter().take(per_name).copied().collect();
                urls.iter().for_each(|u| state.note_seen(u));
                json!({"name": name, "total": hits.len(), "urls": urls})
            })
            .collect();
        let found = results.iter().filter(|r| r["total"] != 0).count();
        let summary = format!("search_site: {found} of {} names found", a.names.len());
        ToolOutput::ok(
            json!({"checked": a.names.len(), "found": found, "results": results, "skipped": skipped}),
            &[],
            summary,
        )
    }

    async fn write_research(&self, draft: ResearchDraft) -> ToolOutput {
        let n = draft.opportunities.len();
        match self.state.lock().await.set_research(draft) {
            Ok(()) => ToolOutput::ok(
                json!({"saved": true}),
                &[],
                format!("research: {n} opportunities"),
            ),
            Err(issues) => ToolOutput::fail_issues(
                &issues,
                &[],
                format!("write_research: {} errors", issues.len()),
            ),
        }
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
        if state.research().is_none() {
            let issue = Issue::error("E12", "research", "write_research was not called");
            return ToolOutput::fail_issues(&[issue], &[], "finish: no research");
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
                "search_site",
                "Find the pages of up to 50 names on the business website, from its sitemap. Every word of a name must appear in the URL. Use it to turn names found on the web into pages of the site, and to count how many of them the site has.",
                schema_for::<SearchSiteArgs>(),
            ),
            spec(
                "write_business",
                "Save the business profile. Replaces the previous one. Page URLs must come from fetched pages or the sitemap.",
                schema_for::<BusinessDraft>(),
            ),
            spec(
                "write_research",
                "Save what you learned: how the business works, the campaign opportunities ranked by expected return, and the open questions. Replaces the previous research.",
                schema_for::<ResearchDraft>(),
            ),
            spec(
                "add_catalog_items",
                "Add catalog items (the entities people search for by name). Duplicates by URL are skipped.",
                schema_for::<AddItemsArgs>(),
            ),
            spec(
                "finish",
                "Finish the init mission. Needs write_business and write_research.",
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
            "search_site" => with_args!(SearchSiteArgs, |a| self.search_site(a)),
            "write_research" => with_args!(ResearchDraft, |a| self.write_research(a)),
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
        skipped: Vec<String>,
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
        async fn fetch_sitemap(&self, _url: Option<&str>) -> Result<SitemapUrls, String> {
            if self.sitemap.is_empty() {
                Err("no sitemap".into())
            } else {
                Ok(SitemapUrls {
                    urls: self.sitemap.clone(),
                    skipped: self.skipped.clone(),
                })
            }
        }
    }

    fn site_with(sitemap: &[&str]) -> Arc<FakeSite> {
        let mut pages = HashMap::new();
        pages.insert(
            "https://vinellu.com".to_string(),
            page("https://vinellu.com", &[]),
        );
        Arc::new(FakeSite {
            pages,
            sitemap: sitemap.iter().map(|u| u.to_string()).collect(),
            skipped: vec![],
            calls: Default::default(),
        })
    }

    fn research_args() -> Value {
        json!({"summary": "A social app for wine lovers. People rate labels and follow friends.",
               "opportunities": [{"name": "Labels by name", "intent": "catalog", "searches": ["alamos malbec"],
                                  "demand": "high", "competition": "low", "evidence": "Bestsellers get searched by name.",
                                  "sources": ["https://example.com/ranking"]}]})
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
            skipped: vec![],
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
    async fn toolset_is_the_documented_seven() {
        let names: Vec<String> = tools(site()).specs().into_iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "fetch_page",
                "fetch_sitemap",
                "search_site",
                "write_business",
                "write_research",
                "add_catalog_items",
                "finish"
            ]
        );
    }

    #[tokio::test]
    async fn search_site_checks_many_names_whatever_the_accents_and_case() {
        let t = tools(site_with(&[
            "https://vinellu.com/w/a1/alamos-malbec-2022",
            "https://vinellu.com/w/b2/alamos-malbec",
            "https://vinellu.com/w/c3/catena-malbec",
            "https://vinellu.com/w/d4/carm%C3%A9n%C3%A8re-reserva",
        ]));
        let out = t
            .call(
                "search_site",
                json!({"names": ["Álamos MALBEC", "Carménère", "Nope Nothing"]}),
            )
            .await;
        assert!(!out.is_error, "{}", out.content);
        let r = &out.content["result"];
        assert_eq!(
            (r["checked"].as_u64(), r["found"].as_u64()),
            (Some(3), Some(2))
        );
        assert_eq!(r["results"][0]["name"], "Álamos MALBEC");
        assert_eq!(r["results"][0]["total"], 2);
        assert_eq!(
            r["results"][0]["urls"],
            json!([
                "https://vinellu.com/w/b2/alamos-malbec",
                "https://vinellu.com/w/a1/alamos-malbec-2022"
            ]),
            "the tighter match comes first"
        );
        assert_eq!(r["results"][1]["total"], 1, "an encoded slug matches");
        assert_eq!(r["results"][2]["total"], 0);
    }

    #[tokio::test]
    async fn search_site_results_become_known_urls() {
        let t = tools(site_with(&["https://vinellu.com/w/b2/alamos-malbec"]));
        let add = json!({"items": [{"name": "Alamos Malbec", "url": "https://vinellu.com/w/b2/alamos-malbec"}]});
        assert_eq!(
            codes(&t.call("add_catalog_items", add.clone()).await),
            ["E07"]
        );
        t.call("search_site", json!({"names": ["alamos"]})).await;
        let after = t.call("add_catalog_items", add).await;
        assert!(!after.is_error, "{}", after.content);
    }

    #[tokio::test]
    async fn search_site_caps_the_urls_per_name() {
        let t = tools(site());
        let out = t
            .call("search_site", json!({"names": ["item"], "limit": 500}))
            .await;
        assert_eq!(out.content["result"]["results"][0]["total"], 300);
        assert_eq!(
            out.content["result"]["results"][0]["urls"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
    }

    #[tokio::test]
    async fn search_site_refuses_bad_name_lists_and_changes_nothing() {
        let t = tools(site_with(&["https://vinellu.com/w/b2/alamos-malbec"]));
        let blank = t
            .call("search_site", json!({"names": ["alamos", " -- "]}))
            .await;
        assert_eq!(codes(&blank), ["QUERY"]);
        assert_eq!(blank.content["errors"][0]["path"], "names[1]");
        let add =
            json!({"items": [{"name": "Alamos", "url": "https://vinellu.com/w/b2/alamos-malbec"}]});
        assert_eq!(
            codes(&t.call("add_catalog_items", add).await),
            ["E07"],
            "a failed call marks nothing as seen"
        );
        assert_eq!(
            codes(&t.call("search_site", json!({"names": []})).await),
            ["QUERY"]
        );
        let many: Vec<String> = (0..51).map(|i| format!("name {i}")).collect();
        assert_eq!(
            codes(&t.call("search_site", json!({"names": many})).await),
            ["QUERY"]
        );
    }

    #[tokio::test]
    async fn sitemaps_that_could_not_be_read_are_shown_to_the_agent() {
        let mut fake = Arc::try_unwrap(site_with(&["https://vinellu.com/w/b2/alamos-malbec"]))
            .ok()
            .unwrap();
        fake.skipped =
            vec!["https://vinellu.com/sitemap-items-1.xml: response is too large".into()];
        let t = tools(Arc::new(fake));
        let listed = t.call("fetch_sitemap", json!({})).await;
        assert_eq!(
            listed.content["result"]["skipped"][0],
            "https://vinellu.com/sitemap-items-1.xml: response is too large"
        );
        let searched = t.call("search_site", json!({"names": ["nope"]})).await;
        assert_eq!(
            searched.content["result"]["skipped"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "a name not found may live in a sitemap that was not read"
        );
    }

    #[tokio::test]
    async fn search_site_without_a_sitemap_is_a_fetch_error() {
        let out = tools(site_with(&[]))
            .call("search_site", json!({"names": ["alamos"]}))
            .await;
        assert_eq!(codes(&out), ["FETCH"]);
    }

    #[tokio::test]
    async fn write_research_validates_and_saves() {
        let t = tools(site());
        let mut bad = research_args();
        bad["opportunities"][0]["demand"] = json!("huge");
        let out = t.call("write_research", bad).await;
        assert!(out.is_error);
        assert_eq!(out.content["errors"][0]["path"], "opportunities[0].demand");
        assert!(!t.call("write_research", research_args()).await.is_error);
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
    async fn finish_needs_a_business_and_the_research() {
        let t = tools(site());
        assert!(t.call("finish", json!({})).await.is_error);
        assert!(!t.finished());
        t.call("fetch_page", json!({"url": "https://vinellu.com"}))
            .await;
        t.call("write_business", business_args()).await;
        let early = t.call("finish", json!({})).await;
        assert!(early.is_error, "the research is the point of init");
        assert_eq!(early.content["errors"][0]["path"], "research");
        t.call("write_research", research_args()).await;
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
