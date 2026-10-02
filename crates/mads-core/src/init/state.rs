use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ColorNote, DESIGN_FILE, DesignDraft, RESEARCH_FILE, ResearchDraft, render_design};
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
    /// The one offer the account advertises. Leave empty to advertise the whole business.
    #[serde(default, skip_serializing)]
    pub focus: FocusDraft,
    /// Google Ads restricted content categories the business falls in, an empty list for none.
    /// Required, so the choice is always made and written down.
    pub restricted: Vec<crate::input::RestrictedCategory>,
}

/// `[focus]` of business.toml: a name and the pages of one offer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusDraft {
    /// The offer as people say it, such as a route, a product line or a location.
    #[serde(default)]
    pub name: String,
    /// The page of the offer and its close variants (such as the other direction of a route). Fetched or in the sitemap.
    #[serde(default)]
    pub urls: Vec<String>,
    /// Groups of words every search about the offer has, one group per part of it, each with the ways
    /// people write that part. For a route: the origin's names, then the destination's names.
    #[serde(default)]
    pub terms: Vec<Vec<String>>,
}

impl FocusDraft {
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty() && self.urls.is_empty()
    }
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
    /// The `image` that fetch_page showed for this item's page: a real photo of the item. Empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub image: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    max_cpc: Option<&'a toml::Value>,
}

/// What a person wrote in an earlier business.toml that init does not draft. `--force` keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeptByHand {
    max_cpc: Option<toml::Value>,
    export: Option<toml::Value>,
    campaigns: Option<toml::Value>,
    app: Option<crate::input::App>,
}

impl KeptByHand {
    /// Reads `[export]`, `[campaigns]` and `budget.max_cpc` from an earlier file. Unparsable text keeps nothing.
    pub fn from_toml(text: &str) -> Self {
        let Ok(old) = text.parse::<toml::Table>() else {
            return Self::default();
        };
        Self {
            max_cpc: old.get("budget").and_then(|b| b.get("max_cpc")).cloned(),
            export: old.get("export").cloned(),
            campaigns: old.get("campaigns").cloned(),
            app: old.get("app").cloned().and_then(|v| v.try_into().ok()),
        }
    }

    /// The names of what is kept, for the progress output.
    pub fn names(&self) -> Vec<&'static str> {
        [
            ("budget.max_cpc", self.max_cpc.is_some()),
            ("[export]", self.export.is_some()),
            ("[campaigns]", self.campaigns.is_some()),
            ("[app]", self.app.is_some()),
        ]
        .into_iter()
        .filter_map(|(n, kept)| kept.then_some(n))
        .collect()
    }
}

#[derive(Serialize)]
struct OutFileRef {
    file: &'static str,
}

#[derive(Serialize)]
struct OutBrand<'a> {
    logo: &'a str,
}

#[derive(Serialize)]
struct OutFile<'a> {
    business: &'a BusinessDraft,
    budget: OutBudget<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog: Option<OutFileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    research: Option<OutFileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand: Option<OutBrand<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    design: Option<OutFileRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    focus: Option<&'a FocusDraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<&'a crate::input::App>,
    #[serde(skip_serializing_if = "Option::is_none")]
    export: Option<&'a toml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    campaigns: Option<&'a toml::Value>,
}

/// The files `init` writes. `csv` and `research` are absent when there is nothing to put in them.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedFiles {
    pub toml: String,
    pub csv: Option<String>,
    pub research: Option<String>,
    pub design: Option<String>,
}

