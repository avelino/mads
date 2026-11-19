//! Campaign strategies: a TOML header between `+++` lines plus prose for the agents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::business::{InputError, check_len, invalid};
use super::{CatalogItem, bare_host, normalize_url, page_key};

pub const SHIPPED: [&str; 3] = ["visibility", "profitability", "need"];
pub const STRATEGY_MAX_CHARS: usize = 8_000;
/// The strategy text `mads init --strategy` writes next to `business.toml`.
pub const STRATEGY_FILE: &str = "STRATEGY.md";
const MAX_TOP: u32 = 50;
const MAX_PRODUCTS: usize = 50;

#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum StrategyIntent {
    Purchase,
    Consideration,
    Discovery,
}

impl StrategyIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Purchase => "purchase",
            Self::Consideration => "consideration",
            Self::Discovery => "discovery",
        }
    }
}

/// How the account is run: what the ads are for and how much of the budget the products get.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strategy {
    pub name: String,
    pub intent: StrategyIntent,
    /// Product page URLs, in priority order.
    pub products: Vec<String>,
    /// Percent of the budget the products get.
    pub min_share: u8,
    #[serde(default)]
    pub prose: String,
}

/// Compares product pages: `page_key` without a leading `www.`. The query stays, some shops
/// identify the product with `?id=`.
pub(crate) fn product_key(url: &str) -> Option<String> {
    page_key(url).map(|k| k.replacen("://www.", "://", 1))
}

impl Strategy {
    /// Catalog ids whose page is one of the products, in product order.
    pub fn product_ids(&self, catalog: &[CatalogItem]) -> Vec<String> {
        self.products
            .iter()
            .filter_map(|p| {
                let key = product_key(p)?;
                catalog
                    .iter()
                    .find(|c| product_key(&c.url).as_ref() == Some(&key))
            })
            .map(|c| c.id.clone())
            .collect()
    }
}

pub enum StrategySource<'a> {
    Shipped(&'a str),
    /// `name` is the file stem, used when the header has no `name`.
    File {
        name: &'a str,
        text: &'a str,
    },
}

