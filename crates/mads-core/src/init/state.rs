use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{RESEARCH_FILE, ResearchDraft};
use crate::{
    google::Issue,
    input::{AllowedUrls, InputError, normalize_url, parse_catalog, parse_input_toml},
    money::Cents,
};

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageDraft {
    pub name: String,
    pub url: String,
}

/// The `[business]` table an agent proposes from the website.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BusinessDraft {
    pub name: String,
    /// The site's main URL.
    pub url: String,
    /// BCP 47 tag of the site's main language, such as pt-BR.
    pub language: String,
    /// Exactly one Google location name, such as Brazil.
    pub locations: Vec<String>,
    /// What the ads should achieve, in a few words.
    pub goal: String,
    /// What the business is, who it is for and what makes it different. 20 to 4000 characters.
    pub description: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub conversion_tracking: bool,
    /// Defaults to the business name when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub brand_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub competitors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avoid: Vec<String>,
    /// Up to 20 key pages that can become sitelinks. URLs must come from fetched pages or the sitemap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PageDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogDraft {
    pub name: String,
    /// Page of this item. Must come from fetched pages or the sitemap.
    pub url: String,
    #[serde(default)]
    pub category: String,
    /// Other ways people write the name.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// True when the name is someone else's trademark.
    #[serde(default)]
    pub third_party: bool,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Added {
    pub added: usize,
    pub skipped: usize,
}

#[derive(Serialize)]
struct OutBudget<'a> {
    daily: f64,
    currency: &'a str,
}

#[derive(Serialize)]
struct OutFileRef {
    file: &'static str,
}

#[derive(Serialize)]
struct OutFile<'a> {
    business: &'a BusinessDraft,
    budget: OutBudget<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog: Option<OutFileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    research: Option<OutFileRef>,
}

/// The files `init` writes. `csv` and `research` are absent when there is nothing to put in them.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedFiles {
    pub toml: String,
    pub csv: Option<String>,
    pub research: Option<String>,
}

pub const CATALOG_FILE: &str = "catalog.csv";

/// What the `init` agent has built so far. Everything it stores passes the same parsers
/// `generate` uses, so a draft that is accepted here always loads.
pub struct InitState {
    daily: Cents,
    currency: String,
    catalog_limit: usize,
    seen: AllowedUrls,
    business: Option<BusinessDraft>,
    catalog: Vec<CatalogDraft>,
    research: Option<ResearchDraft>,
    /// The business host without `www.`: sources there are not web research.
    site_host: String,
    /// Distinct sources outside the site the research must cite. 0 when the agent cannot search.
    web_sources: usize,
}

fn bare_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    Some(host.trim_start_matches("www.").to_string())
}

fn issue_of(e: InputError) -> Issue {
    match e {
        InputError::Invalid { key, message } => Issue::error("INPUT", key, message),
        other => Issue::error("INPUT", "business", other.to_string()),
    }
}

/// `…/a` and `…/a/` are the same page for a crawler.
fn page_key(url: &str) -> Option<String> {
    normalize_url(url).map(|u| u.trim_end_matches('/').to_string())
}

impl InitState {
    pub fn new(start_url: &str, daily: Cents, currency: &str, catalog_limit: usize) -> Self {
        let mut seen = AllowedUrls::default();
        seen.insert(start_url);
        Self {
            daily,
            currency: currency.into(),
            catalog_limit,
            seen,
            business: None,
            catalog: Vec::new(),
            research: None,
            site_host: bare_host(start_url).unwrap_or_default(),
            web_sources: 0,
        }
    }

    /// Makes `set_research` refuse notes that cite fewer than `n` distinct pages outside the site.
    pub fn require_web_sources(&mut self, n: usize) {
        self.web_sources = n;
    }

