use super::{Account, AdGroup, BidStrategy, Campaign, Keyword, MatchType, merge_rsa, normalize};
use crate::input::Input;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvFile {
    pub name: &'static str,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    #[error("brand kit is missing")]
    MissingBrandKit,
    #[error("{0} needs a Google Ads bulk upload template that is not verified yet")]
    PendingTemplate(String),
    #[error("csv write failed: {0}")]
    Csv(String),
    #[error("image campaigns need a logo")]
    MissingLogo,
    #[error("image '{0}' has no file")]
    MissingImage(String),
}

impl From<csv::Error> for ExportError {
    fn from(e: csv::Error) -> Self {
        ExportError::Csv(e.to_string())
    }
}

const CAMPAIGN_HEADER: &[&str] = &[
    "Row Type",
    "Action",
    "Campaign status",
    "Campaign",
    "Campaign type",
    "Networks",
    "Budget",
    "Budget type",
    "Bid strategy type",
    "Language",
    "Location",
    "Final URL suffix",
    "EU political ads",
];
const AD_GROUP_HEADER: &[&str] = &[
    "Row Type",
    "Action",
    "Ad group status",
    "Campaign",
    "Ad group",
    "Ad group type",
    "Default max. CPC",
];
const KEYWORD_HEADER: &[&str] = &[
    "Row Type",
    "Action",
    "Keyword status",
    "Campaign",
    "Ad group",
    "Keyword",
    "Type",
];
const NEGATIVE_HEADER: &[&str] = &[
    "Row Type",
    "Action",
    "Keyword status",
    "Level",
    "Campaign",
    "Ad group",
    "Negative keyword",
    "Type",
];

pub(crate) struct Sheet {
    writer: csv::Writer<Vec<u8>>,
}

impl Sheet {
    pub(crate) fn new<S: AsRef<[u8]>>(header: &[S]) -> Result<Self, ExportError> {
        let mut writer = csv::WriterBuilder::new()
            .terminator(csv::Terminator::CRLF)
            .quote_style(csv::QuoteStyle::Necessary)
            .from_writer(Vec::new());
        writer.write_record(header)?;
        Ok(Self { writer })
    }

    pub(crate) fn row<I, S>(&mut self, fields: I) -> Result<(), ExportError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<[u8]>,
    {
        Ok(self.writer.write_record(fields)?)
    }

    pub(crate) fn finish(self, name: &'static str) -> Result<CsvFile, ExportError> {
        let bytes = self
            .writer
            .into_inner()
            .map_err(|e| ExportError::Csv(e.to_string()))?;
        Ok(CsvFile { name, bytes })
    }
}

/// Search campaigns export only Manual CPC until the bulk templates of the others are verified.
fn bid_label(c: &Campaign) -> Result<&'static str, ExportError> {
    match c.bid_strategy {
        BidStrategy::ManualCpc => Ok(c.bid_strategy.label()),
        other => Err(ExportError::PendingTemplate(other.label().into())),
    }
}

fn match_label(m: MatchType) -> &'static str {
    match m {
        MatchType::Phrase => "Phrase match",
        MatchType::Exact => "Exact match",
    }
}

/// Negatives of one ad group as rows: campaign level first, then the ad group's own, no duplicates.
/// Campaign-level rows need a bulk template that is not verified, so they are expanded per ad group.
fn effective_negatives<'a>(c: &'a Campaign, ag: &'a AdGroup) -> Vec<&'a Keyword> {
    let mut seen = std::collections::BTreeSet::new();
    c.negatives
        .iter()
        .chain(&ag.negatives)
        .filter(|n| seen.insert((normalize(&n.text), n.match_type == MatchType::Exact)))
        .collect()
}

