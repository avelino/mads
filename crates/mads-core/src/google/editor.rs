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
    export::effective_negatives, merge_rsa, normalize,
};
use crate::{
    input::{App, AppGoal, Input},
    money::Cents,
    perf::{Live, LiveStatus},
};

/// Path of the file under the platform directory.
pub const EDITOR_FILE: &str = "editor/account.csv";

const ACTIVE: &str = "Enabled";
const PAUSED: &str = "Paused";
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

/// Columns an in-app action goal adds after the ones above. Only a file with such a campaign has
/// them, so the file of an installs account keeps its header. Google documents no column list for
/// this goal and no real export has shown them: the names are not verified against Editor.
const IN_APP_COLUMNS: [&str; 2] = ["Conversion action", "Target CPA"];

/// One row as column name to value. Missing columns are empty.
#[derive(Default)]
pub(crate) struct Row(BTreeMap<String, String>);

impl Row {
    pub(crate) fn set(mut self, col: &str, value: impl Into<String>) -> Self {
        self.0.insert(col.to_string(), value.into());
        self
    }

    pub(crate) fn numbered(mut self, name: &str, from: usize, values: &[String]) -> Self {
        for (i, v) in values.iter().enumerate() {
            self.0.insert(format!("{name} {}", from + i), v.clone());
        }
        self
    }
}

/// `250.00`: Editor writes money with a dot and 2 decimals whatever the account language.
pub(crate) fn money(c: Cents) -> String {
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
    live: Option<&'a Live>,
}

/// The reports show the campaign in the account and not removed. A campaign of the baseline that
/// was never imported, or that the advertiser removed, gets no row: Editor would create it.
pub(crate) fn is_live(live: &Live, name: &str) -> bool {
    live.performance
        .campaign(name)
        .is_some_and(|p| match p.live_status {
            Some(s) => s != LiveStatus::Removed,
            None => !p.ad_groups.is_empty() || p.metrics.impressions > 0 || p.metrics.clicks > 0,
        })
}

pub(crate) fn same_name(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

/// The campaign of the live account's baseline named like `name`.
pub(crate) fn baseline_campaign<'a>(live: &'a Live, name: &str) -> Option<&'a Campaign> {
    live.baseline
        .campaigns
        .iter()
        .find(|c| same_name(&c.name, name))
}

/// Something the live account serves that the new account dropped. Names come from the live
/// account. `campaign` and `group` are the kept ones of the new account.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Dropped<'a> {
    /// A whole campaign, any kind.
    Campaign { name: &'a str },
    /// An ad group of a kept campaign.
    AdGroup {
        campaign: &'a Campaign,
        name: &'a str,
    },
    /// A keyword of a kept ad group.
    Keyword {
        campaign: &'a Campaign,
        group: &'a AdGroup,
        text: &'a str,
        match_type: MatchType,
    },
    /// An asset group of a kept campaign.
    AssetGroup {
        campaign: &'a Campaign,
        name: &'a str,
    },
}

/// The single owner of "what did the new plan drop": every live campaign, ad group, keyword and
/// asset group with no match by name in `new`. A dropped group hides its keywords: pausing the
/// group is enough.
pub(crate) fn dropped_items<'a>(live: &'a Live, new: &'a Account) -> Vec<Dropped<'a>> {
    let mut out = Vec::new();
    for old in live
        .baseline
        .campaigns
        .iter()
        .filter(|c| is_live(live, &c.name))
    {
        match new.campaigns.iter().find(|c| same_name(&c.name, &old.name)) {
            None => out.push(Dropped::Campaign { name: &old.name }),
            Some(c) => {
                dropped_in(old, c, &mut out);
                out.extend(stray_in(live, old, c));
            }
        }
    }
    out
}

/// Ad groups the reports show enabled in a campaign that the run being optimized does not have:
/// an older import left them serving next to this run's groups, competing for the same searches.
pub(crate) fn stray_groups<'a>(live: &'a Live, old: &Campaign) -> Vec<&'a str> {
    let known = |name: &str| {
        let ads = old.ad_groups.iter().map(|g| &g.name);
        let assets = old.asset_groups.iter().map(|g| &g.name);
        ads.chain(assets).any(|n| same_name(n, name))
    };
    let groups = live.performance.campaign(&old.name).map(|p| &p.ad_groups);
    groups
        .into_iter()
        .flatten()
        .filter(|g| g.live_status == Some(LiveStatus::Enabled) && !known(&g.name))
        .map(|g| g.name.as_str())
        .collect()
}

