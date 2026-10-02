//! The whole account as one Google Ads Editor import file.
//!
//! The format is the one Editor exports (checked against an export of version 2.13.3): UTF-16 LE
//! with a BOM, tab separated, LF line endings, no row type column (the filled columns say what a
//! row is), money with a dot and 2 decimals. Search and App rows match that export, and a file with
//! Search, App and Demand Gen campaigns imported into Editor 2.13.3 with no error and every entity
//! counted. Performance Max rows follow Google's documented headers, not imported yet. Pictures are
//! not in the file: Editor neither exports nor imports the link between an ad and its images.

use std::collections::BTreeMap;

use super::{
    Account, AdGroup, AssetGroup, BrandKit, Campaign, CampaignKind, CsvFile, MatchType,
    export::effective_negatives, merge_rsa,
};
use crate::{input::Input, money::Cents};

/// Path of the file under the platform directory.
pub const EDITOR_FILE: &str = "editor/account.csv";

const ACTIVE: &str = "Enabled";
/// The bid Editor fills in when a field does not apply to the bid strategy. Every ad group of a
/// real export carries it, and Editor warns "the ad group has no bids" without it.
const MIN_BID: &str = "0.01";

/// Columns in the order Editor exports them. `Business name` and `Long headline N` come last:
/// Google documents them, but no real export has shown them yet.
pub fn editor_columns() -> Vec<String> {
    let mut c: Vec<String> = [
        "Campaign",
        "Campaign Type",
        "Networks",
        "Budget",
        "Budget type",
        "EU political ads",
        "Languages",
        "Bid Strategy Type",
        "App campaign store",
        "App campaign package name",
        "Campaign optimization",
        "Final URL suffix",
        "Ad Group",
        "Max CPC",
        "Max CPM",
        "Target CPV",
        "Target CPM",
        "Ad Group Type",
        "Location",
        "Keyword",
        "Criterion Type",
        "Final URL",
        "Ad type",
    ]
    .map(String::from)
    .into();
    c.extend((1..=5).map(|i| format!("Headline {i}")));
    c.extend((1..=5).map(|i| format!("Description {i}")));
    c.push("Asset Group".into());
    c.extend((6..=15).map(|i| format!("Headline {i}")));
    c.extend(["Path 1", "Path 2", "Campaign Status", "Ad Group Status"].map(String::from));
    c.extend(["Asset Group Status", "Status", "Business name"].map(String::from));
    c.extend((1..=5).map(|i| format!("Long headline {i}")));
    c
}

/// One row as column name to value. Missing columns are empty.
#[derive(Default)]
struct Row(BTreeMap<String, String>);

impl Row {
    fn set(mut self, col: &str, value: impl Into<String>) -> Self {
        self.0.insert(col.to_string(), value.into());
        self
    }

    fn numbered(mut self, name: &str, from: usize, values: &[String]) -> Self {
        for (i, v) in values.iter().enumerate() {
            self.0.insert(format!("{name} {}", from + i), v.clone());
        }
        self
    }
}

/// `250.00`: Editor writes money with a dot and 2 decimals whatever the account language.
fn money(c: Cents) -> String {
    format!("{}.{:02}", c.0 / 100, c.0 % 100)
}

fn criterion(m: MatchType, negative: bool) -> &'static str {
    match (m, negative) {
        (MatchType::Phrase, false) => "Phrase",
        (MatchType::Exact, false) => "Exact",
        (MatchType::Phrase, true) => "Negative Phrase",
        (MatchType::Exact, true) => "Negative Exact",
    }
}

struct Ctx<'a> {
    input: &'a Input,
    kit: Option<&'a BrandKit>,
    status: &'static str,
}

