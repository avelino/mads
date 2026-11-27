//! The whole account as one Google Ads Editor import file.
//!
//! The format is the one Editor exports (checked against an export of version 2.13.3): UTF-16 LE
//! with a BOM, tab separated, LF line endings, no row type column (the filled columns say what a
//! row is), money with a dot and 2 decimals. Search and App rows match that export for campaigns,
//! ad groups, ads and positive keywords. Campaign negatives are written once per Search campaign
//! as `Campaign Negative Phrase` or `Campaign Negative Exact` with an empty Ad Group, so Editor
//! imports them on the Negatives tab rather than as positive Phrase keywords. Ad-group negatives
//! use `Negative Phrase` or `Negative Exact` with the Ad Group filled. Performance Max rows follow
//! Google's documented headers, not imported yet. Pictures are not in the file: Editor neither
//! exports nor imports the link between an ad and its images.

use std::collections::BTreeMap;

use super::{
    Account, AdGroup, AssetGroup, BrandKit, Campaign, CampaignKind, CsvFile, MatchType, merge_rsa,
    normalize,
};
use crate::{
    input::{App, AppGoal, Input},
    money::Cents,
    perf::{Live, LiveStatus, ReportKind, ReportedMatch},
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

/// Editor label for a campaign-level negative. An empty Ad Group plus this value is what
/// Editor files as a campaign negative. `Negative Phrase` on an ad group row has been
/// imported as a positive Phrase keyword, so campaign negatives must not use that form.
fn campaign_negative_criterion(m: MatchType) -> &'static str {
    match m {
        MatchType::Phrase => "Campaign Negative Phrase",
        MatchType::Exact => "Campaign Negative Exact",
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
    /// A keyword of a kept ad group, with the match type the account has it in: a plan holds only
    /// Phrase and Exact, the account may also hold Broad.
    Keyword {
        campaign: &'a Campaign,
        group: &'a AdGroup,
        text: &'a str,
        match_type: ReportedMatch,
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
                dropped_in(live, old, c, &mut out);
                out.extend(stray_in(live, old, c));
            }
        }
    }
    out
}

/// Campaigns outside the run that `--pause-outside-run` pauses. The run named them, and they are
/// still enabled in the reports and not in the new account: nothing else gets a row, and a
/// removed campaign never does.
pub(crate) fn outside_to_pause<'a>(live: &'a Live, new: &Account) -> Vec<&'a str> {
    let p = &live.performance;
    let enabled_outside = |name: &str| {
        p.outside
            .iter()
            .any(|o| same_name(&o.name, name) && o.live_status == Some(LiveStatus::Enabled))
    };
    p.pause_outside_run
        .iter()
        .filter(|n| {
            let in_run = new.campaigns.iter().any(|c| same_name(&c.name, n.as_str()));
            !in_run && enabled_outside(n.as_str())
        })
        .map(String::as_str)
        .collect()
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

/// The reports hold a keyword report: what it lists is the account's keywords, not the baseline.
fn has_keyword_report(live: &Live) -> bool {
    live.performance
        .reports
        .iter()
        .any(|r| r.kind == ReportKind::Keywords)
}

/// The keywords the live account serves in the ad group `g` of the baseline. With a keyword report
/// they are the enabled ones it lists, of every match type, Broad included: the baseline lacks
/// what auto-applied recommendations added, and lists what an import already paused or removed.
/// Without one, the baseline's keywords stand in. The baseline's spelling wins when both have it.
fn live_keywords<'a>(
    live: &'a Live,
    old: &'a Campaign,
    g: &'a AdGroup,
) -> Vec<(&'a str, ReportedMatch)> {
    if !has_keyword_report(live) {
        return g
            .keywords
            .iter()
            .map(|k| (k.text.as_str(), k.match_type.into()))
            .collect();
    }
    let reported = live
        .performance
        .campaign(&old.name)
        .and_then(|p| p.ad_groups.iter().find(|a| same_name(&a.name, &g.name)));
    let spelled = |text: &'a str, m: ReportedMatch| {
        g.keywords
            .iter()
            .find(|b| ReportedMatch::from(b.match_type) == m && same_name(&b.text, text))
            .map_or(text, |b| b.text.as_str())
    };
    reported
        .into_iter()
        .flat_map(|a| &a.keywords)
        .filter(|k| matches!(k.live_status, None | Some(LiveStatus::Enabled)))
        .filter_map(|k| Some((spelled(&k.text, k.match_type?), k.match_type?)))
        .collect()
}

