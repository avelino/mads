use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{CatalogItem, normalize_url, parse_catalog};
use crate::{google::CampaignKind, money::Cents};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InputError {
    #[error("{key}: {message}")]
    Invalid { key: String, message: String },
    #[error("{0}")]
    Parse(String),
    #[error("{0}")]
    Io(String),
}

fn invalid(key: &str, message: impl Into<String>) -> InputError {
    InputError::Invalid {
        key: key.into(),
        message: message.into(),
    }
}

/// Research notes reach every plan prompt, so they stay short enough to be cheap.
pub const RESEARCH_MAX_CHARS: usize = 20_000;
/// DESIGN.md goes into every image brief prompt.
pub const DESIGN_MAX_CHARS: usize = 8_000;

const DEFAULT_URL_SUFFIX: &str = "utm_source=google&utm_medium=cpc&utm_campaign={mads_campaign}&utm_content={adgroupid}&utm_term={keyword}";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    business: RawBusiness,
    budget: RawBudget,
    #[serde(default)]
    export: RawExport,
    catalog: Option<RawCatalog>,
    research: Option<RawResearch>,
    brand: Option<RawBrand>,
    campaigns: Option<RawCampaigns>,
    design: Option<RawResearch>,
    focus: Option<RawFocus>,
    app: Option<App>,
    #[serde(default)]
    google_ads: RawGoogleAds,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFocus {
    name: String,
    urls: Vec<String>,
    #[serde(default)]
    terms: Vec<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBusiness {
    name: String,
    url: String,
    language: String,
    locations: Vec<String>,
    goal: String,
    description: String,
    #[serde(default)]
    conversion_tracking: bool,
    brand_terms: Option<Vec<String>>,
    #[serde(default)]
    competitors: Vec<String>,
    #[serde(default)]
    avoid: Vec<String>,
    #[serde(default)]
    pages: Vec<RawPage>,
    #[serde(default)]
    restricted: Vec<RestrictedCategory>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    name: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBudget {
    daily: f64,
    currency: String,
    max_cpc: Option<f64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawExport {
    status: Option<ExportStatus>,
    url_suffix: Option<String>,
    eu_political_ads: Option<bool>,
    decimal_comma: Option<bool>,
}

/// Optional account details for the drive-folder Editor files.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawGoogleAds {
    #[serde(default)]
    customer_id: String,
    #[serde(default)]
    tracking_template: String,
    #[serde(default)]
    devices: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    app_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResearch {
    file: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBrand {
    logo: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCampaigns {
    formats: Vec<CampaignKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    Paused,
    Enabled,
}

impl ExportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportStatus::Paused => "Paused",
            ExportStatus::Enabled => "Enabled",
        }
    }
}

/// Store of the app an `app_installs` campaign promotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppStore {
    GooglePlay,
    AppStore,
}

/// `[app]`: the app an `app_installs` campaign promotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub store: AppStore,
    /// Package name on Google Play (`com.example.app`), numeric id on the App Store.
    pub id: String,
}

impl App {
    /// The store page, which app ads link to.
    pub fn store_url(&self) -> String {
        match self.store {
            AppStore::GooglePlay => {
                format!("https://play.google.com/store/apps/details?id={}", self.id)
            }
            AppStore::AppStore => format!("https://apps.apple.com/app/id{}", self.id),
        }
    }

    /// The `App campaign store` value Google Ads Editor writes.
    pub fn store_label(&self) -> &'static str {
        match self.store {
            AppStore::GooglePlay => "Google Play",
            AppStore::AppStore => "iTunes",
        }
    }
}

impl App {
    /// The app a store link points at: `play.google.com/store/apps/details?id=…` or
    /// `apps.apple.com/…/id<digits>`. None for any other link.
    pub fn from_store_link(link: &str) -> Option<App> {
        let url = url::Url::parse(link).ok()?;
        match url.host_str()? {
            "play.google.com" if url.path().starts_with("/store/apps/details") => {
                let id = url.query_pairs().find(|(k, _)| k == "id")?.1.to_string();
                check_app(Some(App {
                    store: AppStore::GooglePlay,
                    id,
                }))
                .ok()?
            }
            "apps.apple.com" | "itunes.apple.com" => {
                let last = url.path_segments()?.next_back()?;
                let id = last.strip_prefix("id")?.to_string();
                check_app(Some(App {
                    store: AppStore::AppStore,
                    id,
                }))
                .ok()?
            }
            _ => None,
        }
    }
}

