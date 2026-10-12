//! The package as plain data. Every workbook and the script render from a `Package`.

mod cells;
mod images;
mod live;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

pub(crate) use cells::{customer_cell, match_label, store_of, tracking_template};
pub use images::image_campaign_gaps;

use super::folder_name;
use crate::google::editor::{money, same_name};
use crate::google::export::effective_negatives;
use crate::google::{
    Account, AdGroup, BrandKit, Campaign, CampaignKind, ExportError, Intent, Keyword, MergedRsa,
    char_len, merge_rsa,
};
use crate::input::{CatalogItem, Input};
use crate::money::Cents;
use crate::perf::Live;

/// Smallest automated block, so the app can add items without a new export.
const MIN_CAPACITY: usize = 150;
/// Rows of the automated block the feed and the script handle.
const MAX_FEED_ROWS: usize = 1000;
/// Rows of the `Negatives` tab.
const MAX_NEGATIVES: usize = 2000;
const HEADLINES: usize = 15;
const DESCRIPTIONS: usize = 4;
/// Longest headline Google accepts.
const HEADLINE_LEN: usize = 30;

/// What the structural track does with a campaign, ad group or keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructAction {
    Add,
    Edit,
    Pause,
}

impl StructAction {
    pub fn label(self) -> &'static str {
        match self {
            StructAction::Add => "Add",
            StructAction::Edit => "Edit",
            StructAction::Pause => "Pause",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Package {
    pub folder: String,
    pub business: String,
    pub settings: Settings,
    pub campaigns: Vec<CampaignRow>,
    pub catalog: Vec<CatalogRow>,
    pub fixed: Vec<FixedRow>,
    pub templates: Vec<TemplateRow>,
    pub negatives: Vec<NegativeRow>,
    /// Rows of the automated block in `Feed` and of `Catalog`.
    pub capacity: usize,
    pub automated_campaigns: Vec<String>,
    pub has_app: bool,
    pub live: bool,
    /// Performance Max, Demand Gen and App campaigns the script creates.
    pub image_campaigns: Vec<ImageCampaignRow>,
    pub image_assets: Vec<ImageAssetRow>,
    /// Pictures to copy next to the workbooks.
    pub pictures: Vec<PictureCopy>,
}

/// One row of `Image Campaigns`. Columns: `columns::image_campaigns`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageCampaignRow {
    pub key: String,
    pub name: String,
    /// `Performance Max`, `Demand Gen` or `App`.
    pub campaign_type: String,
    pub budget: String,
    /// `manual_cpc`, `maximize_clicks` or `maximize_conversions`.
    pub bid_strategy: String,
    pub max_cpc: String,
    pub language: String,
    pub location: String,
    pub final_url: String,
    pub business_name: String,
    pub app_id: String,
    pub app_store: String,
    pub action: StructAction,
}

/// One row of `Image Assets`. `field` is a Google Ads asset field type such as `HEADLINE` or
/// `SQUARE_MARKETING_IMAGE`. Text fields fill `text`, picture fields fill `image_file`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageAssetRow {
    pub campaign_key: String,
    pub group: String,
    pub field: String,
    pub text: String,
    /// Relative to the package's `Images/` folder: `<campaign_key>/<file>` or `logo.png`.
    pub image_file: String,
}

/// A picture the export copies into the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PictureCopy {
    /// Relative to `google-ads/editor/`, as `ImageBrief.file` stores it.
    pub from: String,
    /// Relative to the package's `Images/` folder.
    pub to: String,
}