/// Files 1 to 5 of the Google Ads bulk upload set, in upload order. Search campaigns only:
/// without one there is no file.
pub fn export_csvs(input: &Input, account: &Account) -> Result<Vec<CsvFile>, ExportError> {
    if !account.campaigns.iter().any(|c| !c.kind.has_images()) {
        return Ok(Vec::new());
    }
    let kit = account
        .brand_kit
        .as_ref()
        .ok_or(ExportError::MissingBrandKit)?;
    let comma = input.export.decimal_comma;
    let (status, language) = (
        input.export.status.as_str(),
        input.business.language_primary(),
    );
    let location = input.business.locations.first().map_or("", String::as_str);
    let eu = if input.export.eu_political_ads {
        "Yes"
    } else {
        "No"
    };

    let mut campaigns = Sheet::new(CAMPAIGN_HEADER)?;
    let mut groups = Sheet::new(AD_GROUP_HEADER)?;
    let mut keywords = Sheet::new(KEYWORD_HEADER)?;
    let mut negatives = Sheet::new(NEGATIVE_HEADER)?;
    let mut ads = Sheet::new(&ads_header())?;

    for c in account.campaigns.iter().filter(|c| !c.kind.has_images()) {
        let suffix = input.export.url_suffix.replace("{mads_campaign}", &c.slug);
        let budget = c.daily_budget.format_budget(comma);
        campaigns.row([
            "Campaign",
            "Add",
            status,
            &c.name,
            "Search",
            "Google search",
            &budget,
            "Daily",
            bid_label(c)?,
            language,
            location,
            &suffix,
            eu,
        ])?;
        for ag in &c.ad_groups {
            groups.row([
                "Ad group",
                "Add",
                "Enabled",
                &c.name,
                &ag.name,
                "Standard",
                &ag.default_cpc.format_cpc(comma),
            ])?;
            for k in &ag.keywords {
                keywords.row([
                    "Keyword",
                    "Add",
                    "Enabled",
                    &c.name,
                    &ag.name,
                    &k.text,
                    match_label(k.match_type),
                ])?;
            }
            for n in effective_negatives(c, ag) {
                negatives.row([
                    "Negative keyword",
                    "Add",
                    "Enabled",
                    "Ad group",
                    &c.name,
                    &ag.name,
                    &n.text,
                    match_label(n.match_type),
                ])?;
            }
            ads.row(ad_row(&c.name, ag, &merge_rsa(&ag.rsa, kit)))?;
        }
    }
    Ok(vec![
        campaigns.finish("1-campaign.csv")?,
        groups.finish("2-ad-groups.csv")?,
        keywords.finish("3-keywords.csv")?,
        negatives.finish("4-negative-keywords.csv")?,
        ads.finish("5-responsive-search-ads.csv")?,
    ])
}

fn ads_header() -> Vec<String> {
    let mut h: Vec<String> = [
        "Row Type",
        "Action",
        "Ad status",
        "Campaign",
        "Ad group",
        "Ad type",
    ]
    .map(String::from)
    .into();
    h.extend((1..=15).map(|i| format!("Headline {i}")));
    h.extend((1..=4).map(|i| format!("Description {i}")));
    h.extend(["Path 1", "Path 2", "Final URL"].map(String::from));
    h
}