    fn web_source_issue(&self, draft: &ResearchDraft) -> Option<Issue> {
        let external: std::collections::BTreeSet<&str> = draft
            .opportunities
            .iter()
            .flat_map(|o| o.sources.iter())
            .filter(|u| bare_host(u).is_some_and(|h| h != self.site_host))
            .map(String::as_str)
            .collect();
        (external.len() < self.web_sources).then(|| {
            Issue::error(
                "INPUT",
                "opportunities[].sources",
                format!(
                    "cites {} of {} web pages outside the business site: search the web for demand and competition, then cite what you used",
                    external.len(),
                    self.web_sources
                ),
            )
        })
    }

    pub fn note_seen(&mut self, url: &str) {
        self.seen.insert(url);
    }

    pub fn is_seen(&self, url: &str) -> bool {
        let trimmed = url.trim_end_matches('/');
        self.seen.contains(url)
            || self.seen.contains(trimmed)
            || self.seen.contains(&format!("{trimmed}/"))
    }

    pub fn business(&self) -> Option<&BusinessDraft> {
        self.business.as_ref()
    }

    pub fn catalog_len(&self) -> usize {
        self.catalog.len()
    }

    pub fn catalog(&self) -> &[CatalogDraft] {
        &self.catalog
    }

    pub fn research(&self) -> Option<&ResearchDraft> {
        self.research.as_ref()
    }

    /// Replaces the notes. A draft with any issue changes nothing.
    pub fn set_research(&mut self, draft: ResearchDraft) -> Result<(), Vec<Issue>> {
        let mut issues = draft.validate();
        issues.extend(self.web_source_issue(&draft));
        if !issues.is_empty() {
            return Err(issues);
        }
        self.research = Some(draft);
        Ok(())
    }

    fn render_toml(&self, business: &BusinessDraft, complete: bool) -> Result<String, String> {
        let file = OutFile {
            business,
            budget: OutBudget {
                daily: self.daily.0 as f64 / 100.0,
                currency: &self.currency,
            },
            catalog: (complete && !self.catalog.is_empty())
                .then_some(OutFileRef { file: CATALOG_FILE }),
            research: (complete && self.research.is_some()).then_some(OutFileRef {
                file: RESEARCH_FILE,
            }),
        };
        toml::to_string(&file).map_err(|e| e.to_string())
    }

    pub fn set_business(&mut self, draft: BusinessDraft) -> Result<(), Vec<Issue>> {
        let mut issues: Vec<Issue> = draft
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| !self.is_seen(&p.url))
            .map(|(i, p)| {
                Issue::error(
                    "E07",
                    format!("business.pages[{i}].url"),
                    format!("URL was not fetched or listed in the sitemap: {}", p.url),
                )
            })
            .collect();
        let text = self
            .render_toml(&draft, false)
            .map_err(|m| vec![Issue::error("INPUT", "business", m)])?;
        if let Err(e) = parse_input_toml(&text) {
            issues.push(issue_of(e));
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        self.business = Some(draft);
        Ok(())
    }