impl Ctx<'_> {
    fn under(&self, c: &Campaign) -> Row {
        Row::default()
            .set("Campaign", &c.name)
            .set("Campaign Status", self.status)
    }

    fn in_group(&self, c: &Campaign, group: &str) -> Row {
        self.under(c)
            .set("Ad Group", group)
            .set("Ad Group Status", ACTIVE)
    }

    fn campaign(&self, c: &Campaign) -> Row {
        let e = &self.input.export;
        let eu = if e.eu_political_ads {
            "Has EU political ads"
        } else {
            "Doesn't have EU political ads"
        };
        let row = self
            .under(c)
            .set("Campaign Type", c.kind.label())
            .set("Budget", money(c.daily_budget))
            .set("Budget type", "Daily")
            .set("EU political ads", eu)
            .set("Bid Strategy Type", c.bid_strategy.label());
        // Demand Gen targets language on its ad groups: Editor refuses it on the campaign.
        let row = if c.kind == CampaignKind::DemandGen {
            row
        } else {
            row.set("Languages", self.input.business.language_primary())
        };
        let suffix = e.url_suffix.replace("{mads_campaign}", &c.slug);
        match (c.kind, &self.input.app) {
            (CampaignKind::AppInstalls, Some(app)) => row
                .set("App campaign store", app.store_label())
                .set("App campaign package name", &app.id)
                .set("Campaign optimization", "Installs"),
            (CampaignKind::AppInstalls, None) => row,
            (CampaignKind::Search, _) => row
                .set("Networks", "Google search")
                .set("Final URL suffix", suffix),
            _ => row.set("Final URL suffix", suffix),
        }
    }

    /// An ad group row with the bids every ad group of a real export carries.
    fn group_row(&self, c: &Campaign, group: &str, max_cpc: String) -> Row {
        self.in_group(c, group)
            .set("Max CPC", max_cpc)
            .set("Max CPM", MIN_BID)
            .set("Target CPV", MIN_BID)
            .set("Target CPM", MIN_BID)
            .set("Ad Group Type", "Standard")
    }

    /// Location rows. Demand Gen targets location on its ad groups: Editor refuses it on the campaign.
    fn locations(&self, c: &Campaign) -> Vec<Row> {
        let Some(place) = self.input.business.locations.first() else {
            return Vec::new();
        };
        let row = |r: Row| r.set("Location", place).set("Status", ACTIVE);
        if c.kind == CampaignKind::DemandGen {
            c.asset_groups
                .iter()
                .map(|g| row(self.in_group(c, &g.name)))
                .collect()
        } else {
            vec![row(self.under(c))]
        }
    }

    fn keyword(&self, c: &Campaign, ag: &AdGroup, text: &str, kind: &str) -> Row {
        self.in_group(c, &ag.name)
            .set("Keyword", text)
            .set("Criterion Type", kind)
            .set("Status", ACTIVE)
    }

    fn search_group(&self, c: &Campaign, ag: &AdGroup, out: &mut Vec<Row>) {
        out.push(self.group_row(c, &ag.name, money(ag.default_cpc)));
        for k in &ag.keywords {
            out.push(self.keyword(c, ag, &k.text, criterion(k.match_type, false)));
        }
        for n in effective_negatives(c, ag) {
            out.push(self.keyword(c, ag, &n.text, criterion(n.match_type, true)));
        }
        let Some(kit) = self.kit else {
            return;
        };
        let ad = merge_rsa(&ag.rsa, kit);
        out.push(
            self.in_group(c, &ag.name)
                .set("Final URL", &ag.final_url)
                .set("Ad type", "Responsive search ad")
                .numbered("Headline", 1, &ad.headlines)
                .numbered("Description", 1, &ad.descriptions)
                .set("Path 1", ad.path1.unwrap_or_default())
                .set("Path 2", ad.path2.unwrap_or_default())
                .set("Status", ACTIVE),
        );
    }

    /// App and Demand Gen: an ad group with one ad holding the texts.
    fn group_with_ad(&self, c: &Campaign, g: &AssetGroup, ad_type: &str, out: &mut Vec<Row>) {
        let mut group = self.group_row(c, &g.name, MIN_BID.into());
        if c.kind == CampaignKind::DemandGen {
            group = group.set("Languages", self.input.business.language_primary());
        }
        out.push(group);
        let mut ad = self
            .in_group(c, &g.name)
            .set("Ad type", ad_type)
            .numbered("Headline", 1, &g.headlines)
            .numbered("Description", 1, &g.descriptions)
            .set("Status", ACTIVE);
        if c.kind == CampaignKind::DemandGen {
            ad = ad
                .set("Final URL", &g.final_url)
                .set("Business name", &g.business_name);
        }
        out.push(ad);
    }

    fn asset_group(&self, c: &Campaign, g: &AssetGroup) -> Row {
        self.under(c)
            .set("Asset Group", &g.name)
            .set("Asset Group Status", ACTIVE)
            .set("Final URL", &g.final_url)
            .set("Business name", &g.business_name)
            .numbered("Headline", 1, &g.headlines)
            .numbered("Long headline", 1, &g.long_headlines)
            .numbered("Description", 1, &g.descriptions)
    }

    fn rows_of(&self, c: &Campaign) -> Vec<Row> {
        let mut out = vec![self.campaign(c)];
        out.extend(self.locations(c));
        match c.kind {
            CampaignKind::Search => c
                .ad_groups
                .iter()
                .for_each(|ag| self.search_group(c, ag, &mut out)),
            CampaignKind::AppInstalls => c
                .asset_groups
                .iter()
                .for_each(|g| self.group_with_ad(c, g, "App ad for installs", &mut out)),
            CampaignKind::DemandGen => c
                .asset_groups
                .iter()
                .for_each(|g| self.group_with_ad(c, g, "Demand Gen image ad", &mut out)),
            CampaignKind::PerformanceMax => {
                out.extend(c.asset_groups.iter().map(|g| self.asset_group(c, g)));
            }
        }
        out
    }
}