/// Live keywords of a kept ad group that the new plan does not carry with the same text and match
/// type, once each.
pub(crate) fn gone_keywords<'a>(
    live: &'a Live,
    old: &'a Campaign,
    g: &'a AdGroup,
    kept: &AdGroup,
) -> Vec<(&'a str, ReportedMatch)> {
    let planned = |text: &str, m: ReportedMatch| {
        kept.keywords
            .iter()
            .any(|n| ReportedMatch::from(n.match_type) == m && same_name(&n.text, text))
    };
    let mut seen = std::collections::BTreeSet::new();
    live_keywords(live, old, g)
        .into_iter()
        .filter(|&(text, m)| !planned(text, m))
        .filter(|&(text, m)| seen.insert((normalize(text), m.label())))
        .collect()
}

fn dropped_in<'a>(live: &'a Live, old: &'a Campaign, c: &'a Campaign, out: &mut Vec<Dropped<'a>>) {
    for g in &old.ad_groups {
        let Some(kept) = c.ad_groups.iter().find(|n| same_name(&n.name, &g.name)) else {
            out.push(Dropped::AdGroup {
                campaign: c,
                name: &g.name,
            });
            continue;
        };
        out.extend(
            gone_keywords(live, old, g, kept)
                .into_iter()
                .map(|(text, match_type)| Dropped::Keyword {
                    campaign: c,
                    group: kept,
                    text,
                    match_type,
                }),
        );
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
        let mut rows: Vec<Row> = dropped_items(live, new)
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
                    .keyword(campaign, group, text, match_type.label())
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
            .collect();
        rows.extend(outside_to_pause(live, new).into_iter().map(|name| {
            Row::default()
                .set("Campaign", name)
                .set("Campaign Status", PAUSED)
        }));
        rows
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

    /// A campaign negative: Keyword and Criterion Type, Ad Group left empty.
    fn campaign_negative(&self, c: &Campaign, text: &str, kind: &str) -> Row {
        self.under(c)
            .set("Keyword", text)
            .set("Criterion Type", kind)
            .set("Status", ACTIVE)
    }

    fn search_group(&self, c: &Campaign, ag: &AdGroup, out: &mut Vec<Row>) {
        out.push(self.group_row(c, &ag.name, money(ag.default_cpc)));
        for k in &ag.keywords {
            out.push(self.keyword(c, ag, &k.text, criterion(k.match_type, false)));
        }
        for n in &ag.negatives {
            let already_at_campaign = c
                .negatives
                .iter()
                .any(|cn| cn.match_type == n.match_type && same_name(&cn.text, &n.text));
            if already_at_campaign {
                continue;
            }
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
        if c.kind == CampaignKind::Search {
            for n in &c.negatives {
                out.push(self.campaign_negative(
                    c,
                    &n.text,
                    campaign_negative_criterion(n.match_type),
                ));
            }
        }
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
    use crate::perf::{AdGroupPerf, KeywordPerf, Live, LiveStatus, ReportFile};

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
        assert!(header.contains("Conversion action") && header.contains("Target CPA"));
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

    #[test]
    fn campaigns_outside_the_run_are_paused_only_when_the_flag_named_them() {
        let catalogo = || search("Catalogo", &[("tintos", &["malbec"])]);
        let new = Account {
            brand_kit: None,
            campaigns: vec![catalogo()],
        };
        let mut base = live(vec![catalogo()], &[("catalogo", LiveStatus::Enabled)]);
        let outside = |name: &str, status| crate::perf::OutsideCampaign {
            name: name.into(),
            live_status: Some(status),
            ..Default::default()
        };
        base.performance.outside = vec![
            outside("Antiga", LiveStatus::Enabled),
            outside("Removida", LiveStatus::Removed),
            outside("Pausada", LiveStatus::Paused),
            outside("Outra", LiveStatus::Enabled),
        ];
        let others = |base: &Live| -> Vec<(String, String)> {
            let file = export_editor_live(&crate::testutil::input(), &new, Some(base)).unwrap();
            read_editor(&file.bytes)
                .unwrap()
                .into_iter()
                .filter(|r| r.get("Campaign").map(String::as_str) != Some("Catalogo"))
                .map(|r| {
                    let get = |k: &str| r.get(k).cloned().unwrap_or_default();
                    (get("Campaign"), get("Campaign Status"))
                })
                .collect()
        };
        assert!(others(&base).is_empty(), "no flag, no row");
        // The run never names a removed or paused campaign, and one of the run is not outside it.
        base.performance.pause_outside_run = vec![
            "Antiga".into(),
            "Removida".into(),
            "Pausada".into(),
            "Catalogo".into(),
        ];
        assert_eq!(
            others(&base),
            [("Antiga".to_string(), "Paused".to_string())]
        );
    }

    fn kperf(text: &str, m: ReportedMatch, status: Option<LiveStatus>) -> KeywordPerf {
        KeywordPerf {
            text: text.into(),
            match_type: Some(m),
            live_status: status,
            status_reasons: String::new(),
            signals: vec![],
            max_cpc: None,
            quality_score: None,
            first_page_bid: None,
            top_of_page_bid: None,
            metrics: Default::default(),
        }
    }

    /// The reports of `base` hold a keyword report listing `keywords` in the first campaign.
    fn with_keyword_report(base: &mut Live, group: &str, keywords: Vec<KeywordPerf>) {
        base.performance.reports.push(ReportFile {
            file: "keywords.csv".into(),
            kind: ReportKind::Keywords,
            rows: keywords.len(),
            window: None,
        });
        base.performance.campaigns[0].ad_groups = vec![AdGroupPerf {
            name: group.into(),
            keywords,
            ..Default::default()
        }];
    }

    fn keyword(text: &str, match_type: MatchType) -> super::super::Keyword {
        super::super::Keyword {
            text: text.into(),
            match_type,
        }
    }

    /// `(keyword, criterion type)` of every keyword row the import pauses, sorted.
    fn paused_keywords(new: &Account, base: &Live) -> Vec<(String, String)> {
        let file = export_editor_live(&crate::testutil::input(), new, Some(base)).unwrap();
        let get = |r: &BTreeMap<String, String>, k: &str| r.get(k).cloned().unwrap_or_default();
        let mut out: Vec<(String, String)> = read_editor(&file.bytes)
            .unwrap()
            .iter()
            .filter(|r| !get(r, "Keyword").is_empty() && get(r, "Status") == "Paused")
            .map(|r| (get(r, "Keyword"), get(r, "Criterion Type")))
            .collect();
        out.sort();
        out
    }

    /// `Catalogo` / `tintos` of the baseline holds `malbec` and `merlot` as Phrase. The new plan
    /// keeps `malbec` as Phrase and Exact and drops `merlot`.
    fn kept_group_setup() -> (Account, Live) {
        let mut old = search("Catalogo", &[("tintos", &["malbec", "merlot"])]);
        old.ad_groups[0]
            .keywords
            .push(keyword("malbec", MatchType::Exact));
        let mut new = search("Catalogo", &[("tintos", &["malbec"])]);
        new.ad_groups[0]
            .keywords
            .push(keyword("malbec", MatchType::Exact));
        let base = live(vec![old], &[("Catalogo", LiveStatus::Enabled)]);
        let new = Account {
            brand_kit: None,
            campaigns: vec![new],
        };
        (new, base)
    }

    #[test]
    fn without_a_keyword_report_the_baseline_decides_and_broad_is_not_seen() {
        let (new, mut base) = kept_group_setup();
        base.performance.campaigns[0].ad_groups = vec![AdGroupPerf {
            name: "tintos".into(),
            keywords: vec![kperf(
                "malbec",
                ReportedMatch::Broad,
                Some(LiveStatus::Enabled),
            )],
            ..Default::default()
        }];
        assert_eq!(
            paused_keywords(&new, &base),
            [("merlot".to_string(), "Phrase".to_string())]
        );
    }

    #[test]
    fn a_broad_keyword_the_plan_cannot_keep_is_paused_and_planned_ones_are_not() {
        let (new, mut base) = kept_group_setup();
        with_keyword_report(
            &mut base,
            "tintos",
            vec![
                kperf("malbec", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("malbec", ReportedMatch::Exact, Some(LiveStatus::Enabled)),
                kperf("malbec", ReportedMatch::Broad, Some(LiveStatus::Enabled)),
                kperf("syrah tinto", ReportedMatch::Broad, None),
            ],
        );
        assert_eq!(
            paused_keywords(&new, &base),
            [
                ("malbec".to_string(), "Broad".to_string()),
                ("syrah tinto".to_string(), "Broad".to_string()),
            ],
            "the planned Phrase and Exact of malbec stay enabled"
        );
    }

    #[test]
    fn a_keyword_absent_from_the_keyword_report_is_not_paused() {
        let (new, mut base) = kept_group_setup();
        with_keyword_report(
            &mut base,
            "tintos",
            vec![
                kperf("malbec", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("malbec", ReportedMatch::Exact, Some(LiveStatus::Enabled)),
            ],
        );
        assert!(
            paused_keywords(&new, &base).is_empty(),
            "merlot is gone live already: a Paused row would make Editor report a post error"
        );
    }

    #[test]
    fn a_dropped_keyword_still_enabled_in_the_report_is_paused_once() {
        let (new, mut base) = kept_group_setup();
        with_keyword_report(
            &mut base,
            "tintos",
            vec![
                kperf("Merlot", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("merlot", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("merlot", ReportedMatch::Exact, Some(LiveStatus::Paused)),
                kperf("tannat", ReportedMatch::Exact, Some(LiveStatus::Removed)),
            ],
        );
        assert_eq!(
            paused_keywords(&new, &base),
            [("merlot".to_string(), "Phrase".to_string())],
            "the baseline spelling is used, a paused or removed keyword gets no row"
        );
    }

    #[test]
    fn dropped_items_names_broad_as_the_match_type_to_pause() {
        let (new, mut base) = kept_group_setup();
        with_keyword_report(
            &mut base,
            "tintos",
            vec![kperf(
                "rioja",
                ReportedMatch::Broad,
                Some(LiveStatus::Enabled),
            )],
        );
        let found: Vec<(&str, ReportedMatch)> = dropped_items(&base, &new)
            .into_iter()
            .filter_map(|d| match d {
                Dropped::Keyword {
                    text, match_type, ..
                } => Some((text, match_type)),
                _ => None,
            })
            .collect();
        assert_eq!(found, [("rioja", ReportedMatch::Broad)]);
    }

    fn keyword_rows(account: &Account) -> Vec<BTreeMap<String, String>> {
        let file = export_editor(&crate::testutil::input(), account).unwrap();
        read_editor(&file.bytes)
            .unwrap()
            .into_iter()
            .filter(|r| r.get("Keyword").is_some_and(|k| !k.is_empty()))
            .collect()
    }

    fn cell(r: &BTreeMap<String, String>, k: &str) -> String {
        r.get(k).cloned().unwrap_or_default()
    }

    #[test]
    fn campaign_negatives_are_written_once_with_empty_ad_group() {
        let mut c = search(
            "Catalogo",
            &[("tintos", &["malbec"]), ("brancos", &["riesling"])],
        );
        c.negatives = vec![
            keyword("free", MatchType::Phrase),
            keyword("jobs", MatchType::Exact),
        ];
        c.ad_groups[0].negatives = vec![keyword("recipe", MatchType::Phrase)];
        let account = Account {
            brand_kit: None,
            campaigns: vec![c],
        };
        let rows = keyword_rows(&account);
        let kinds = |text: &str| {
            rows.iter()
                .filter(|r| cell(r, "Keyword") == text)
                .map(|r| {
                    (
                        cell(r, "Ad Group"),
                        cell(r, "Criterion Type"),
                        cell(r, "Status"),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds("free"),
            [(
                "".into(),
                "Campaign Negative Phrase".into(),
                "Enabled".into()
            )],
            "a campaign phrase negative is written once, Ad Group empty"
        );
        assert_eq!(
            kinds("jobs"),
            [(
                "".into(),
                "Campaign Negative Exact".into(),
                "Enabled".into()
            )],
        );
        assert_eq!(
            kinds("recipe"),
            [("tintos".into(), "Negative Phrase".into(), "Enabled".into())],
            "an ad-group negative stays on that group"
        );
        assert!(
            kinds("malbec").iter().all(|(_, kind, _)| kind == "Phrase"),
            "positives stay Phrase, never a negative label"
        );
        assert!(
            rows.iter().all(|r| {
                let kind = cell(r, "Criterion Type");
                !(cell(r, "Keyword") == "free" && kind == "Negative Phrase")
                    && !(cell(r, "Keyword") == "free" && kind == "Phrase")
                    && !(cell(r, "Keyword") == "jobs" && kind == "Negative Exact")
                    && !(cell(r, "Keyword") == "jobs" && kind == "Exact")
            }),
            "campaign negatives must not be cloned onto ad groups as Negative Phrase or as positives"
        );
    }

    #[test]
    fn a_duplicate_ad_group_negative_is_not_written_when_it_is_already_a_campaign_negative() {
        let mut c = search("Catalogo", &[("tintos", &["malbec"])]);
        c.negatives = vec![keyword("free", MatchType::Phrase)];
        c.ad_groups[0].negatives = vec![keyword("free", MatchType::Phrase)];
        let account = Account {
            brand_kit: None,
            campaigns: vec![c],
        };
        let rows = keyword_rows(&account);
        let free: Vec<_> = rows
            .iter()
            .filter(|r| cell(r, "Keyword") == "free")
            .map(|r| (cell(r, "Ad Group"), cell(r, "Criterion Type")))
            .collect();
        assert_eq!(free, [("".into(), "Campaign Negative Phrase".into())]);
    }

    #[test]
    fn an_enabled_positive_that_is_only_a_negative_in_the_plan_is_paused() {
        let mut new = search("Catalogo", &[("tintos", &["malbec"])]);
        new.negatives = vec![keyword("buy", MatchType::Phrase)];
        let account = Account {
            brand_kit: None,
            campaigns: vec![new],
        };
        let mut base = live(
            vec![search("Catalogo", &[("tintos", &["malbec"])])],
            &[("Catalogo", LiveStatus::Enabled)],
        );
        with_keyword_report(
            &mut base,
            "tintos",
            vec![
                kperf("malbec", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("buy", ReportedMatch::Phrase, Some(LiveStatus::Enabled)),
                kperf("buy", ReportedMatch::Exact, Some(LiveStatus::Enabled)),
                kperf("jobs", ReportedMatch::Broad, Some(LiveStatus::Enabled)),
            ],
        );
        let paused = paused_keywords(&account, &base);
        assert_eq!(
            paused,
            [
                ("buy".to_string(), "Exact".to_string()),
                ("buy".to_string(), "Phrase".to_string()),
                ("jobs".to_string(), "Broad".to_string()),
            ],
            "a leftover Enabled positive is paused even when the plan holds the same text as a negative"
        );
        let file = export_editor_live(&crate::testutil::input(), &account, Some(&base)).unwrap();
        let rows = read_editor(&file.bytes).unwrap();
        let buy_neg: Vec<_> = rows
            .iter()
            .filter(|r| {
                r.get("Keyword").map(String::as_str) == Some("buy")
                    && r.get("Criterion Type").map(String::as_str)
                        == Some("Campaign Negative Phrase")
            })
            .map(|r| {
                (
                    r.get("Ad Group").cloned().unwrap_or_default(),
                    r.get("Status").cloned().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            buy_neg,
            [("".into(), "Enabled".into())],
            "the campaign negative stays enabled next to the paused leftover positives"
        );
    }
}