fn check_app(app: Option<App>) -> Result<Option<App>, InputError> {
    let Some(a) = app else {
        return Ok(None);
    };
    let ok = match a.store {
        AppStore::GooglePlay => {
            a.id.contains('.')
                && a.id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
        }
        AppStore::AppStore => !a.id.is_empty() && a.id.chars().all(|c| c.is_ascii_digit()),
    };
    if !ok {
        let msg =
            "Google Play takes a package name such as com.example.app, the App Store a numeric id";
        return Err(invalid("app.id", msg));
    }
    Ok(Some(a))
}

/// The one offer an account advertises, such as a route or a product line. Every final URL is one of `urls`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub name: String,
    pub urls: Vec<String>,
    /// Groups of words that make a search about the focus. Every keyword has one word of each group.
    #[serde(default)]
    pub terms: Vec<Vec<String>>,
}

const FOCUS_MAX_URLS: usize = 10;
const FOCUS_MAX_GROUPS: usize = 5;
const FOCUS_MAX_TERMS: usize = 10;

fn check_focus_terms(groups: &[Vec<String>]) -> Result<(), InputError> {
    let key = "focus.terms";
    if groups.len() > FOCUS_MAX_GROUPS {
        return Err(invalid(key, format!("at most {FOCUS_MAX_GROUPS} groups")));
    }
    for (i, g) in groups.iter().enumerate() {
        if g.is_empty() || g.len() > FOCUS_MAX_TERMS {
            let msg = format!("group {i} needs 1 to {FOCUS_MAX_TERMS} terms");
            return Err(invalid(key, msg));
        }
        check_list(&format!("{key}[{i}]"), g)?;
    }
    Ok(())
}

fn check_focus(raw: Option<RawFocus>) -> Result<Option<Focus>, InputError> {
    let Some(f) = raw else {
        return Ok(None);
    };
    check_len("focus.name", &f.name, 1, 80)?;
    if f.urls.is_empty() || f.urls.len() > FOCUS_MAX_URLS {
        return Err(invalid(
            "focus.urls",
            format!("1 to {FOCUS_MAX_URLS} URLs, got {}", f.urls.len()),
        ));
    }
    if let Some(bad) = f.urls.iter().find(|u| normalize_url(u).is_none()) {
        return Err(invalid(
            "focus.urls",
            format!("not an absolute http(s) URL: {bad}"),
        ));
    }
    check_focus_terms(&f.terms)?;
    Ok(Some(Focus {
        name: f.name,
        urls: f.urls,
        terms: f.terms,
    }))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Business {
    pub name: String,
    pub url: String,
    pub language: String,
    pub locations: Vec<String>,
    pub goal: String,
    pub description: String,
    pub conversion_tracking: bool,
    pub brand_terms: Vec<String>,
    pub competitors: Vec<String>,
    pub avoid: Vec<String>,
    pub pages: Vec<Page>,
    /// Google Ads restricted content categories the business falls in. Empty: none.
    #[serde(default)]
    pub restricted: Vec<RestrictedCategory>,
}

/// A category of Google Ads' restricted content policy. Ads and keywords in it face extra review,
/// country rules and automatic disapprovals.
// No doc comments on the variants: schemars would turn them into a `oneOf` some providers reject.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RestrictedCategory {
    Alcohol,
    Gambling,
    Healthcare,
    FinancialServices,
    Political,
    SexualContent,
}

impl RestrictedCategory {
    pub const ALL: [RestrictedCategory; 6] = [
        RestrictedCategory::Alcohol,
        RestrictedCategory::Gambling,
        RestrictedCategory::Healthcare,
        RestrictedCategory::FinancialServices,
        RestrictedCategory::Political,
        RestrictedCategory::SexualContent,
    ];

    /// The policy's name in Google Ads help.
    pub fn policy(self) -> &'static str {
        match self {
            RestrictedCategory::Alcohol => "Alcohol",
            RestrictedCategory::Gambling => "Gambling and games",
            RestrictedCategory::Healthcare => "Healthcare and medicines",
            RestrictedCategory::FinancialServices => "Financial services",
            RestrictedCategory::Political => "Political content",
            RestrictedCategory::SexualContent => "Sexual content",
        }
    }
}