    pub fn add_catalog(&mut self, items: Vec<CatalogDraft>) -> Result<Added, Vec<Issue>> {
        let mut issues = Vec::new();
        let mut known: Vec<String> = self
            .catalog
            .iter()
            .filter_map(|c| page_key(&c.url))
            .collect();
        let (mut fresh, mut skipped) = (Vec::new(), 0);
        for (i, item) in items.into_iter().enumerate() {
            if !self.is_seen(&item.url) {
                issues.push(Issue::error(
                    "E07",
                    format!("items[{i}].url"),
                    format!("URL was not fetched or listed in the sitemap: {}", item.url),
                ));
                continue;
            }
            match page_key(&item.url) {
                Some(key) if known.contains(&key) => skipped += 1,
                Some(key) => {
                    known.push(key);
                    fresh.push(item);
                }
                None => issues.push(Issue::error(
                    "INPUT",
                    format!("items[{i}].url"),
                    "not an absolute http(s) URL",
                )),
            }
        }
        if self.catalog.len() + fresh.len() > self.catalog_limit {
            let msg = format!(
                "{} items would exceed the limit of {}",
                self.catalog.len() + fresh.len(),
                self.catalog_limit
            );
            issues.push(Issue::error("E13", "items", msg));
        }
        let mut candidate = self.catalog.clone();
        candidate.extend(fresh.iter().cloned());
        if let Err(e) = parse_catalog(catalog_csv(&candidate)?.as_bytes()) {
            issues.push(issue_of(e));
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        let added = fresh.len();
        self.catalog = candidate;
        Ok(Added { added, skipped })
    }

    /// `business.toml`, plus `catalog.csv` and `research.md` when there is something to put in them.
    pub fn render_files(&self) -> Result<RenderedFiles, String> {
        let business = self
            .business
            .as_ref()
            .ok_or("write_business was not called")?;
        let toml = self.render_toml(business, true)?;
        let csv = if self.catalog.is_empty() {
            None
        } else {
            Some(
                catalog_csv(&self.catalog)
                    .map_err(|i| i.first().map(|x| x.message.clone()).unwrap_or_default())?,
            )
        };
        let research = self
            .research
            .as_ref()
            .map(|r| r.to_markdown(&business.name));
        Ok(RenderedFiles {
            toml,
            csv,
            research,
        })
    }
}

fn catalog_csv(items: &[CatalogDraft]) -> Result<String, Vec<Issue>> {
    let fail = |e: String| vec![Issue::error("INPUT", "catalog", e)];
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(["name", "url", "category", "aliases", "third_party", "notes"])
        .map_err(|e| fail(e.to_string()))?;
    for i in items {
        let aliases = i.aliases.join("|");
        let third = if i.third_party { "true" } else { "false" };
        let row = [
            i.name.as_str(),
            i.url.as_str(),
            i.category.as_str(),
            aliases.as_str(),
            third,
            i.notes.as_str(),
        ];
        w.write_record(row).map_err(|e| fail(e.to_string()))?;
    }
    let bytes = w.into_inner().map_err(|e| fail(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| fail(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{parse_catalog, parse_input_toml};

    fn state() -> InitState {
        let mut s = InitState::new("https://vinellu.com", Cents(5000), "BRL", 3);
        s.note_seen("https://vinellu.com/app");
        s.note_seen("https://vinellu.com/w/alamos");
        s.note_seen("https://vinellu.com/w/luigi");
        s.note_seen("https://vinellu.com/w/cartuxa");
        s.note_seen("https://vinellu.com/w/extra");
        s
    }

    fn business() -> BusinessDraft {
        BusinessDraft {
            name: "Vinellu".into(),
            url: "https://vinellu.com".into(),
            language: "pt-BR".into(),
            locations: vec!["Brazil".into()],
            goal: "cadastros no app".into(),
            description:
                "App social de vinhos com reviews, safras e harmonização.\nMais de 170 mil rótulos."
                    .into(),
            conversion_tracking: false,
            brand_terms: vec![],
            competitors: vec!["Vivino".into()],
            avoid: vec![],
            pages: vec![PageDraft {
                name: "Baixe o app".into(),
                url: "https://vinellu.com/app".into(),
            }],
        }
    }

    fn item(name: &str, url: &str) -> CatalogDraft {
        CatalogDraft {
            name: name.into(),
            url: url.into(),
            category: "malbec".into(),
            aliases: vec!["alamos".into(), "alamos malbec".into()],
            third_party: true,
            notes: String::new(),
        }
    }

    #[test]
    fn start_url_is_always_a_known_url() {
        let s = InitState::new("https://vinellu.com", Cents(5000), "BRL", 10);
        assert!(s.is_seen("https://vinellu.com/"));
        assert!(!s.is_seen("https://vinellu.com/other"));
    }

    #[test]
    fn a_valid_business_is_accepted() {
        let mut s = state();
        assert_eq!(s.set_business(business()), Ok(()));
        assert!(s.business().is_some());
    }

    #[test]
    fn the_generated_toml_round_trips_through_the_real_parser() {
        let mut s = state();
        s.set_business(business()).unwrap();
        s.add_catalog(vec![item("Alamos Malbec", "https://vinellu.com/w/alamos")])
            .unwrap();
        let RenderedFiles { toml, csv, .. } = s.render_files().unwrap();
        let parsed = parse_input_toml(&toml).unwrap();
        assert_eq!(parsed.business.name, "Vinellu");
        assert_eq!(parsed.business.description, business().description);
        assert_eq!(parsed.business.competitors, ["Vivino"]);
        assert_eq!(parsed.business.pages[0].url, "https://vinellu.com/app");
        assert_eq!(parsed.budget.daily, Cents(5000));
        assert_eq!(parsed.budget.currency, "BRL");
        assert_eq!(parsed.catalog_file.as_deref(), Some("catalog.csv"));
        let items = parse_catalog(csv.unwrap().as_bytes()).unwrap();
        assert_eq!(items[0].aliases, ["alamos", "alamos malbec"]);
        assert!(items[0].third_party);
    }

    #[test]
    fn without_catalog_items_there_is_no_csv_and_no_catalog_key() {
        let mut s = state();
        s.set_business(business()).unwrap();
        let RenderedFiles { toml, csv, .. } = s.render_files().unwrap();
        assert!(csv.is_none());
        assert_eq!(parse_input_toml(&toml).unwrap().catalog_file, None);
    }

    #[test]
    fn business_errors_name_the_toml_key() {
        let mut s = state();
        let mut b = business();
        b.locations = vec!["Brazil".into(), "Chile".into()];
        let err = s.set_business(b).unwrap_err();
        assert_eq!(err[0].path, "business.locations");
        let mut b = business();
        b.description = "curto".into();
        assert_eq!(
            s.set_business(b).unwrap_err()[0].path,
            "business.description"
        );
    }

    #[test]
    fn page_urls_must_have_been_seen_in_this_run() {
        let mut s = state();
        let mut b = business();
        b.pages = vec![PageDraft {
            name: "Inventada".into(),
            url: "https://vinellu.com/inventada".into(),
        }];
        let err = s.set_business(b).unwrap_err();
        assert!(err.iter().any(|i| i.code == "E07"), "{err:?}");
    }

    #[test]
    fn a_failed_business_keeps_the_previous_one() {
        let mut s = state();
        s.set_business(business()).unwrap();
        let mut bad = business();
        bad.name = String::new();
        assert!(s.set_business(bad).is_err());
        assert_eq!(s.business().unwrap().name, "Vinellu");
    }

    #[test]
    fn catalog_items_need_seen_urls_and_valid_rows() {
        let mut s = state();
        let err = s
            .add_catalog(vec![item("Fantasma", "https://vinellu.com/w/fantasma")])
            .unwrap_err();
        assert!(err.iter().any(|i| i.code == "E07"));
        let err = s
            .add_catalog(vec![item("", "https://vinellu.com/w/alamos")])
            .unwrap_err();
        assert!(!err.is_empty());
        assert_eq!(s.catalog_len(), 0, "nothing is added when any item is bad");
    }

    #[test]
    fn duplicate_urls_are_skipped_not_errors() {
        let mut s = state();
        assert_eq!(
            s.add_catalog(vec![item("Alamos", "https://vinellu.com/w/alamos")])
                .unwrap(),
            Added {
                added: 1,
                skipped: 0
            }
        );
        let again = s
            .add_catalog(vec![
                item("Alamos Malbec", "https://vinellu.com/w/alamos/"),
                item("Luigi", "https://vinellu.com/w/luigi"),
            ])
            .unwrap();
        assert_eq!(
            again,
            Added {
                added: 1,
                skipped: 1
            }
        );
        assert_eq!(s.catalog_len(), 2);
    }

    #[test]
    fn the_catalog_limit_is_enforced() {
        let mut s = state();
        s.add_catalog(vec![
            item("A", "https://vinellu.com/w/alamos"),
            item("B", "https://vinellu.com/w/luigi"),
            item("C", "https://vinellu.com/w/cartuxa"),
        ])
        .unwrap();
        let err = s
            .add_catalog(vec![item("D", "https://vinellu.com/w/extra")])
            .unwrap_err();
        assert!(err.iter().any(|i| i.code == "E13"), "{err:?}");
        assert_eq!(s.catalog_len(), 3);
    }

    fn research() -> ResearchDraft {
        ResearchDraft {
            summary: "A social app for wine lovers. People rate labels and follow friends.".into(),
            opportunities: vec![],
            open_questions: vec![],
        }
    }

    fn research_citing(sources: &[&str]) -> ResearchDraft {
        let mut r = research();
        r.opportunities = vec![super::super::Opportunity {
            name: "Items by name".into(),
            intent: "catalog".into(),
            searches: vec!["item review".into()],
            demand: "high".into(),
            competition: "low".into(),
            evidence: "Bestsellers are searched by name.".into(),
            sources: sources.iter().map(|s| s.to_string()).collect(),
            names_checked: 0,
            names_found: 0,
        }];
        r
    }

    #[test]
    fn with_web_search_the_research_must_cite_the_web() {
        let mut s = state();
        s.require_web_sources(3);
        let err = s
            .set_research(research_citing(&[
                "https://vinellu.com/vinhos",
                "https://www.vinellu.com/app",
                "https://example.com/ranking",
                "https://example.com/ranking",
            ]))
            .unwrap_err();
        assert_eq!(err[0].path, "opportunities[].sources");
        assert!(err[0].message.contains("1 of 3"), "{}", err[0].message);
        assert!(s.research().is_none());
        let ok = research_citing(&[
            "https://example.com/ranking",
            "https://news.example.org/top",
            "https://shop.example.net/bestsellers",
        ]);
        assert_eq!(s.set_research(ok), Ok(()));
    }

    #[test]
    fn without_web_search_site_sources_are_enough() {
        let mut s = state();
        assert_eq!(
            s.set_research(research_citing(&["https://vinellu.com/vinhos"])),
            Ok(())
        );
    }

    #[test]
    fn research_is_validated_and_a_bad_one_keeps_the_previous() {
        let mut s = state();
        assert_eq!(s.set_research(research()), Ok(()));
        let mut bad = research();
        bad.summary = "short".into();
        let err = s.set_research(bad).unwrap_err();
        assert_eq!(err[0].path, "summary");
        assert_eq!(s.research(), Some(&research()));
    }

    #[test]
    fn research_becomes_a_markdown_file_the_input_loader_points_at() {
        let mut s = state();
        s.set_business(business()).unwrap();
        s.set_research(research()).unwrap();
        let RenderedFiles {
            toml, research: md, ..
        } = s.render_files().unwrap();
        assert_eq!(
            parse_input_toml(&toml).unwrap().research_file.as_deref(),
            Some("research.md")
        );
        assert!(md.unwrap().starts_with("# Research: Vinellu"));
    }

    #[test]
    fn without_research_there_is_no_research_key() {
        let mut s = state();
        s.set_business(business()).unwrap();
        let RenderedFiles {
            toml, research: md, ..
        } = s.render_files().unwrap();
        assert_eq!(parse_input_toml(&toml).unwrap().research_file, None);
        assert!(md.is_none());
    }

    #[test]
    fn rendering_needs_a_business() {
        assert!(state().render_files().is_err());
    }

    #[test]
    fn aliases_with_pipes_and_commas_survive_the_csv() {
        let mut s = state();
        s.set_business(business()).unwrap();
        let mut it = item("Casa, Vinho & Cia", "https://vinellu.com/w/alamos");
        it.notes = "tem \"aspas\", vírgula".into();
        s.add_catalog(vec![it]).unwrap();
        let RenderedFiles { csv, .. } = s.render_files().unwrap();
        let items = parse_catalog(csv.unwrap().as_bytes()).unwrap();
        assert_eq!(items[0].name, "Casa, Vinho & Cia");
        assert_eq!(items[0].notes, "tem \"aspas\", vírgula");
    }
}