impl Package {
    /// Data rows of the `Feed` tab: the automated block, then one row per fixed row.
    pub fn feed_rows(&self) -> usize {
        self.capacity + self.fixed.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Settings {
    pub customer_id: String,
    pub domain: String,
    pub tracking_template: String,
    pub default_max_cpc: String,
    pub match_type: String,
    pub app_id: String,
    pub app_store: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CampaignRow {
    pub key: String,
    pub automated: bool,
    pub name: String,
    pub campaign_type: String,
    pub networks: String,
    pub budget: String,
    pub budget_type: String,
    pub bid_strategy: String,
    pub language: String,
    pub location: String,
    pub devices: String,
    pub tracking_template: String,
    pub label: String,
    pub action: StructAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogRow {
    pub item_id: String,
    pub item: String,
    pub category: String,
    pub campaign_key: String,
    pub active: bool,
    pub keyword: String,
    pub final_url: String,
    pub headline_1: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedRow {
    /// Keyword-level action (S4). Distinct from `group_action`.
    pub action: StructAction,
    /// Ad-group-level action for S3/S5/S6. Edit when the live group exists, even if the first
    /// keyword is new.
    pub group_action: StructAction,
    pub campaign_key: String,
    pub campaign: String,
    pub ad_group: String,
    /// Empty on a row that pauses a whole ad group.
    pub keyword: String,
    pub match_type: String,
    pub max_cpc: String,
    pub final_url: String,
    pub path1: String,
    pub path2: String,
    /// 15 entries.
    pub headlines: Vec<String>,
    /// 4 entries.
    pub descriptions: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemplateRow {
    pub campaign_key: String,
    /// 15 entries. `[0]` is the fallback Headline 1.
    pub headlines: Vec<String>,
    /// 4 entries.
    pub descriptions: Vec<String>,
    pub path1: String,
    pub path2: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NegativeRow {
    /// `Campaign` or `Ad group`.
    pub level: String,
    pub campaign: String,
    pub ad_group: String,
    pub keyword: String,
    pub match_type: String,
    pub reason: String,
}

/// Builds the package from the plan. Search campaigns fill the Search tabs, the other kinds fill
/// `image_campaigns` (`images.rs`). With `live`, rows carry `Edit`, `Add` or `Pause` against that
/// account.
pub(crate) fn package(
    input: &Input,
    account: &Account,
    live: Option<&Live>,
) -> Result<Package, ExportError> {
    let search: Vec<&Campaign> = account
        .campaigns
        .iter()
        .filter(|c| c.kind == CampaignKind::Search)
        .collect();
    if search.iter().any(|c| !c.ad_groups.is_empty()) && account.brand_kit.is_none() {
        return Err(ExportError::MissingBrandKit);
    }
    let empty = BrandKit {
        headlines: Vec::new(),
        descriptions: Vec::new(),
    };
    let plan = Plan {
        input,
        kit: account.brand_kit.as_ref().unwrap_or(&empty),
        labels: input.google_ads.labels.join(";"),
        live,
    };
    let mut p = Package {
        folder: folder_name(&input.business.name),
        business: input.business.name.clone(),
        settings: settings(input, account),
        campaigns: Vec::new(),
        catalog: Vec::new(),
        fixed: Vec::new(),
        templates: Vec::new(),
        negatives: Vec::new(),
        capacity: 0,
        automated_campaigns: Vec::new(),
        has_app: !input.google_ads.app_id.trim().is_empty(),
        live: live.is_some(),
        image_campaigns: Vec::new(),
        image_assets: Vec::new(),
        pictures: Vec::new(),
    };
    let mut group_negatives = Vec::new();
    for c in search {
        p.campaigns.push(plan.campaign_row(c));
        p.templates.push(plan.template(c));
        p.negatives
            .extend(c.negatives.iter().map(|n| negative("Campaign", c, "", n)));
        let automated = is_automated(c);
        if automated {
            p.automated_campaigns.push(c.name.clone());
            p.catalog.extend(catalog_rows(input, c));
        }
        // A group of an automated campaign with no catalog item has no feed row to come
        // from, so it is written as a fixed group of that campaign instead of dropped.
        for ag in c
            .ad_groups
            .iter()
            .filter(|ag| !automated || !has_catalog_item(input, c, ag))
        {
            p.fixed.extend(plan.fixed_rows(c, ag));
            group_negatives.extend(
                effective_negatives(c, ag)
                    .into_iter()
                    .map(|n| negative("Ad group", c, &ag.name, n)),
            );
        }
    }
    p.negatives.extend(group_negatives);
    if let Some(live) = live {
        live::dropped(live, account, input, &mut p);
    }
    images::add(input, account, live, &mut p);
    p.capacity = capacity(p.catalog.len());
    check_limits(&p)?;
    Ok(p)
}

/// Search campaigns of catalog intent: one ad group per item, driven by the feed.
fn is_automated(c: &Campaign) -> bool {
    c.kind == CampaignKind::Search && c.intent == Intent::Catalog
}

/// `max(150, ceil(1.5 * rows))`. Every automated campaign shares one block of the feed.
fn capacity(rows: usize) -> usize {
    MIN_CAPACITY.max((3 * rows).div_ceil(2))
}

fn check_limits(p: &Package) -> Result<(), ExportError> {
    if p.capacity > MAX_FEED_ROWS {
        return Err(ExportError::Capacity(format!(
            "{} catalog rows need {} feed rows, over the limit of {MAX_FEED_ROWS}",
            p.catalog.len(),
            p.capacity
        )));
    }
    if p.negatives.len() > MAX_NEGATIVES {
        return Err(ExportError::Capacity(format!(
            "{} negative keyword rows, over the limit of {MAX_NEGATIVES}",
            p.negatives.len()
        )));
    }
    Ok(())
}

fn settings(input: &Input, account: &Account) -> Settings {
    let app_id = input.google_ads.app_id.trim();
    Settings {
        customer_id: customer_cell(&input.google_ads.customer_id).to_string(),
        domain: input.business.url.clone(),
        // Account wide: Google rejects the unknown `{mads_campaign}`, so the ValueTrack
        // `{campaignid}` stands in for it. Campaign rows keep the slug.
        tracking_template: tracking_template(input, "{campaignid}"),
        default_max_cpc: input
            .budget
            .max_cpc
            .or_else(|| median_automated_bid(account))
            .map(money)
            .unwrap_or_default(),
        match_type: "Phrase".into(),
        app_id: app_id.to_string(),
        app_store: if app_id.is_empty() {
            String::new()
        } else {
            store_of(app_id).0.to_string()
        },
    }
}

/// Manual CPC needs a bid or Editor refuses the ad group, so without `budget.max_cpc` the daily
/// groups bid what the plan bid on its catalog groups: the median, the lower one on a tie.
fn median_automated_bid(account: &Account) -> Option<Cents> {
    let mut bids: Vec<Cents> = account
        .campaigns
        .iter()
        .filter(|c| is_automated(c))
        .flat_map(|c| c.ad_groups.iter().map(|g| g.default_cpc))
        .collect();
    bids.sort_by_key(|b| b.0);
    bids.get(bids.len().saturating_sub(1) / 2).copied()
}

fn negative(level: &str, c: &Campaign, ad_group: &str, n: &Keyword) -> NegativeRow {
    NegativeRow {
        level: level.into(),
        campaign: c.name.clone(),
        ad_group: ad_group.into(),
        keyword: n.text.clone(),
        match_type: match_label(n.match_type).into(),
        reason: String::new(),
    }
}

/// One row per catalog item the campaign's planned ad groups list, in plan order, once each.
fn catalog_rows(input: &Input, c: &Campaign) -> Vec<CatalogRow> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for planned in &c.planned_ad_groups {
        let group = c
            .ad_groups
            .iter()
            .find(|g| same_name(&g.name, &planned.name));
        for id in &planned.entity_ids {
            let Some(item) = input.catalog.iter().find(|i| &i.id == id) else {
                continue;
            };
            if !seen.insert(id.as_str()) {
                continue;
            }
            let keyword = group
                .and_then(|g| g.keywords.iter().find(|k| same_name(&k.text, &item.name)))
                .map_or_else(|| item.name.to_lowercase(), |k| k.text.clone());
            out.push(catalog_row(item, c, true, keyword));
        }
    }
    out
}

/// The planned group named like `ag` lists at least one id found in the catalog.
fn has_catalog_item(input: &Input, c: &Campaign, ag: &AdGroup) -> bool {
    c.planned_ad_groups
        .iter()
        .filter(|planned| same_name(&planned.name, &ag.name))
        .flat_map(|planned| &planned.entity_ids)
        .any(|id| input.catalog.iter().any(|i| &i.id == id))
}

fn catalog_row(item: &CatalogItem, c: &Campaign, active: bool, keyword: String) -> CatalogRow {
    CatalogRow {
        item_id: item.id.clone(),
        item: item.name.clone(),
        category: item.category.clone(),
        campaign_key: c.slug.clone(),
        active,
        keyword,
        final_url: item.url.clone(),
        headline_1: headline_1(&item.name),
    }
}

/// The item name when Google accepts it as a headline. Empty sends the feed to the fallback.
fn headline_1(name: &str) -> String {
    if char_len(name) <= HEADLINE_LEN {
        name.to_string()
    } else {
        String::new()
    }
}

fn padded(values: &[String], len: usize) -> Vec<String> {
    let mut out = values.to_vec();
    out.resize(len, String::new());
    out
}

struct Plan<'a> {
    input: &'a Input,
    kit: &'a BrandKit,
    labels: String,
    live: Option<&'a Live>,
}

impl Plan<'_> {
    fn campaign_row(&self, c: &Campaign) -> CampaignRow {
        let business = &self.input.business;
        CampaignRow {
            key: c.slug.clone(),
            automated: is_automated(c),
            name: c.name.clone(),
            campaign_type: c.kind.label().into(),
            networks: "Google search".into(),
            budget: money(c.daily_budget),
            budget_type: "Daily".into(),
            bid_strategy: c.bid_strategy.label().into(),
            language: business.language_primary().into(),
            location: business.locations.first().cloned().unwrap_or_default(),
            devices: self.input.google_ads.devices.trim().into(),
            tracking_template: tracking_template(self.input, &c.slug),
            label: self.labels.clone(),
            action: live::campaign_action(self.live, c),
        }
    }

    fn rsa(&self, ag: Option<&AdGroup>) -> MergedRsa {
        match ag {
            Some(ag) => merge_rsa(&ag.rsa, self.kit),
            None => merge_rsa(&Default::default(), self.kit),
        }
    }

    /// The campaign's ad texts: its first ad group merged with the brand kit.
    fn template(&self, c: &Campaign) -> TemplateRow {
        let ad = self.rsa(c.ad_groups.first());
        TemplateRow {
            campaign_key: c.slug.clone(),
            headlines: padded(&ad.headlines, HEADLINES),
            descriptions: padded(&ad.descriptions, DESCRIPTIONS),
            path1: ad.path1.unwrap_or_default(),
            path2: ad.path2.unwrap_or_default(),
        }
    }

    fn fixed_rows(&self, c: &Campaign, ag: &AdGroup) -> Vec<FixedRow> {
        let ad = self.rsa(Some(ag));
        ag.keywords
            .iter()
            .map(|k| FixedRow {
                action: live::keyword_action(self.live, c, ag, k),
                group_action: live::ad_group_action(self.live, c, ag),
                campaign_key: c.slug.clone(),
                campaign: c.name.clone(),
                ad_group: ag.name.clone(),
                keyword: k.text.clone(),
                match_type: match_label(k.match_type).into(),
                max_cpc: money(ag.default_cpc),
                final_url: ag.final_url.clone(),
                path1: ad.path1.clone().unwrap_or_default(),
                path2: ad.path2.clone().unwrap_or_default(),
                headlines: padded(&ad.headlines, HEADLINES),
                descriptions: padded(&ad.descriptions, DESCRIPTIONS),
                label: self.labels.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;

    fn fixed_row() -> FixedRow {
        FixedRow {
            action: StructAction::Add,
            group_action: StructAction::Add,
            campaign_key: "brand".into(),
            campaign: "Brand".into(),
            ad_group: "Brand".into(),
            keyword: "vinellu".into(),
            match_type: "Phrase".into(),
            max_cpc: "1.00".into(),
            final_url: "https://vinellu.com".into(),
            path1: String::new(),
            path2: String::new(),
            headlines: vec![String::new(); 15],
            descriptions: vec![String::new(); 4],
            label: String::new(),
        }
    }

    #[test]
    fn feed_rows_is_capacity_plus_fixed_rows() {
        let p = Package {
            folder: "Vinellu Ads Automation".into(),
            business: "Vinellu".into(),
            settings: Settings::default(),
            campaigns: Vec::new(),
            catalog: Vec::new(),
            fixed: vec![fixed_row(), fixed_row()],
            templates: Vec::new(),
            negatives: Vec::new(),
            capacity: 150,
            automated_campaigns: Vec::new(),
            has_app: false,
            live: false,
            image_campaigns: Vec::new(),
            image_assets: Vec::new(),
            pictures: Vec::new(),
        };
        assert_eq!(p.feed_rows(), 152);
    }

    #[test]
    fn struct_action_labels() {
        assert_eq!(StructAction::Add.label(), "Add");
        assert_eq!(StructAction::Edit.label(), "Edit");
        assert_eq!(StructAction::Pause.label(), "Pause");
    }
}