impl Business {
    /// `pt-BR` becomes `pt`; this is what the CSV `Language` column carries.
    pub fn language_primary(&self) -> &str {
        self.language.split('-').next().unwrap_or(&self.language)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub daily: Cents,
    pub currency: String,
    pub max_cpc: Option<Cents>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportConfig {
    pub status: ExportStatus,
    pub url_suffix: String,
    pub eu_political_ads: bool,
    pub decimal_comma: bool,
}

/// Optional `[google_ads]` values for the drive-folder Editor files.
/// Empty strings mean unset. `customer_id` is never the `000-000-0000` placeholder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GoogleAds {
    #[serde(default)]
    pub customer_id: String,
    #[serde(default)]
    pub tracking_template: String,
    #[serde(default)]
    pub devices: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub app_id: String,
}

/// Validated `business.toml`. `catalog_file` is still relative to the TOML file.
#[derive(Debug, Clone, PartialEq)]
pub struct InputFile {
    pub business: Business,
    pub budget: Budget,
    pub export: ExportConfig,
    pub catalog_file: Option<String>,
    /// Markdown notes from `mads init`, relative to the TOML file.
    pub research_file: Option<String>,
    /// Square logo for image campaigns, relative to the TOML file.
    pub logo_file: Option<String>,
    /// Campaign formats the plan must use. Empty: the plan agent picks.
    pub formats: Vec<CampaignKind>,
    /// DESIGN.md, relative to the TOML file.
    pub design_file: Option<String>,
    pub focus: Option<Focus>,
    pub app: Option<App>,
    pub google_ads: GoogleAds,
}

/// Everything `generate` consumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub business: Business,
    pub budget: Budget,
    pub export: ExportConfig,
    pub catalog: Vec<CatalogItem>,
    /// What `mads init` learned about the business and its demand. Empty when there are no notes.
    #[serde(default)]
    pub research: String,
    /// Absolute path of a logo that passed Google's checks. Image campaigns need it.
    #[serde(default)]
    pub logo: Option<String>,
    /// Formats the account must have, at least one campaign each. Empty: the plan agent picks.
    #[serde(default)]
    pub formats: Vec<CampaignKind>,
    /// Brand identity for image campaigns, from DESIGN.md. Empty without one.
    #[serde(default)]
    pub design: String,
    /// The offer the account advertises. None: the whole business.
    #[serde(default)]
    pub focus: Option<Focus>,
    /// The app `app_installs` campaigns promote.
    #[serde(default)]
    pub app: Option<App>,
    /// Account details for `--layout drive-folders`. Absent in older `workspace.json` files.
    #[serde(default)]
    pub google_ads: GoogleAds,
}

fn check_len(key: &str, s: &str, min: usize, max: usize) -> Result<(), InputError> {
    let n = s.trim().chars().count();
    if (min..=max).contains(&n) {
        Ok(())
    } else {
        Err(invalid(
            key,
            format!("must be {min} to {max} chars, got {n}"),
        ))
    }
}

fn check_list(key: &str, items: &[String]) -> Result<(), InputError> {
    for (i, s) in items.iter().enumerate() {
        check_len(&format!("{key}[{i}]"), s, 1, 80)?;
    }
    Ok(())
}