/// Quotes a field the way Editor does: when it holds a comma, a quote, a tab or a line break.
fn field(v: &str) -> String {
    if v.contains([',', '"', '\t', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

/// UTF-16 LE with a byte order mark, as Editor writes its exports.
fn utf16le(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    out.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    out
}

/// Every campaign of the account in one Editor import file. None for an empty account.
pub fn export_editor(input: &Input, account: &Account) -> Option<CsvFile> {
    if account.campaigns.is_empty() {
        return None;
    }
    let ctx = Ctx {
        input,
        kit: account.brand_kit.as_ref(),
        status: input.export.status.as_str(),
    };
    let cols = editor_columns();
    let mut text = cols.join("\t");
    text.push('\n');
    for row in account.campaigns.iter().flat_map(|c| ctx.rows_of(c)) {
        let line: Vec<String> = cols
            .iter()
            .map(|col| field(row.0.get(col).map_or("", String::as_str)))
            .collect();
        text.push_str(&line.join("\t"));
        text.push('\n');
    }
    Some(CsvFile {
        name: EDITOR_FILE,
        bytes: utf16le(&text),
    })
}

/// Reads an Editor file back: header and rows as column to value. For tests and tools.
pub fn read_editor(bytes: &[u8]) -> Result<Vec<BTreeMap<String, String>>, String> {
    let body = bytes
        .strip_prefix(&[0xFF, 0xFE])
        .ok_or("no UTF-16 LE byte order mark")?;
    if body.len() % 2 != 0 {
        return Err("odd number of UTF-16 bytes".into());
    }
    let (pairs, _) = body.as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|b| u16::from_le_bytes(*b)).collect();
    let text = String::from_utf16(&units).map_err(|e| e.to_string())?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_reader(text.as_bytes());
    let header: Vec<String> = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(String::from)
        .collect();
    rdr.records()
        .map(|r| {
            let r = r.map_err(|e| e.to_string())?;
            Ok(header
                .iter()
                .zip(r.iter())
                .filter(|(_, v)| !v.is_empty())
                .map(|(h, v)| (h.clone(), v.to_string()))
                .collect())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::{BidStrategy, Intent};

    fn demand_gen() -> Campaign {
        Campaign {
            name: "Feed".into(),
            slug: "feed".into(),
            kind: CampaignKind::DemandGen,
            intent: Intent::Generic,
            daily_budget: Cents(5000),
            bid_strategy: BidStrategy::MaximizeClicks { max_cpc: None },
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![],
            asset_groups: vec![AssetGroup {
                name: "tintos".into(),
                final_url: "https://vinellu.com/app".into(),
                business_name: "Vinellu".into(),
                headlines: vec!["H".into()],
                long_headlines: vec![],
                descriptions: vec!["D".into()],
                search_themes: vec![],
                images: vec![],
            }],
            negatives: vec![],
            assets: None,
        }
    }

    #[test]
    fn demand_gen_targets_language_on_its_ad_groups_not_on_the_campaign() {
        // Editor: "campaign languages are not allowed when location and language targeting is at
        // the ad group level", which is how a Demand Gen campaign starts.
        let account = Account {
            brand_kit: None,
            campaigns: vec![demand_gen()],
        };
        let file = export_editor(&crate::testutil::input(), &account).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let campaign = rows
            .iter()
            .find(|r| r.contains_key("Campaign Type"))
            .unwrap();
        assert!(!campaign.contains_key("Languages"), "{campaign:?}");
        let group = rows
            .iter()
            .find(|r| r.contains_key("Ad Group Type"))
            .unwrap();
        assert_eq!(group.get("Languages").map(String::as_str), Some("pt"));
        assert_eq!(
            group.get("Max CPM").map(String::as_str),
            Some("0.01"),
            "no-bids warning"
        );
        let locations: Vec<_> = rows.iter().filter(|r| r.contains_key("Location")).collect();
        assert_eq!(locations.len(), 1);
        assert_eq!(
            locations[0].get("Ad Group").map(String::as_str),
            Some("tintos"),
            "Demand Gen targets location on the ad group too"
        );
    }
}
