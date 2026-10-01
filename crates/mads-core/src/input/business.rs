use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{CatalogItem, normalize_url, parse_catalog};
use crate::money::Cents;

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

/// Validated `business.toml`. `catalog_file` is still relative to the TOML file.
#[derive(Debug, Clone, PartialEq)]
pub struct InputFile {
    pub business: Business,
    pub budget: Budget,
    pub export: ExportConfig,
    pub catalog_file: Option<String>,
    /// Markdown notes from `mads init`, relative to the TOML file.
    pub research_file: Option<String>,
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
        },
        budget: Budget {
            daily,
            currency: raw.budget.currency,
            max_cpc,
        },
        export,
        catalog_file: raw.catalog.map(|c| c.file),
        research_file: raw.research.map(|r| r.file),
    })
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
    Ok(Input {
        business: file.business,
        budget: file.budget,
        export: file.export,
        catalog,
        research,
    })
}

fn load_research(path: &Path) -> Result<String, InputError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| InputError::Io(format!("{}: {e}", path.display())))?;
    let n = text.chars().count();
    if n > RESEARCH_MAX_CHARS {
        return Err(invalid(
            "research.file",
            format!(
                "{} has {n} chars, at most {RESEARCH_MAX_CHARS}",
                path.display()
            ),
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

    #[test]
    fn load_input_missing_file_is_io_error() {
        let err = load_input(Path::new("/nonexistent/business.toml")).unwrap_err();
        assert!(matches!(err, InputError::Io(_)));
    }
}