fn valid_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let primary_ok = parts
        .next()
        .is_some_and(|p| (2..=3).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphabetic()));
    primary_ok
        && parts.all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn parse_input_toml(text: &str) -> Result<InputFile, InputError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| InputError::Parse(e.to_string()))?;
    let b = raw.business;

    check_len("business.name", &b.name, 1, 80)?;
    if normalize_url(&b.url).is_none() {
        return Err(invalid("business.url", "must be an absolute http(s) URL"));
    }
    if !valid_language_tag(&b.language) {
        return Err(invalid(
            "business.language",
            "must be a BCP 47 tag such as pt-BR",
        ));
    }
    if b.locations.len() != 1 {
        return Err(invalid(
            "business.locations",
            "exactly 1 location is supported in v1",
        ));
    }
    check_len("business.locations[0]", &b.locations[0], 1, 80)?;
    check_len("business.goal", &b.goal, 1, 200)?;
    check_len("business.description", &b.description, 20, 4000)?;
    check_list("business.competitors", &b.competitors)?;
    check_list("business.avoid", &b.avoid)?;
    let brand_terms = match b.brand_terms {
        Some(t) => {
            check_list("business.brand_terms", &t)?;
            t
        }
        None => vec![b.name.trim().to_lowercase()],
    };
    if b.pages.len() > 20 {
        return Err(invalid("business.pages", "at most 20 pages"));
    }
    let mut pages = Vec::new();
    for (i, p) in b.pages.into_iter().enumerate() {
        check_len(&format!("business.pages[{i}].name"), &p.name, 1, 80)?;
        if normalize_url(&p.url).is_none() {
            return Err(invalid(
                &format!("business.pages[{i}].url"),
                "must be an absolute http(s) URL",
            ));
        }
        pages.push(Page {
            name: p.name,
            url: p.url,
        });
    }

    let daily = Cents::from_f64(raw.budget.daily).ok_or_else(|| {
        invalid(
            "budget.daily",
            "must be greater than 0 with at most 2 decimals",
        )
    })?;
    let c = &raw.budget.currency;
    if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_uppercase()) {
        return Err(invalid("budget.currency", "must be 3 uppercase letters"));
    }
    let max_cpc = match raw.budget.max_cpc {
        Some(v) => Some(Cents::from_f64(v).ok_or_else(|| {
            invalid(
                "budget.max_cpc",
                "must be greater than 0 with at most 2 decimals",
            )
        })?),
        None => None,
    };

    let b_tracking = b.conversion_tracking;
    let primary = b.language.split('-').next().unwrap_or("").to_lowercase();
    let comma_default = matches!(primary.as_str(), "pt" | "es" | "fr" | "de" | "it");
    let e = raw.export;
    let export = ExportConfig {
        status: e.status.unwrap_or(ExportStatus::Paused),
        url_suffix: e
            .url_suffix
            .unwrap_or_else(|| DEFAULT_URL_SUFFIX.to_string()),
        eu_political_ads: e.eu_political_ads.unwrap_or(false),
        decimal_comma: e.decimal_comma.unwrap_or(comma_default),
    };

    Ok(InputFile {
        business: Business {
            name: b.name,
            url: b.url,
            language: b.language,
            locations: b.locations,
            goal: b.goal,
            description: b.description,
            conversion_tracking: b.conversion_tracking,
            brand_terms,
            competitors: b.competitors,
            avoid: b.avoid,
            pages,
            restricted: b.restricted,
        },
        budget: Budget {
            daily,
            currency: raw.budget.currency,
            max_cpc,
        },
        export,
        catalog_file: raw.catalog.map(|c| c.file),
        research_file: raw.research.map(|r| r.file),
        logo_file: raw.brand.as_ref().map(|b| b.logo.clone()),
        formats: check_formats(raw.campaigns, raw.brand.is_some(), b_tracking)?,
        design_file: raw.design.map(|d| d.file),
        focus: check_focus(raw.focus)?,
        app: check_app(raw.app)?,
        google_ads: check_google_ads(raw.google_ads)?,
    })
}

fn check_google_ads(raw: RawGoogleAds) -> Result<GoogleAds, InputError> {
    let customer_id = raw.customer_id.trim().to_string();
    if !customer_id.is_empty() {
        check_customer_id(&customer_id)?;
    }
    let tracking_template = raw.tracking_template.trim().to_string();
    if !tracking_template.is_empty() {
        check_len("google_ads.tracking_template", &tracking_template, 1, 2048)?;
    }
    let devices = raw.devices.trim().to_string();
    if !devices.is_empty() {
        check_len("google_ads.devices", &devices, 1, 80)?;
    }
    check_list("google_ads.labels", &raw.labels)?;
    let labels: Vec<String> = raw.labels.iter().map(|l| l.trim().to_string()).collect();
    if let Some(i) = labels.iter().position(|l| l.contains(';')) {
        return Err(invalid(
            &format!("google_ads.labels[{i}]"),
            "Editor separates labels with ';', so one label cannot hold it",
        ));
    }
    let app_id = raw.app_id.trim().to_string();
    if !app_id.is_empty() {
        check_drive_app_id(&app_id)?;
    }
    Ok(GoogleAds {
        customer_id,
        tracking_template,
        devices,
        labels,
        app_id,
    })
}