/// The stray groups the new plan does not take back by name: paused like the ones it dropped.
pub(crate) fn stray_in<'a>(live: &'a Live, old: &Campaign, c: &'a Campaign) -> Vec<Dropped<'a>> {
    let kept = |name: &str| {
        let ads = c.ad_groups.iter().map(|g| &g.name);
        let assets = c.asset_groups.iter().map(|g| &g.name);
        ads.chain(assets).any(|n| same_name(n, name))
    };
    stray_groups(live, old)
        .into_iter()
        .filter(|name| !kept(name))
        .map(|name| match c.kind {
            CampaignKind::Search => Dropped::AdGroup { campaign: c, name },
            _ => Dropped::AssetGroup { campaign: c, name },
        })
        .collect()
}

fn dropped_in<'a>(old: &'a Campaign, c: &'a Campaign, out: &mut Vec<Dropped<'a>>) {
    for g in &old.ad_groups {
        let Some(kept) = c.ad_groups.iter().find(|n| same_name(&n.name, &g.name)) else {
            out.push(Dropped::AdGroup {
                campaign: c,
                name: &g.name,
            });
            continue;
        };
        let gone = g.keywords.iter().filter(|k| {
            !kept
                .keywords
                .iter()
                .any(|n| n.match_type == k.match_type && same_name(&n.text, &k.text))
        });
        out.extend(gone.map(|k| Dropped::Keyword {
            campaign: c,
            group: kept,
            text: &k.text,
            match_type: k.match_type,
        }));
    }
    for g in &old.asset_groups {
        if !c.asset_groups.iter().any(|n| same_name(&n.name, &g.name)) {
            out.push(Dropped::AssetGroup {
                campaign: c,
                name: &g.name,
            });
        }
    }
}