#[derive(Default)]
pub struct StrategyOverrides {
    pub products: Vec<String>,
    pub top: Option<u32>,
    /// `--focus`: with no products the focus pages are the products.
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedStrategy {
    pub name: String,
    pub intent: StrategyIntent,
    pub products: Vec<String>,
    pub top: Option<u32>,
    pub min_share: u8,
    pub prose: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct RawHeader {
    extends: Option<String>,
    name: Option<String>,
    intent: Option<StrategyIntent>,
    products: Option<Vec<String>>,
    top: Option<u32>,
    min_share: Option<u8>,
}

struct Parsed {
    header: RawHeader,
    body: String,
}

fn shipped_text(name: &str) -> Option<&'static str> {
    match name {
        "visibility" => Some(include_str!("../../prompts/strategies/visibility.md")),
        "profitability" => Some(include_str!("../../prompts/strategies/profitability.md")),
        "need" => Some(include_str!("../../prompts/strategies/need.md")),
        _ => None,
    }
}

fn parse(text: &str) -> Result<Parsed, InputError> {
    let bad = || {
        invalid(
            "strategy",
            "a strategy file starts with a TOML header between +++ lines",
        )
    };
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("+++") {
        return Err(bad());
    }
    let mut header = String::new();
    let mut closed = false;
    let mut body = String::new();
    for line in lines {
        if closed {
            body.push_str(line);
        } else if line.trim_end() == "+++" {
            closed = true;
        } else {
            header.push_str(line);
        }
    }
    if !closed {
        return Err(bad());
    }
    let header = toml::from_str(&header).map_err(|e| invalid("strategy", e.to_string()))?;
    Ok(Parsed { header, body })
}

fn shipped(name: &str) -> Result<Parsed, InputError> {
    let text = shipped_text(name).ok_or_else(|| unknown_base(name))?;
    let parsed = parse(text)?;
    if parsed.header.extends.is_some() {
        return Err(invalid(
            "strategy.extends",
            "a shipped strategy cannot extend another",
        ));
    }
    Ok(parsed)
}

fn unknown_base(name: &str) -> InputError {
    invalid(
        "strategy.extends",
        format!(
            "`{name}` is not a shipped strategy, use one of: {}",
            SHIPPED.join(", ")
        ),
    )
}

fn join_prose(base: &str, child: &str) -> String {
    match (base.trim(), child.trim()) {
        ("", c) => c.to_string(),
        (b, "") => b.to_string(),
        (b, c) => format!("{b}\n\n{c}"),
    }
}

fn merge(base: RawHeader, child: RawHeader) -> RawHeader {
    RawHeader {
        extends: child.extends,
        name: child.name.or(base.name),
        intent: child.intent.or(base.intent),
        products: child.products.or(base.products),
        top: child.top.or(base.top),
        min_share: child.min_share.or(base.min_share),
    }
}

fn load(src: &StrategySource) -> Result<(String, RawHeader, String), InputError> {
    match src {
        StrategySource::Shipped(name) => {
            let p = shipped(name)?;
            Ok((name.to_string(), p.header, p.body))
        }
        StrategySource::File { name, text } => {
            let p = parse(text)?;
            let Some(base) = p.header.extends.clone() else {
                return Ok((name.to_string(), p.header, p.body));
            };
            if !SHIPPED.contains(&base.as_str()) {
                return Err(unknown_base(&base));
            }
            let b = shipped(&base)?;
            let prose = join_prose(&b.body, &p.body);
            Ok((name.to_string(), merge(b.header, p.header), prose))
        }
    }
}

pub fn resolve_strategy(
    src: StrategySource,
    ov: StrategyOverrides,
) -> Result<ResolvedStrategy, InputError> {
    let (fallback, mut h, prose) = load(&src)?;
    // `need` lists its products: a `top` does not stand in for them.
    let need =
        matches!(src, StrategySource::Shipped("need")) || h.extends.as_deref() == Some("need");
    if !ov.products.is_empty() {
        h.products = Some(ov.products);
    }
    if ov.top.is_some() {
        h.top = ov.top;
    }
    let name = h.name.clone().unwrap_or(fallback);
    let prose = prose.trim().to_string();
    check(h, name, prose, ov.focus, need)
}

fn check(
    h: RawHeader,
    name: String,
    prose: String,
    focus: bool,
    need: bool,
) -> Result<ResolvedStrategy, InputError> {
    check_len("strategy.name", &name, 1, 80)?;
    let intent = h
        .intent
        .ok_or_else(|| invalid("strategy.intent", "intent is required"))?;
    let min_share = h
        .min_share
        .ok_or_else(|| invalid("strategy.min_share", "min_share is required"))?;
    if min_share > 100 {
        return Err(invalid(
            "strategy.min_share",
            "min_share is a percentage, 0 to 100",
        ));
    }
    if h.top.is_some_and(|t| !(1..=MAX_TOP).contains(&t)) {
        return Err(invalid("strategy.top", format!("top is 1 to {MAX_TOP}")));
    }
    let products = h.products.unwrap_or_default();
    check_products(&products)?;
    if products.is_empty() && !focus {
        if need {
            return Err(invalid(
                "strategy.products",
                "need lists its products: give product URLs",
            ));
        }
        if h.top.is_none() {
            return Err(invalid(
                "strategy.products",
                "give product URLs or a top count",
            ));
        }
    }
    if prose.chars().count() > STRATEGY_MAX_CHARS {
        return Err(invalid(
            "strategy.file",
            format!("the strategy text is over {STRATEGY_MAX_CHARS} characters"),
        ));
    }
    Ok(ResolvedStrategy {
        name,
        intent,
        products,
        top: h.top,
        min_share,
        prose,
    })
}

fn check_products(products: &[String]) -> Result<(), InputError> {
    if products.len() > MAX_PRODUCTS {
        return Err(invalid(
            "strategy.products",
            format!("at most {MAX_PRODUCTS} products"),
        ));
    }
    match products.iter().find(|p| normalize_url(p).is_none()) {
        Some(bad) => Err(invalid(
            "strategy.products",
            format!("`{bad}` is not an http(s) URL"),
        )),
        None => Ok(()),
    }
}

/// Every product must sit on the site the run starts from.
pub fn check_product_hosts(start_url: &str, products: &[String]) -> Result<(), InputError> {
    let site = bare_host(start_url);
    match products
        .iter()
        .find(|p| bare_host(p) != site || site.is_none())
    {
        Some(p) => Err(invalid(
            "strategy.products",
            format!("`{p}` is not on the same site as {start_url}"),
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_key(e: InputError) -> String {
        match e {
            InputError::Invalid { key, .. } => key,
            other => panic!("not an Invalid error: {other}"),
        }
    }

    #[test]
    fn shipped_strategies_resolve_with_their_values() {
        let v =
            resolve_strategy(StrategySource::Shipped("visibility"), Default::default()).unwrap();
        assert_eq!(
            (v.intent, v.top, v.min_share),
            (StrategyIntent::Consideration, Some(10), 60)
        );
        let p =
            resolve_strategy(StrategySource::Shipped("profitability"), Default::default()).unwrap();
        assert_eq!(
            (p.intent, p.top, p.min_share),
            (StrategyIntent::Purchase, Some(5), 70)
        );
        let ov = StrategyOverrides {
            products: vec!["https://x.com/a".into()],
            ..Default::default()
        };
        let n = resolve_strategy(StrategySource::Shipped("need"), ov).unwrap();
        assert_eq!(
            (n.intent, n.min_share, n.products.len()),
            (StrategyIntent::Purchase, 80, 1)
        );
        assert!(!n.prose.trim().is_empty());
    }

    #[test]
    fn need_without_products_is_refused() {
        let e = resolve_strategy(StrategySource::Shipped("need"), Default::default()).unwrap_err();
        assert_eq!(err_key(e), "strategy.products");
    }

    #[test]
    fn need_with_a_top_and_no_products_is_refused() {
        let ov = StrategyOverrides {
            top: Some(3),
            ..Default::default()
        };
        let e = resolve_strategy(StrategySource::Shipped("need"), ov).unwrap_err();
        assert_eq!(err_key(e), "strategy.products");
        let text = "+++\nextends = \"need\"\ntop = 3\n+++\n";
        let e = resolve_strategy(StrategySource::File { name: "f", text }, Default::default())
            .unwrap_err();
        assert_eq!(err_key(e), "strategy.products");
    }

    #[test]
    fn with_focus_no_products_and_no_top_resolves() {
        let ov = || StrategyOverrides {
            focus: true,
            ..Default::default()
        };
        let n = resolve_strategy(StrategySource::Shipped("need"), ov()).unwrap();
        assert!(n.products.is_empty() && n.top.is_none());
        let text = "+++\nintent = \"purchase\"\nmin_share = 50\n+++\n";
        resolve_strategy(StrategySource::File { name: "f", text }, ov()).unwrap();
    }

    #[test]
    fn a_file_extends_a_shipped_one_key_by_key_and_appends_its_prose() {
        let text = "+++\nextends = \"profitability\"\nmin_share = 90\n+++\nOnly buyers.\n";
        let s = resolve_strategy(
            StrategySource::File { name: "mine", text },
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            (s.name.as_str(), s.intent, s.top, s.min_share),
            ("mine", StrategyIntent::Purchase, Some(5), 90)
        );
        let base =
            resolve_strategy(StrategySource::Shipped("profitability"), Default::default()).unwrap();
        assert!(
            s.prose.starts_with(base.prose.trim_end())
                && s.prose.trim_end().ends_with("Only buyers.")
        );
    }

    #[test]
    fn flags_override_the_file() {
        let ov = StrategyOverrides {
            products: vec!["https://x.com/b".into()],
            top: Some(3),
            ..Default::default()
        };
        let s = resolve_strategy(StrategySource::Shipped("visibility"), ov).unwrap();
        assert_eq!(
            (s.products, s.top),
            (vec!["https://x.com/b".to_string()], Some(3))
        );
    }

    #[test]
    fn bad_files_name_the_key() {
        let cases = [
            ("Just prose, no header.\n", "strategy"),
            ("+++\nextends = \"nope\"\n+++\n", "strategy.extends"),
            ("+++\nextends = \"./other.md\"\n+++\n", "strategy.extends"),
            ("+++\nmin_share = 50\ntop = 3\n+++\n", "strategy.intent"),
            (
                "+++\nintent = \"purchase\"\ntop = 3\n+++\n",
                "strategy.min_share",
            ),
            (
                "+++\nintent = \"purchase\"\nmin_share = 50\n+++\n",
                "strategy.products",
            ),
            (
                "+++\nintent = \"purchase\"\nmin_share = 101\ntop = 3\n+++\n",
                "strategy.min_share",
            ),
            (
                "+++\nintent = \"purchase\"\nmin_share = 50\ntop = 0\n+++\n",
                "strategy.top",
            ),
            (
                "+++\nintent = \"buy\"\nmin_share = 50\ntop = 3\n+++\n",
                "strategy",
            ),
            (
                "+++\nintent = \"purchase\"\nmin_share = 50\nproducts = [\"/a\"]\n+++\n",
                "strategy.products",
            ),
        ];
        for (text, key) in cases {
            let e = resolve_strategy(StrategySource::File { name: "f", text }, Default::default())
                .unwrap_err();
            assert_eq!(err_key(e), key, "{text}");
        }
    }

    #[test]
    fn the_name_is_checked_when_the_strategy_resolves() {
        let long = "a".repeat(81);
        let headers = ["name = \"\"\n".to_string(), format!("name = \"{long}\"\n")];
        for name in headers {
            let text = format!("+++\n{name}intent = \"purchase\"\nmin_share = 50\ntop = 3\n+++\n");
            let src = StrategySource::File {
                name: "f",
                text: &text,
            };
            let e = resolve_strategy(src, Default::default()).unwrap_err();
            assert_eq!(err_key(e), "strategy.name", "{name}");
        }
        let stem = "s".repeat(81);
        let text = "+++\nintent = \"purchase\"\nmin_share = 50\ntop = 3\n+++\n";
        let src = StrategySource::File { name: &stem, text };
        let e = resolve_strategy(src, Default::default()).unwrap_err();
        assert_eq!(err_key(e), "strategy.name");
    }

    #[test]
    fn prose_over_the_cap_is_refused() {
        let text = format!(
            "+++\nextends = \"visibility\"\n+++\n{}",
            "a".repeat(STRATEGY_MAX_CHARS + 1)
        );
        let e = resolve_strategy(
            StrategySource::File {
                name: "f",
                text: &text,
            },
            Default::default(),
        )
        .unwrap_err();
        assert_eq!(err_key(e), "strategy.file");
    }

    #[test]
    fn products_must_be_on_the_start_host() {
        assert!(
            check_product_hosts("https://www.loja.com/", &["https://loja.com/a".into()]).is_ok()
        );
        let e =
            check_product_hosts("https://loja.com", &["https://other.com/a".into()]).unwrap_err();
        assert_eq!(err_key(e), "strategy.products");
    }
}