/// 10 digits, or `123-456-7890`. All zeros is the placeholder Vaz's sheet used.
fn check_customer_id(id: &str) -> Result<(), InputError> {
    let key = "google_ads.customer_id";
    let dashed = id.len() == 12
        && id.chars().enumerate().all(|(i, c)| match i {
            3 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    let plain = id.len() == 10 && id.chars().all(|c| c.is_ascii_digit());
    if !(dashed || plain) {
        return Err(invalid(key, "use 10 digits or 123-456-7890"));
    }
    if id.chars().all(|c| c == '0' || c == '-') {
        return Err(invalid(
            key,
            "000-000-0000 is a placeholder: leave customer_id empty or set the real account id",
        ));
    }
    Ok(())
}

fn check_drive_app_id(id: &str) -> Result<(), InputError> {
    let play = id.contains('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
    let apple = id.chars().all(|c| c.is_ascii_digit());
    if play || apple {
        Ok(())
    } else {
        Err(invalid(
            "google_ads.app_id",
            "Google Play takes a package name such as com.example.app, the App Store a numeric id",
        ))
    }
}

/// `[campaigns] formats`: known, distinct, and possible with this file.
fn check_formats(
    raw: Option<RawCampaigns>,
    has_logo: bool,
    conversion_tracking: bool,
) -> Result<Vec<CampaignKind>, InputError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let key = "campaigns.formats";
    if raw.formats.is_empty() {
        return Err(invalid(
            key,
            "list at least one format, or remove [campaigns]",
        ));
    }
    let mut seen = Vec::new();
    for f in raw.formats {
        if seen.contains(&f) {
            return Err(invalid(key, format!("{} is listed twice", f.label())));
        }
        if f.has_images() && !has_logo {
            return Err(invalid(key, format!("{} needs [brand] logo", f.label())));
        }
        if f == CampaignKind::PerformanceMax && !conversion_tracking {
            return Err(invalid(
                key,
                "Performance Max needs business.conversion_tracking = true",
            ));
        }
        seen.push(f);
    }
    Ok(seen)
}

/// Reads `business.toml` and, when configured, the catalog next to it.
pub fn load_input(path: &Path) -> Result<Input, InputError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| InputError::Io(format!("{}: {e}", path.display())))?;
    let file = parse_input_toml(&text)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let catalog = match &file.catalog_file {
        Some(rel) => {
            let p = dir.join(rel);
            let bytes =
                std::fs::read(&p).map_err(|e| InputError::Io(format!("{}: {e}", p.display())))?;
            parse_catalog(&bytes)?
        }
        None => Vec::new(),
    };
    let research = match &file.research_file {
        Some(rel) => load_research(&dir.join(rel))?,
        None => String::new(),
    };
    let design = match &file.design_file {
        Some(rel) => load_text(&dir.join(rel), "design.file", DESIGN_MAX_CHARS)?,
        None => String::new(),
    };
    let logo = match &file.logo_file {
        Some(rel) => Some(load_logo(&dir.join(rel))?),
        None => None,
    };
    Ok(Input {
        business: file.business,
        budget: file.budget,
        export: file.export,
        catalog,
        research,
        logo,
        formats: file.formats,
        design,
        focus: file.focus,
        app: file.app,
        google_ads: file.google_ads,
    })
}

/// Checks the logo against Google's limits and returns its absolute path.
fn load_logo(path: &Path) -> Result<String, InputError> {
    let bytes =
        std::fs::read(path).map_err(|e| InputError::Io(format!("{}: {e}", path.display())))?;
    crate::images::check_logo(&bytes)
        .map_err(|e| invalid("brand.logo", format!("{}: {e}", path.display())))?;
    let abs = std::path::absolute(path)
        .map_err(|e| InputError::Io(format!("{}: {e}", path.display())))?;
    Ok(abs.display().to_string())
}

fn load_research(path: &Path) -> Result<String, InputError> {
    load_text(path, "research.file", RESEARCH_MAX_CHARS)
}