impl Ctx<'_> {
    /// A campaign the plan keeps is one worth the money, so it is exported enabled even when it
    /// is paused in the account: the plan leaves out what should stay off, and the import pauses
    /// that. A campaign the account does not have yet follows `export.status`.
    fn status_of(&self, c: &Campaign) -> &'static str {
        let live = self.live.and_then(|l| l.performance.campaign(&c.name));
        match live.and_then(|p| p.live_status) {
            Some(LiveStatus::Enabled | LiveStatus::Paused) => ACTIVE,
            _ => self.status,
        }
    }

    fn under(&self, c: &Campaign) -> Row {
        Row::default()
            .set("Campaign", &c.name)
            .set("Campaign Status", self.status_of(c))
    }

    /// Rows that pause what the live account has and the new account dropped. Editor never
    /// deletes on import: a missing row would leave the old entity serving.
    fn dropped(&self, new: &Account) -> Vec<Row> {
        let Some(live) = self.live else {
            return Vec::new();
        };
        dropped_items(live, new)
            .into_iter()
            .map(|d| match d {
                Dropped::Campaign { name } => Row::default()
                    .set("Campaign", name)
                    .set("Campaign Status", PAUSED),
                Dropped::AdGroup { campaign, name } => self
                    .under(campaign)
                    .set("Ad Group", name)
                    .set("Ad Group Status", PAUSED),
                Dropped::Keyword {
                    campaign,
                    group,
                    text,
                    match_type,
                } => self
                    .keyword(campaign, group, text, criterion(match_type, false))
                    .set("Status", PAUSED),
                Dropped::AssetGroup { campaign, name }
                    if campaign.kind == CampaignKind::PerformanceMax =>
                {
                    self.under(campaign)
                        .set("Asset Group", name)
                        .set("Asset Group Status", PAUSED)
                }
                Dropped::AssetGroup { campaign, name } => self
                    .under(campaign)
                    .set("Ad Group", name)
                    .set("Ad Group Status", PAUSED),
            })
            .collect()
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
            .set("EU political ads", eu);
        // A strategy the advertiser changed in Google Ads stays: an empty cell changes nothing.
        let by_hand = self
            .live
            .and_then(|l| l.performance.campaign(&c.name))
            .is_some_and(|p| p.bids_unlike(c.bid_strategy.label()));
        let row = if by_hand {
            row
        } else {
            row.set("Bid Strategy Type", c.bid_strategy.label())
        };
        // Demand Gen targets language on its ad groups: Editor refuses it on the campaign.
        let row = if c.kind == CampaignKind::DemandGen {
            row
        } else {
            row.set("Languages", self.input.business.language_primary())
        };
        let suffix = e.url_suffix.replace("{mads_campaign}", &c.slug);
        match (c.kind, self.input.app_for(c.app_store)) {
            (CampaignKind::AppInstalls, Some(app)) => self.app_campaign(row, app),
            (CampaignKind::AppInstalls, None) => row,
            (CampaignKind::Search, _) => row
                .set("Networks", "Google search")
                .set("Final URL suffix", suffix),
            _ => row.set("Final URL suffix", suffix),
        }
    }

    /// The App columns: the store, and what the campaign optimizes for.
    fn app_campaign(&self, row: Row, app: &App) -> Row {
        let row = row
            .set("App campaign store", app.store_label())
            .set("App campaign package name", &app.id);
        if app.optimize_for != AppGoal::InAppActions {
            return row.set("Campaign optimization", "Installs");
        }
        let row = row
            .set("Campaign optimization", "In-app actions")
            .set("Conversion action", &app.conversion_action);
        match self.input.budget.target_cpa_in_app {
            Some(target) => row.set("Target CPA", money(target)),
            None => row,
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

    /// One Location row per place. Demand Gen targets location on its ad groups: Editor refuses it
    /// on the campaign.
    fn locations(&self, c: &Campaign) -> Vec<Row> {
        let mut out = Vec::new();
        for place in &self.input.business.locations {
            let row = |r: Row| r.set("Location", place).set("Status", ACTIVE);
            if c.kind == CampaignKind::DemandGen {
                out.extend(
                    c.asset_groups
                        .iter()
                        .map(|g| row(self.in_group(c, &g.name))),
                );
            } else {
                out.push(row(self.under(c)));
            }
        }
        out
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

    /// Some App campaign of the account optimizes for an in-app action.
    fn has_in_app_goal(&self, account: &Account) -> bool {
        account.campaigns.iter().any(|c| {
            c.kind == CampaignKind::AppInstalls
                && self
                    .input
                    .app_for(c.app_store)
                    .is_some_and(|a| a.optimize_for == AppGoal::InAppActions)
        })
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
    export_editor_live(input, account, None)
}

/// The Editor file of an account that replaces a live one: campaigns already live keep their
/// status, and what the live account has that this one dropped is paused.
pub fn export_editor_live(
    input: &Input,
    account: &Account,
    live: Option<&Live>,
) -> Option<CsvFile> {
    if account.campaigns.is_empty() {
        return None;
    }
    let ctx = Ctx {
        input,
        kit: account.brand_kit.as_ref(),
        status: input.export.status.as_str(),
        live,
    };
    let mut cols = editor_columns();
    if ctx.has_in_app_goal(account) {
        cols.extend(IN_APP_COLUMNS.map(String::from));
    }
    let rows = account
        .campaigns
        .iter()
        .flat_map(|c| ctx.rows_of(c))
        .chain(ctx.dropped(account));
    Some(editor_sheet(EDITOR_FILE, &cols, rows))
}

/// UTF-16 LE with a BOM, tab separated, LF. The same writer `editor_columns` feeds.
pub(crate) fn editor_sheet<S: std::borrow::Borrow<str>>(
    name: &'static str,
    cols: &[S],
    rows: impl IntoIterator<Item = Row>,
) -> CsvFile {
    let mut text = cols.join("\t");
    text.push('\n');
    for row in rows {
        let line: Vec<String> = cols
            .iter()
            .map(|col| field(row.0.get(col.borrow()).map_or("", String::as_str)))
            .collect();
        text.push_str(&line.join("\t"));
        text.push('\n');
    }
    CsvFile {
        name: name.to_string(),
        bytes: utf16le(&text),
    }
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
    use crate::google::{AdGroup, BidStrategy, Intent};
    use crate::perf::{Live, LiveStatus};

    fn demand_gen() -> Campaign {
        Campaign {
            app_store: None,
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

    fn app_campaign() -> Campaign {
        let mut c = demand_gen();
        c.name = "App".into();
        c.slug = "app".into();
        c.kind = CampaignKind::AppInstalls;
        c.bid_strategy = BidStrategy::MaximizeConversions;
        c
    }

    fn app_input(goal: crate::input::AppGoal, action: &str) -> Input {
        let mut input = crate::testutil::input();
        let mut app =
            crate::input::App::installs(crate::input::AppStore::GooglePlay, "com.example.app");
        app.optimize_for = goal;
        app.conversion_action = action.into();
        input.apps = vec![app];
        input
    }

    fn app_rows(input: &Input) -> (String, Vec<BTreeMap<String, String>>) {
        let account = Account {
            brand_kit: None,
            campaigns: vec![app_campaign()],
        };
        let file = export_editor(input, &account).unwrap();
        let text = String::from_utf16(
            &file.bytes[2..]
                .chunks(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let header = text.lines().next().unwrap().to_string();
        (header, read_editor(&file.bytes).unwrap())
    }

    #[test]
    fn an_app_campaign_optimizes_for_installs_by_default() {
        let (header, rows) = app_rows(&app_input(crate::input::AppGoal::Installs, ""));
        let campaign = rows
            .iter()
            .find(|r| r.contains_key("Campaign Type"))
            .unwrap();
        assert_eq!(
            campaign.get("Campaign optimization").map(String::as_str),
            Some("Installs")
        );
        assert!(!campaign.contains_key("Conversion action"));
        assert!(!campaign.contains_key("Target CPA"));
        assert_eq!(
            header,
            editor_columns().join("\t"),
            "the installs file keeps its columns"
        );
    }

    #[test]
    fn an_app_campaign_can_optimize_for_an_in_app_action() {
        let mut input = app_input(crate::input::AppGoal::InAppActions, "Sign up");
        input.budget.target_cpa_in_app = Some(Cents(1250));
        let (header, rows) = app_rows(&input);
        let campaign = rows
            .iter()
            .find(|r| r.contains_key("Campaign Type"))
            .unwrap();
        let get = |k: &str| campaign.get(k).map(String::as_str);
        assert_eq!(get("Campaign optimization"), Some("In-app actions"));
        assert_eq!(get("Conversion action"), Some("Sign up"));
        assert_eq!(get("Target CPA"), Some("12.50"));
        assert_eq!(get("App campaign package name"), Some("com.example.app"));
        assert_eq!(get("Bid Strategy Type"), Some("Maximize conversions"));
        assert!(header.starts_with(&editor_columns().join("\t")));
        assert!(
            header.contains("Conversion action") && header.contains("Target CPA")
        );
        let others = rows.iter().filter(|r| !r.contains_key("Campaign Type"));
        assert!(others.clone().all(|r| !r.contains_key("Conversion action")));
        assert!(others.clone().all(|r| !r.contains_key("Target CPA")));
        assert!(others.count() > 0);
    }

    #[test]
    fn an_in_app_campaign_without_a_target_writes_no_target() {
        let (_, rows) = app_rows(&app_input(crate::input::AppGoal::InAppActions, "Sign up"));
        let campaign = rows
            .iter()
            .find(|r| r.contains_key("Campaign Type"))
            .unwrap();
        assert_eq!(
            campaign.get("Conversion action").map(String::as_str),
            Some("Sign up")
        );
        assert!(!campaign.contains_key("Target CPA"));
    }

    #[test]
    fn every_location_gets_its_own_row() {
        let mut input = crate::testutil::input();
        input.business.locations = vec![
            "Belo Horizonte,State of Minas Gerais,Brazil".into(),
            "Sao Paulo,State of Sao Paulo,Brazil".into(),
        ];
        let account = Account {
            brand_kit: None,
            campaigns: vec![
                demand_gen(),
                search("Rota", &[("bh-sp", &["onibus bh sp"])]),
            ],
        };
        let file = export_editor(&input, &account).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let places = |campaign: &str| -> Vec<String> {
            rows.iter()
                .filter(|r| r.get("Campaign").map(String::as_str) == Some(campaign))
                .filter_map(|r| r.get("Location").cloned())
                .collect()
        };
        assert_eq!(places("Feed"), input.business.locations);
        assert_eq!(places("Rota"), input.business.locations);
    }

    fn search(name: &str, groups: &[(&str, &[&str])]) -> Campaign {
        Campaign {
            app_store: None,
            name: name.into(),
            slug: crate::input::slugify(name),
            kind: CampaignKind::Search,
            intent: Intent::Generic,
            daily_budget: Cents(5000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: groups
                .iter()
                .map(|(g, kws)| AdGroup {
                    name: (*g).into(),
                    default_cpc: Cents(120),
                    cpc_rationale: String::new(),
                    final_url: "https://vinellu.com/app".into(),
                    keywords: kws
                        .iter()
                        .map(|k| super::super::Keyword {
                            text: (*k).into(),
                            match_type: MatchType::Phrase,
                        })
                        .collect(),
                    negatives: vec![],
                    rsa: Default::default(),
                })
                .collect(),
            asset_groups: vec![],
            negatives: vec![],
            assets: None,
        }
    }

    fn live(baseline: Vec<Campaign>, status: &[(&str, LiveStatus)]) -> Live {
        let mut performance = crate::perf::Performance::default();
        for (name, s) in status {
            performance.campaigns.push(crate::perf::CampaignPerf {
                name: (*name).into(),
                live_status: Some(*s),
                ..Default::default()
            });
        }
        Live {
            baseline: Account {
                brand_kit: None,
                campaigns: baseline,
            },
            performance,
        }
    }

    fn row(
        rows: &[BTreeMap<String, String>],
        pick: impl Fn(&BTreeMap<String, String>) -> bool,
    ) -> Option<&BTreeMap<String, String>> {
        rows.iter().find(|r| pick(r))
    }

    #[test]
    fn a_bid_strategy_changed_by_hand_is_left_as_it_is() {
        let old = vec![
            search("Catalogo", &[("tintos", &["malbec"])]),
            search("Marca", &[("marca", &["vinellu"])]),
        ];
        let new = Account {
            brand_kit: None,
            campaigns: old.clone(),
        };
        let mut base = live(
            old,
            &[
                ("Catalogo", LiveStatus::Enabled),
                ("Marca", LiveStatus::Enabled),
            ],
        );
        base.performance.campaigns[0].bid_strategy = "Maximizar as conversões".into();
        base.performance.campaigns[1].bid_strategy = "CPC manual".into();
        let file = export_editor_live(&crate::testutil::input(), &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let get = |r: &BTreeMap<String, String>, k: &str| r.get(k).cloned().unwrap_or_default();
        let strategy = |name: &str| {
            let r = row(&rows, |r| {
                get(r, "Campaign") == name && !get(r, "Budget").is_empty()
            })
            .expect("campaign row");
            get(r, "Bid Strategy Type")
        };
        assert_eq!(
            strategy("Catalogo"),
            "",
            "Editor keeps what the account has"
        );
        assert_eq!(strategy("Marca"), "Manual CPC");
    }

    #[test]
    fn a_group_of_an_older_import_still_serving_is_paused() {
        let old = vec![search("Catalogo", &[("tintos", &["malbec"])])];
        let new = Account {
            brand_kit: None,
            campaigns: vec![search(
                "Catalogo",
                &[("tintos", &["malbec"]), ("volta", &["merlot"])],
            )],
        };
        let mut base = live(old, &[("Catalogo", LiveStatus::Enabled)]);
        let group = |name: &str, s: LiveStatus| crate::perf::AdGroupPerf {
            name: name.into(),
            live_status: Some(s),
            ..Default::default()
        };
        base.performance.campaigns[0].ad_groups = vec![
            group("tintos", LiveStatus::Enabled),
            group("Antigo", LiveStatus::Enabled),
            group("Volta", LiveStatus::Enabled),
            group("parado", LiveStatus::Paused),
            group("morto", LiveStatus::Removed),
        ];
        let file = export_editor_live(&crate::testutil::input(), &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let get = |r: &BTreeMap<String, String>, k: &str| r.get(k).cloned().unwrap_or_default();
        let status = |name: &str| -> Vec<String> {
            rows.iter()
                .filter(|r| get(r, "Ad Group") == name && get(r, "Keyword").is_empty())
                .map(|r| get(r, "Ad Group Status"))
                .collect()
        };
        assert_eq!(status("Antigo"), ["Paused"]);
        assert!(status("parado").is_empty() && status("morto").is_empty());
        assert!(
            !status("Volta").contains(&"Paused".to_string()) && status("Volta").is_empty(),
            "the new plan took the name back: only its own rows, as `volta`"
        );
    }

    #[test]
    fn what_the_new_account_drops_is_paused() {
        let old = vec![
            search(
                "Catalogo",
                &[
                    ("tintos", &["malbec", "merlot"]),
                    ("brancos", &["chardonnay"]),
                ],
            ),
            search("Antiga", &[("g", &["x"])]),
            demand_gen(),
        ];
        let mut feed = demand_gen();
        feed.asset_groups.clear();
        let new = Account {
            brand_kit: None,
            campaigns: vec![search("Catalogo", &[("tintos", &["malbec"])]), feed],
        };
        let base = live(
            old,
            &[
                ("Catalogo", LiveStatus::Enabled),
                ("Antiga", LiveStatus::Enabled),
                ("Feed", LiveStatus::Paused),
            ],
        );
        let file = export_editor_live(&crate::testutil::input(), &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let get = |r: &BTreeMap<String, String>, k: &str| r.get(k).cloned().unwrap_or_default();
        let paused_kw = row(&rows, |r| get(r, "Keyword") == "merlot").expect("merlot row");
        assert_eq!(get(paused_kw, "Status"), "Paused");
        assert_eq!(get(paused_kw, "Criterion Type"), "Phrase");
        let kept_kw = row(&rows, |r| get(r, "Keyword") == "malbec").unwrap();
        assert_eq!(get(kept_kw, "Status"), "Enabled");
        let group = row(&rows, |r| get(r, "Ad Group") == "brancos").expect("brancos row");
        assert_eq!(get(group, "Ad Group Status"), "Paused");
        assert!(
            row(&rows, |r| get(r, "Keyword") == "chardonnay").is_none(),
            "pausing the group is enough"
        );
        let campaign = row(&rows, |r| get(r, "Campaign") == "Antiga").expect("Antiga row");
        assert_eq!(get(campaign, "Campaign Status"), "Paused");
        assert_eq!(
            rows.iter()
                .filter(|r| get(r, "Campaign") == "Antiga")
                .count(),
            1
        );
        let tintos = row(&rows, |r| {
            get(r, "Ad Group") == "tintos" && get(r, "Campaign") == "Feed"
        })
        .expect("asset group row");
        assert_eq!(get(tintos, "Ad Group Status"), "Paused");
    }

    #[test]
    fn only_campaigns_the_reports_show_live_get_paused_rows() {
        let old = vec![
            search("Nunca importada", &[("g", &["x"])]),
            search("Removida", &[("g", &["x"])]),
        ];
        let new = Account {
            brand_kit: None,
            campaigns: vec![search("Catalogo", &[("tintos", &["malbec"])])],
        };
        let base = live(old, &[("Removida", LiveStatus::Removed)]);
        let file = export_editor_live(&crate::testutil::input(), &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        for name in ["Nunca importada", "Removida"] {
            assert!(
                !rows
                    .iter()
                    .any(|r| r.get("Campaign").map(String::as_str) == Some(name)),
                "{name} must not be written: Editor would create it"
            );
        }
    }

    #[test]
    fn a_paused_live_campaign_the_plan_keeps_is_enabled() {
        let new = Account {
            brand_kit: None,
            campaigns: vec![search("Catalogo", &[("tintos", &["malbec"])])],
        };
        let base = live(
            vec![search("Catalogo", &[("tintos", &["malbec"])])],
            &[("catalogo", LiveStatus::Paused)],
        );
        let file = export_editor_live(&crate::testutil::input(), &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let statuses: Vec<_> = rows
            .iter()
            .filter(|r| r.get("Campaign").map(String::as_str) == Some("Catalogo"))
            .filter_map(|r| r.get("Campaign Status"))
            .collect();
        assert!(!statuses.is_empty());
        assert!(statuses.iter().all(|s| *s == "Enabled"), "{statuses:?}");
    }

    #[test]
    fn campaigns_already_live_keep_their_status() {
        let input = crate::testutil::input();
        assert_eq!(input.export.status.as_str(), "Paused");
        let new = Account {
            brand_kit: None,
            campaigns: vec![
                search("Catalogo", &[("tintos", &["malbec"])]),
                search("Nova", &[("g", &["x"])]),
            ],
        };
        let base = live(
            vec![search("Catalogo", &[("tintos", &["malbec"])])],
            &[("catalogo", LiveStatus::Enabled)],
        );
        let file = export_editor_live(&input, &new, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let status = |c: &str| {
            rows.iter()
                .filter(|r| r.get("Campaign").map(String::as_str) == Some(c))
                .map(|r| r.get("Campaign Status").cloned().unwrap_or_default())
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(
            status("Catalogo").into_iter().collect::<Vec<_>>(),
            ["Enabled"]
        );
        assert_eq!(
            status("Nova").into_iter().collect::<Vec<_>>(),
            ["Paused"],
            "new campaigns follow export.status"
        );
    }
}