pub const CATALOG_FILE: &str = "catalog.csv";
/// Where `init` saves the logo it downloads, relative to the output folder.
pub const LOGO_FILE: &str = "brand/logo.png";
const MAX_LOGO_CANDIDATES: usize = 20;

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
    /// `og:image` URLs of fetched pages: the only photos a catalog item may point at.
    images: AllowedUrls,
    logo_candidates: Vec<String>,
    /// Set once `init` saved a logo, relative to the output folder.
    logo: Option<String>,
    design: Option<DesignDraft>,
    /// Logo colors first, then the theme colors of fetched pages.
    colors: Vec<ColorNote>,
    /// `--focus`: the business must name a focus that holds the start URL.
    focus_required: bool,
    start_url: String,
    kept: KeptByHand,
    /// The first Google Play app the site links to, else the first App Store one.
    app: Option<crate::input::App>,
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
            images: AllowedUrls::default(),
            logo_candidates: Vec::new(),
            logo: None,
            design: None,
            colors: Vec::new(),
            focus_required: false,
            start_url: start_url.to_string(),
            kept: KeptByHand::default(),
            app: None,
        }
    }

    /// A kept `[campaigns] formats` that leaves out the app this run found: the old choice is
    /// kept, and the person should know it skips the App campaign.
    pub fn stale_formats_note(&self) -> Option<String> {
        let formats = self.kept.campaigns.as_ref()?.get("formats")?.as_array()?;
        let has_app_format = formats.iter().any(|f| f.as_str() == Some("app_installs"));
        let app = self.app.as_ref().or(self.kept.app.as_ref())?;
        (!has_app_format).then(|| {
            format!(
                "kept formats leave out the app {} found on the site: add \"app_installs\" to [campaigns] formats to advertise it",
                app.id
            )
        })
    }

    /// Keeps the app a page links to. Google Play wins over the App Store: one campaign takes one store.
    pub fn note_app_links(&mut self, links: &[String]) {
        use crate::input::{App, AppStore};
        for app in links.iter().filter_map(|l| App::from_store_link(l)) {
            let better = match &self.app {
                None => true,
                Some(current) => {
                    current.store == AppStore::AppStore && app.store == AppStore::GooglePlay
                }
            };
            if better {
                self.app = Some(app);
            }
        }
    }

    /// Carries hand-written parts of an earlier business.toml into the new one.
    pub fn keep(&mut self, kept: KeptByHand) {
        self.kept = kept;
    }

    pub fn require_focus(&mut self) {
        self.focus_required = true;
    }

    fn focus(&self) -> Option<&FocusDraft> {
        self.business
            .as_ref()
            .map(|b| &b.focus)
            .filter(|f| !f.is_empty())
    }

    fn focus_issues(&self, f: &FocusDraft) -> Vec<Issue> {
        let mut out = Vec::new();
        let unseen = f.urls.iter().filter(|u| !self.is_seen(u));
        out.extend(unseen.map(|u| {
            Issue::error(
                "E07",
                "business.focus.urls",
                format!("URL was not fetched or listed in the sitemap: {u}"),
            )
        }));
        let start = page_key(&self.start_url);
        let has_start = f.urls.iter().any(|u| page_key(u) == start);
        if self.focus_required && (f.name.trim().is_empty() || !has_start) {
            let msg = format!(
                "--focus needs business.focus with a name and {} in urls",
                self.start_url
            );
            out.push(Issue::error("E22", "business.focus", msg));
        }
        if self.focus_required && f.terms.is_empty() {
            let msg =
                "--focus needs business.focus.terms: the words every search about the offer has";
            out.push(Issue::error("E23", "business.focus.terms", msg));
        }
        out
    }

    /// Keeps a `theme-color` the site declares, once per color.
    pub fn note_theme_color(&mut self, page: &str, color: &str) {
        let Some(rgb) = crate::images::parse_hex(color) else {
            return;
        };
        let hex = crate::images::hex(rgb);
        if !self.colors.iter().any(|c| c.hex == hex) {
            let source = format!("theme color of {page}");
            self.colors.push(ColorNote { hex, source });
        }
    }

    /// Puts the logo colors ahead of the theme colors.
    pub fn set_logo_colors(&mut self, hexes: &[String]) {
        let mut notes: Vec<ColorNote> = hexes
            .iter()
            .map(|h| ColorNote {
                hex: h.clone(),
                source: "logo".into(),
            })
            .collect();
        notes.extend(self.colors.drain(..).filter(|c| !hexes.contains(&c.hex)));
        self.colors = notes;
    }

    pub fn set_design(&mut self, draft: DesignDraft) -> Result<(), Vec<Issue>> {
        let issues = draft.validate();
        if !issues.is_empty() {
            return Err(issues);
        }
        self.design = Some(draft);
        Ok(())
    }

    fn design_markdown(&self, business: &str) -> Option<String> {
        render_design(business, &self.colors, self.design.as_ref())
    }

    /// Records what a fetched page showed: its photo and its logo candidates.
    pub fn note_page_media(&mut self, image: &str, logos: &[String]) {
        if !image.is_empty() {
            self.images.insert(image);
        }
        for l in logos {
            if self.logo_candidates.len() < MAX_LOGO_CANDIDATES && !self.logo_candidates.contains(l)
            {
                self.logo_candidates.push(l.clone());
            }
        }
    }

    pub fn logo_candidates(&self) -> &[String] {
        &self.logo_candidates
    }

    pub fn set_logo(&mut self, rel: &str) {
        self.logo = Some(rel.to_string());
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

    fn render_toml(
        &self,
        business: &BusinessDraft,
        complete: bool,
        kept: &KeptByHand,
    ) -> Result<String, String> {
        let file = OutFile {
            business,
            budget: OutBudget {
                daily: self.daily.0 as f64 / 100.0,
                currency: &self.currency,
                max_cpc: kept.max_cpc.as_ref().filter(|_| complete),
            },
            catalog: (complete && !self.catalog.is_empty())
                .then_some(OutFileRef { file: CATALOG_FILE }),
            research: (complete && self.research.is_some()).then_some(OutFileRef {
                file: RESEARCH_FILE,
            }),
            brand: self
                .logo
                .as_deref()
                .filter(|_| complete)
                .map(|logo| OutBrand { logo }),
            design: (complete && self.design_markdown(&business.name).is_some())
                .then_some(OutFileRef { file: DESIGN_FILE }),
            focus: Some(&business.focus).filter(|f| !f.is_empty()),
            app: self.app.as_ref().or(kept.app.as_ref()).filter(|_| complete),
            export: kept.export.as_ref().filter(|_| complete),
            campaigns: kept.campaigns.as_ref().filter(|_| complete),
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
        issues.extend(self.focus_issues(&draft.focus));
        let text = self
            .render_toml(&draft, false, &KeptByHand::default())
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
            if let Some(f) = self.focus()
                && !f.urls.iter().any(|u| page_key(u) == page_key(&item.url))
            {
                issues.push(Issue::error(
                    "E22",
                    format!("items[{i}].url"),
                    format!("not a page of the focus '{}': {}", f.name, item.url),
                ));
                continue;
            }
            if !item.image.is_empty() && !self.images.contains(&item.image) {
                issues.push(Issue::error(
                    "E07",
                    format!("items[{i}].image"),
                    format!("image was not shown by a fetched page: {}", item.image),
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
        let mut toml = self.render_toml(business, true, &self.kept)?;
        // A kept part can stop fitting the new draft (a format that needs a logo the site no
        // longer has): the draft wins and the kept parts are left out.
        if parse_input_toml(&toml).is_err() {
            toml = self.render_toml(business, true, &KeptByHand::default())?;
        }
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
            design: self.design_markdown(&business.name),
        })
    }
}

fn catalog_csv(items: &[CatalogDraft]) -> Result<String, Vec<Issue>> {
    let fail = |e: String| vec![Issue::error("INPUT", "catalog", e)];
    let mut w = csv::Writer::from_writer(Vec::new());
    let photos = items.iter().any(|i| !i.image.is_empty());
    let mut header = vec!["name", "url", "category", "aliases", "third_party", "notes"];
    if photos {
        header.push("image");
    }
    w.write_record(&header).map_err(|e| fail(e.to_string()))?;
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
            i.image.as_str(),
        ];
        w.write_record(&row[..header.len()])
            .map_err(|e| fail(e.to_string()))?;
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
            focus: FocusDraft::default(),
            restricted: vec![crate::input::RestrictedCategory::Alcohol],
        }
    }

    #[test]
    fn with_focus_required_the_business_names_it_and_the_catalog_stays_inside() {
        let mut s = state();
        s.require_focus();
        let errs = s.set_business(business()).unwrap_err();
        assert!(errs.iter().any(|i| i.code == "E22"), "{errs:?}");
        let mut b = business();
        b.focus = FocusDraft {
            name: "Alamos".into(),
            urls: vec!["https://vinellu.com/w/alamos".into()],
            terms: vec![],
        };
        let errs = s.set_business(b.clone()).unwrap_err();
        assert!(
            errs.iter().any(|i| i.code == "E22"),
            "the start URL must be in the focus"
        );
        assert!(errs.iter().any(|i| i.code == "E23"), "terms are required");
        b.focus.terms = vec![vec!["alamos".into()]];
        b.focus.urls.push("https://vinellu.com".into());
        s.set_business(b).unwrap();
        let errs = s
            .add_catalog(vec![item("Luigi", "https://vinellu.com/w/luigi")])
            .unwrap_err();
        assert_eq!(errs[0].code, "E22");
        s.add_catalog(vec![item("Alamos", "https://vinellu.com/w/alamos")])
            .unwrap();
        let toml = s.render_files().unwrap().toml;
        let parsed = parse_input_toml(&toml).unwrap();
        assert_eq!(parsed.focus.unwrap().name, "Alamos");
    }

    fn item(name: &str, url: &str) -> CatalogDraft {
        CatalogDraft {
            name: name.into(),
            url: url.into(),
            category: "malbec".into(),
            aliases: vec!["alamos".into(), "alamos malbec".into()],
            third_party: true,
            notes: String::new(),
            image: String::new(),
        }
    }

    #[test]
    fn an_item_photo_must_come_from_a_fetched_page_and_reaches_the_csv() {
        let mut s = state();
        let mut a = item("Alamos", "https://vinellu.com/w/alamos");
        a.image = "https://cdn.vinellu.com/alamos.jpg".into();
        let errs = s.add_catalog(vec![a.clone()]).unwrap_err();
        assert_eq!(errs[0].path, "items[0].image");
        s.note_page_media("https://cdn.vinellu.com/alamos.jpg", &[]);
        s.add_catalog(vec![a, item("Luigi", "https://vinellu.com/w/luigi")])
            .unwrap();
        s.set_business(business()).unwrap();
        let csv = s.render_files().unwrap().csv.unwrap();
        let parsed = parse_catalog(csv.as_bytes()).unwrap();
        assert_eq!(
            parsed[0].image.as_deref(),
            Some("https://cdn.vinellu.com/alamos.jpg")
        );
        assert_eq!(parsed[1].image, None);
    }

    #[test]
    fn a_saved_logo_becomes_the_brand_table() {
        let mut s = state();
        s.set_business(business()).unwrap();
        assert!(!s.render_files().unwrap().toml.contains("[brand]"));
        s.note_page_media(
            "",
            &[
                "https://vinellu.com/a.png".into(),
                "https://vinellu.com/a.png".into(),
            ],
        );
        assert_eq!(s.logo_candidates().len(), 1);
        s.set_logo(LOGO_FILE);
        let toml = s.render_files().unwrap().toml;
        assert!(
            toml.contains("[brand]\nlogo = \"brand/logo.png\""),
            "{toml}"
        );
        assert_eq!(
            parse_input_toml(&toml).unwrap().logo_file.as_deref(),
            Some(LOGO_FILE)
        );
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
    fn colors_and_the_design_draft_become_design_md() {
        let mut s = state();
        s.set_business(business()).unwrap();
        assert!(s.render_files().unwrap().design.is_none());
        s.note_theme_color("https://vinellu.com", "#ad1457");
        s.note_theme_color("https://vinellu.com/app", "#AD1457");
        s.note_theme_color("https://vinellu.com", "not a color");
        s.set_logo_colors(&["#F0476A".into(), "#AD1457".into()]);
        let RenderedFiles { toml, design, .. } = s.render_files().unwrap();
        let md = design.unwrap();
        assert!(
            md.contains("- Pink #F0476A: logo\n- Dark pink #AD1457: logo\n"),
            "{md}"
        );
        assert!(toml.contains("[design]\nfile = \"DESIGN.md\""), "{toml}");
        let bad = DesignDraft {
            style: String::new(),
            imagery: String::new(),
            voice: String::new(),
            avoid: vec![],
        };
        assert!(s.set_design(bad).is_err());
    }

    #[test]
    fn the_restricted_decision_is_written_even_when_empty() {
        let mut s = state();
        let mut b = business();
        b.restricted = vec![];
        s.set_business(b).unwrap();
        let toml = s.render_files().unwrap().toml;
        assert!(toml.contains("restricted = []"), "{toml}");
        let mut s = state();
        s.set_business(business()).unwrap();
        let toml = s.render_files().unwrap().toml;
        assert!(toml.contains("restricted = [\"alcohol\"]"), "{toml}");
        assert_eq!(
            parse_input_toml(&toml).unwrap().business.restricted,
            [crate::input::RestrictedCategory::Alcohol]
        );
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