/// A Markdown file that goes into prompts, so it has a size limit.
fn load_text(path: &Path, key: &str, max: usize) -> Result<String, InputError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| InputError::Io(format!("{}: {e}", path.display())))?;
    let n = text.chars().count();
    if n > max {
        return Err(invalid(
            key,
            format!("{} has {n} chars, at most {max}", path.display()),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
[business]
name = "Vinellu"
url = "https://vinellu.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "cadastros no app"
description = "App social de vinhos com reviews, safras e harmonização."

[budget]
daily = 50
currency = "BRL"
"#;

    fn with(replace_from: &str, replace_to: &str) -> String {
        MINIMAL.replace(replace_from, replace_to)
    }

    fn err_key(toml: &str) -> String {
        match parse_input_toml(toml) {
            Err(InputError::Invalid { key, .. }) => key,
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn minimal_applies_defaults() {
        let f = parse_input_toml(MINIMAL).unwrap();
        assert_eq!(f.business.brand_terms, vec!["vinellu"]);
        assert_eq!(f.budget.daily, Cents(5000));
        assert_eq!(f.export.status, ExportStatus::Paused);
        assert!(f.export.decimal_comma);
        assert!(!f.export.eu_political_ads);
        assert!(f.export.url_suffix.contains("{mads_campaign}"));
        assert!(!f.business.conversion_tracking);
        assert_eq!(f.catalog_file, None);
    }

    #[test]
    fn decimal_comma_defaults_by_language() {
        let f = parse_input_toml(&with("pt-BR", "en-US")).unwrap();
        assert!(!f.export.decimal_comma);
        let f = parse_input_toml(&with("pt-BR", "es-AR")).unwrap();
        assert!(f.export.decimal_comma);
    }

    #[test]
    fn explicit_decimal_comma_wins() {
        let toml = format!("{MINIMAL}\n[export]\ndecimal_comma = false\n");
        assert!(!parse_input_toml(&toml).unwrap().export.decimal_comma);
    }

    #[test]
    fn unknown_key_is_rejected() {
        let toml = with("goal =", "gooal = \"x\"\ngoal =");
        assert!(matches!(parse_input_toml(&toml), Err(InputError::Parse(_))));
    }

    #[test]
    fn two_locations_rejected() {
        assert_eq!(
            err_key(&with("[\"Brazil\"]", "[\"Brazil\", \"Chile\"]")),
            "business.locations"
        );
    }

    #[test]
    fn empty_name_rejected() {
        assert_eq!(err_key(&with("\"Vinellu\"", "\"  \"")), "business.name");
    }

    #[test]
    fn relative_url_rejected() {
        assert_eq!(
            err_key(&with("https://vinellu.com", "vinellu.com")),
            "business.url"
        );
    }

    #[test]
    fn short_description_rejected() {
        let toml = with(
            "App social de vinhos com reviews, safras e harmonização.",
            "curto",
        );
        assert_eq!(err_key(&toml), "business.description");
    }

    #[test]
    fn budget_with_three_decimals_rejected() {
        assert_eq!(
            err_key(&with("daily = 50", "daily = 50.123")),
            "budget.daily"
        );
    }

    #[test]
    fn zero_budget_rejected() {
        assert_eq!(err_key(&with("daily = 50", "daily = 0")), "budget.daily");
    }

    #[test]
    fn lowercase_currency_rejected() {
        assert_eq!(err_key(&with("BRL", "brl")), "budget.currency");
    }

    #[test]
    fn bad_language_tag_rejected() {
        assert_eq!(
            err_key(&with("pt-BR", "portugues do brasil")),
            "business.language"
        );
    }

    #[test]
    fn page_url_must_be_absolute() {
        let toml = MINIMAL.replace(
            "[budget]",
            "[[business.pages]]\nname = \"App\"\nurl = \"/app\"\n\n[budget]",
        );
        assert_eq!(err_key(&toml), "business.pages[0].url");
    }

    #[test]
    fn max_cpc_must_have_two_decimals_at_most() {
        let toml = MINIMAL.replace("currency = \"BRL\"", "currency = \"BRL\"\nmax_cpc = 1.234");
        assert_eq!(err_key(&toml), "budget.max_cpc");
    }

    #[test]
    fn google_ads_is_optional_and_rejects_the_placeholder_customer_id() {
        let f = parse_input_toml(MINIMAL).unwrap();
        assert!(f.google_ads.customer_id.is_empty());
        assert!(f.google_ads.app_id.is_empty());
        let toml = format!(
            "{MINIMAL}\n[google_ads]\ncustomer_id = \"123-456-7890\"\ndevices = \"Mobile;Desktop;Tablet\"\nlabels = [\"estrutural\"]\napp_id = \"com.vinellu.app\"\ntracking_template = \"{{lpurl}}?utm_campaign={{mads_campaign}}\"\n"
        );
        let f = parse_input_toml(&toml).unwrap();
        assert_eq!(f.google_ads.customer_id, "123-456-7890");
        assert_eq!(f.google_ads.labels, vec!["estrutural"]);
        assert_eq!(f.google_ads.app_id, "com.vinellu.app");
        let placeholder = format!("{MINIMAL}\n[google_ads]\ncustomer_id = \"000-000-0000\"\n");
        assert_eq!(err_key(&placeholder), "google_ads.customer_id");
        let joined = format!("{MINIMAL}\n[google_ads]\nlabels = [\"ok\", \"a;b\"]\n");
        assert_eq!(
            err_key(&joined),
            "google_ads.labels[1]",
            "Editor splits labels on ';'"
        );
        let padded = format!("{MINIMAL}\n[google_ads]\nlabels = [\"  estrutural \"]\n");
        assert_eq!(
            parse_input_toml(&padded).unwrap().google_ads.labels,
            vec!["estrutural"]
        );
    }

    #[test]
    fn customer_id_takes_ten_digits_plain_or_dashed() {
        for ok in ["1234567890", "123-456-7890"] {
            assert!(check_customer_id(ok).is_ok(), "{ok}");
        }
        for bad in [
            "123456789",
            "12345678901",
            "123-456-789a",
            "12-3456-7890",
            "1234-56-7890",
            "123-456-78900",
            "123--456-789",
            "abc-def-ghij",
            "123 456 7890",
            "0000000000",
            "000-000-0000",
            "１２３-456-7890",
        ] {
            assert!(check_customer_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn catalog_file_is_read_from_toml() {
        let toml = format!("{MINIMAL}\n[catalog]\nfile = \"catalog.csv\"\n");
        assert_eq!(
            parse_input_toml(&toml).unwrap().catalog_file.as_deref(),
            Some("catalog.csv")
        );
    }

    #[test]
    fn load_input_reads_catalog_relative_to_the_toml() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[catalog]\nfile = \"c.csv\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        std::fs::write(
            dir.path().join("c.csv"),
            "name,url\nAlamos,https://vinellu.com/w/a\n",
        )
        .unwrap();
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        assert_eq!(input.catalog.len(), 1);
        assert_eq!(input.catalog[0].id, "alamos");
    }

    #[test]
    fn load_input_reads_the_research_notes_relative_to_the_toml() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[research]\nfile = \"notes.md\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        std::fs::write(
            dir.path().join("notes.md"),
            "# Research\n\nPeople search labels.",
        )
        .unwrap();
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        assert_eq!(input.research, "# Research\n\nPeople search labels.");
    }

    #[test]
    fn without_research_the_notes_are_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("business.toml"), MINIMAL).unwrap();
        assert_eq!(
            load_input(&dir.path().join("business.toml"))
                .unwrap()
                .research,
            ""
        );
    }

    #[test]
    fn a_missing_research_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[research]\nfile = \"gone.md\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        assert!(matches!(
            load_input(&dir.path().join("business.toml")),
            Err(InputError::Io(_))
        ));
    }

    #[test]
    fn research_notes_over_the_limit_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[research]\nfile = \"notes.md\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        std::fs::write(
            dir.path().join("notes.md"),
            "x".repeat(RESEARCH_MAX_CHARS + 1),
        )
        .unwrap();
        match load_input(&dir.path().join("business.toml")) {
            Err(InputError::Invalid { key, .. }) => assert_eq!(key, "research.file"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_workspace_written_before_research_existed_still_loads() {
        let mut v = serde_json::to_value(crate::testutil::input()).unwrap();
        v.as_object_mut().unwrap().remove("research");
        let input: Input = serde_json::from_value(v).unwrap();
        assert_eq!(input.research, "");
    }

    fn logo_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn load_input_checks_the_logo_and_stores_its_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[brand]\nlogo = \"brand/logo.png\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        std::fs::create_dir(dir.path().join("brand")).unwrap();
        std::fs::write(dir.path().join("brand/logo.png"), logo_png(300, 300)).unwrap();
        let input = load_input(&dir.path().join("business.toml")).unwrap();
        let logo = input.logo.unwrap();
        assert!(Path::new(&logo).is_absolute());
        assert!(logo.ends_with("brand/logo.png"));
    }

    #[test]
    fn a_logo_that_is_not_square_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let toml = format!("{MINIMAL}\n[brand]\nlogo = \"logo.png\"\n");
        std::fs::write(dir.path().join("business.toml"), toml).unwrap();
        std::fs::write(dir.path().join("logo.png"), logo_png(400, 200)).unwrap();
        match load_input(&dir.path().join("business.toml")) {
            Err(InputError::Invalid { key, message }) => {
                assert_eq!(key, "brand.logo");
                assert!(message.contains("square"), "{message}");
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn without_brand_there_is_no_logo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("business.toml"), MINIMAL).unwrap();
        assert_eq!(
            load_input(&dir.path().join("business.toml")).unwrap().logo,
            None
        );
    }

    #[test]
    fn restricted_categories_are_optional_and_known() {
        assert!(
            parse_input_toml(MINIMAL)
                .unwrap()
                .business
                .restricted
                .is_empty()
        );
        let toml = MINIMAL.replace("goal =", "restricted = [\"alcohol\"]\ngoal =");
        assert_eq!(
            parse_input_toml(&toml).unwrap().business.restricted,
            [RestrictedCategory::Alcohol]
        );
        let unknown = MINIMAL.replace("goal =", "restricted = [\"wine\"]\ngoal =");
        assert!(matches!(
            parse_input_toml(&unknown),
            Err(InputError::Parse(_))
        ));
    }

    #[test]
    fn app_is_optional_and_checked() {
        assert!(parse_input_toml(MINIMAL).unwrap().app.is_none());
        let play = format!("{MINIMAL}\n[app]\nstore = \"google_play\"\nid = \"com.vinellu.app\"\n");
        let app = parse_input_toml(&play).unwrap().app.unwrap();
        assert_eq!(
            app.store_url(),
            "https://play.google.com/store/apps/details?id=com.vinellu.app"
        );
        assert_eq!(app.store_label(), "Google Play");
        let apple = format!("{MINIMAL}\n[app]\nstore = \"app_store\"\nid = \"1234567\"\n");
        assert_eq!(
            parse_input_toml(&apple).unwrap().app.unwrap().store_url(),
            "https://apps.apple.com/app/id1234567"
        );
        let bad = format!("{MINIMAL}\n[app]\nstore = \"app_store\"\nid = \"com.x\"\n");
        assert_eq!(err_key(&bad), "app.id");
    }

    #[test]
    fn store_links_name_the_app() {
        let play = App::from_store_link(
            "https://play.google.com/store/apps/details?id=com.vinellu.app&hl=pt_BR",
        )
        .unwrap();
        assert_eq!(
            (play.store, play.id.as_str()),
            (AppStore::GooglePlay, "com.vinellu.app")
        );
        let apple =
            App::from_store_link("https://apps.apple.com/br/app/vinellu/id6748123456").unwrap();
        assert_eq!(
            (apple.store, apple.id.as_str()),
            (AppStore::AppStore, "6748123456")
        );
        assert!(App::from_store_link("https://play.google.com/store/apps/dev?id=123").is_none());
        assert!(App::from_store_link("https://vinellu.com/app").is_none());
    }

    #[test]
    fn focus_is_optional_and_checked() {
        assert!(parse_input_toml(MINIMAL).unwrap().focus.is_none());
        let ok = format!(
            "{MINIMAL}\n[focus]\nname = \"BH <> SP\"\nurls = [\"https://vinellu.com/a\", \"https://vinellu.com/b\"]\n"
        );
        let f = parse_input_toml(&ok).unwrap().focus.unwrap();
        assert_eq!((f.name.as_str(), f.urls.len()), ("BH <> SP", 2));
        for bad in [
            "[focus]\nname = \"x\"\nurls = []\n",
            "[focus]\nname = \"x\"\nurls = [\"/a\"]\n",
        ] {
            assert_eq!(err_key(&format!("{MINIMAL}\n{bad}")), "focus.urls");
        }
        let terms = format!(
            "{MINIMAL}\n[focus]\nname = \"x\"\nurls = [\"https://x.com\"]\nterms = [[\"bh\", \"belo horizonte\"], [\"sp\"]]\n"
        );
        assert_eq!(
            parse_input_toml(&terms).unwrap().focus.unwrap().terms.len(),
            2
        );
        let empty_group =
            format!("{MINIMAL}\n[focus]\nname = \"x\"\nurls = [\"https://x.com\"]\nterms = [[]]\n");
        assert_eq!(err_key(&empty_group), "focus.terms");
        let no_name = format!("{MINIMAL}\n[focus]\nname = \"\"\nurls = [\"https://x.com\"]\n");
        assert_eq!(err_key(&no_name), "focus.name");
    }

    #[test]
    fn formats_are_optional_and_checked() {
        assert!(parse_input_toml(MINIMAL).unwrap().formats.is_empty());
        let brand = "\n[brand]\nlogo = \"logo.png\"\n";
        let ok = format!("{MINIMAL}{brand}\n[campaigns]\nformats = [\"search\", \"demand_gen\"]\n");
        assert_eq!(
            parse_input_toml(&ok).unwrap().formats,
            [CampaignKind::Search, CampaignKind::DemandGen]
        );
        let cases = [
            format!("{MINIMAL}\n[campaigns]\nformats = [\"demand_gen\"]\n"),
            format!("{MINIMAL}{brand}\n[campaigns]\nformats = [\"performance_max\"]\n"),
            format!("{MINIMAL}\n[campaigns]\nformats = [\"search\", \"search\"]\n"),
            format!("{MINIMAL}\n[campaigns]\nformats = []\n"),
        ];
        for toml in cases {
            assert_eq!(err_key(&toml), "campaigns.formats", "{toml}");
        }
        let bad_name = format!("{MINIMAL}\n[campaigns]\nformats = [\"video\"]\n");
        assert!(matches!(
            parse_input_toml(&bad_name),
            Err(InputError::Parse(_))
        ));
    }

    #[test]
    fn load_input_missing_file_is_io_error() {
        let err = load_input(Path::new("/nonexistent/business.toml")).unwrap_err();
        assert!(matches!(err, InputError::Io(_)));
    }
}