fn ad_row(campaign: &str, ag: &AdGroup, ad: &super::MergedRsa) -> Vec<String> {
    let mut row: Vec<String> = [
        "Ad",
        "Add",
        "Enabled",
        campaign,
        &ag.name,
        "Responsive search ad",
    ]
    .map(String::from)
    .into();
    row.extend((0..15).map(|i| ad.headlines.get(i).cloned().unwrap_or_default()));
    row.extend((0..4).map(|i| ad.descriptions.get(i).cloned().unwrap_or_default()));
    row.push(ad.path1.clone().unwrap_or_default());
    row.push(ad.path2.clone().unwrap_or_default());
    row.push(ag.final_url.clone());
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::*;
    use crate::input::{Budget, Business, ExportConfig, ExportStatus, Input};

    fn input() -> Input {
        Input {
            logo: None,
            formats: Vec::new(),
            design: String::new(),
            focus: None,
            business: Business {
                name: "Acme".into(),
                url: "https://acme.com".into(),
                language: "en-US".into(),
                locations: vec!["United States".into()],
                goal: "g".into(),
                description: "d".repeat(30),
                conversion_tracking: false,
                brand_terms: vec!["acme".into()],
                competitors: vec![],
                avoid: vec![],
                pages: vec![],
            },
            budget: Budget {
                daily: Cents(3750),
                currency: "USD".into(),
                max_cpc: None,
            },
            export: ExportConfig {
                status: ExportStatus::Enabled,
                url_suffix: "utm_campaign={mads_campaign}&utm_term={keyword}".into(),
                eu_political_ads: true,
                decimal_comma: false,
            },
            research: String::new(),
            catalog: vec![],
        }
    }

    fn kw(t: &str, m: MatchType) -> Keyword {
        Keyword {
            text: t.into(),
            match_type: m,
        }
    }

    fn account() -> Account {
        Account {
            brand_kit: Some(BrandKit {
                headlines: vec!["Kit headline".into()],
                descriptions: vec!["Kit, with comma".into()],
            }),
            campaigns: vec![Campaign {
                kind: Default::default(),
                asset_groups: Vec::new(),
                name: "Acme - Brand".into(),
                slug: "acme-brand".into(),
                intent: Intent::Brand,
                daily_budget: Cents(3750),
                bid_strategy: BidStrategy::ManualCpc,
                rationale: String::new(),
                planned_ad_groups: vec![],
                ad_groups: vec![AdGroup {
                    name: "g1".into(),
                    default_cpc: Cents(150),
                    cpc_rationale: String::new(),
                    final_url: "https://acme.com/x".into(),
                    keywords: vec![kw("acme", MatchType::Phrase)],
                    negatives: vec![kw("free", MatchType::Phrase)],
                    rsa: Rsa {
                        headlines: vec!["H1".into()],
                        descriptions: vec!["D1".into()],
                        path1: None,
                        path2: None,
                    },
                }],
                negatives: vec![kw("jobs", MatchType::Phrase), kw("free", MatchType::Phrase)],
                assets: None,
            }],
        }
    }

    fn file<'a>(files: &'a [CsvFile], name: &str) -> &'a str {
        std::str::from_utf8(&files.iter().find(|f| f.name == name).unwrap().bytes).unwrap()
    }

    #[test]
    fn campaign_row_uses_config_and_replaces_slug_placeholder() {
        let files = export_csvs(&input(), &account()).unwrap();
        let rows: Vec<&str> = file(&files, "1-campaign.csv").split("\r\n").collect();
        assert_eq!(
            rows[1],
            "Campaign,Add,Enabled,Acme - Brand,Search,Google search,37.50,Daily,Manual CPC,en,United States,utm_campaign=acme-brand&utm_term={keyword},Yes"
        );
    }

    #[test]
    fn every_record_ends_with_crlf_and_there_is_no_bom() {
        let files = export_csvs(&input(), &account()).unwrap();
        for f in &files {
            assert!(f.bytes.ends_with(b"\r\n"), "{}", f.name);
            assert!(!f.bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "{}", f.name);
            assert!(
                !String::from_utf8_lossy(&f.bytes)
                    .replace("\r\n", "")
                    .contains('\n'),
                "{}",
                f.name
            );
        }
    }

    #[test]
    fn campaign_negatives_are_expanded_into_ad_group_rows_without_duplicates() {
        let files = export_csvs(&input(), &account()).unwrap();
        let text = file(&files, "4-negative-keywords.csv");
        let rows: Vec<&str> = text.trim_end().split("\r\n").skip(1).collect();
        assert_eq!(
            rows,
            [
                "Negative keyword,Add,Enabled,Ad group,Acme - Brand,g1,jobs,Phrase match",
                "Negative keyword,Add,Enabled,Ad group,Acme - Brand,g1,free,Phrase match",
            ]
        );
    }

    #[test]
    fn rsa_row_merges_brand_kit_and_leaves_unused_cells_empty() {
        let files = export_csvs(&input(), &account()).unwrap();
        let text = file(&files, "5-responsive-search-ads.csv");
        let row = text.split("\r\n").nth(1).unwrap();
        assert!(
            row.starts_with("Ad,Add,Enabled,Acme - Brand,g1,Responsive search ad,"),
            "{row}"
        );
        assert!(row.ends_with("https://acme.com/x"));
        assert!(row.contains("Kit headline"));
        assert!(row.contains("\"Kit, with comma\""));
    }

    #[test]
    fn cpc_uses_decimal_setting() {
        let files = export_csvs(&input(), &account()).unwrap();
        assert!(file(&files, "2-ad-groups.csv").contains("Standard,1.50"));
        let mut inp = input();
        inp.export.decimal_comma = true;
        let files = export_csvs(&inp, &account()).unwrap();
        assert!(file(&files, "2-ad-groups.csv").contains("Standard,\"1,50\""));
    }

    #[test]
    fn missing_brand_kit_is_an_error() {
        let mut a = account();
        a.brand_kit = None;
        assert_eq!(
            export_csvs(&input(), &a).unwrap_err(),
            ExportError::MissingBrandKit
        );
    }

    #[test]
    fn bid_strategies_without_a_verified_template_are_rejected() {
        let mut a = account();
        a.campaigns[0].bid_strategy = BidStrategy::MaximizeConversions;
        assert!(matches!(
            export_csvs(&input(), &a).unwrap_err(),
            ExportError::PendingTemplate(_)
        ));
        a.campaigns[0].bid_strategy = BidStrategy::MaximizeClicks { max_cpc: None };
        assert!(matches!(
            export_csvs(&input(), &a).unwrap_err(),
            ExportError::PendingTemplate(_)
        ));
    }
}
