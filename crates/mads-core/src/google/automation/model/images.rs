//! Performance Max, Demand Gen and App campaigns for the script's `setupImageCampaigns()`:
//! the `Image Campaigns` and `Image Assets` tabs and the pictures they name.

use super::{ImageAssetRow, ImageCampaignRow, Package, PictureCopy, StructAction, store_of};
use crate::google::editor::{Dropped, dropped_items, is_live, money, same_name};
use crate::google::{Account, AspectRatio, AssetGroup, BidStrategy, Campaign, CampaignKind};
use crate::images::logo_rel_path;
use crate::input::{AppStore, Input};
use crate::perf::Live;

const NO_PICTURES: &str = "no picture files";
const NO_LOGO: &str = "no logo (business.toml logo)";
const NO_APP_ID: &str = "no app id (google_ads.app_id or [app])";

/// Image campaigns the package leaves out: their index in `account.campaigns` and what they
/// miss, as one sentence. Warning W12 reads it.
pub fn image_campaign_gaps(input: &Input, account: &Account) -> Vec<(usize, String)> {
    account
        .campaigns
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let missing = missing(input, c);
            (!missing.is_empty()).then(|| (i, missing.join(", ")))
        })
        .collect()
}

fn missing(input: &Input, c: &Campaign) -> Vec<&'static str> {
    let mut out = Vec::new();
    if campaign_type(c.kind).is_none() {
        return out;
    }
    if !c.asset_groups.iter().any(|g| pictures(g).next().is_some()) {
        out.push(NO_PICTURES);
    }
    if has_logo(c.kind) && logo_rel_path(input).is_none() {
        out.push(NO_LOGO);
    }
    if c.kind == CampaignKind::AppInstalls && app(input).is_none() {
        out.push(NO_APP_ID);
    }
    out
}

/// Fills the two tabs and the picture copies. With `live`, a campaign the account serves is
/// `Edit`, and an image campaign the new plan dropped gets a `Pause` row.
pub(super) fn add(input: &Input, account: &Account, live: Option<&Live>, p: &mut Package) {
    let gaps = image_campaign_gaps(input, account);
    let logo = logo_rel_path(input);
    let mut needs_logo = false;
    for (i, c) in account.campaigns.iter().enumerate() {
        let Some(kind) = campaign_type(c.kind) else {
            continue;
        };
        if gaps.iter().any(|(g, _)| *g == i) {
            continue;
        }
        p.image_campaigns.push(row(input, live, c, kind));
        let brand = logo
            .as_deref()
            .filter(|_| has_logo(c.kind))
            .map(|rel| Brand {
                logo: logo_name(rel),
                name: &input.business.name,
            });
        needs_logo |= brand.is_some();
        for g in &c.asset_groups {
            assets(c, g, brand.as_ref(), &mut p.image_assets, &mut p.pictures);
        }
    }
    if let Some(from) = logo.filter(|_| needs_logo) {
        let to = logo_name(&from).to_string();
        push_once(&mut p.pictures, PictureCopy { from, to });
    }
    if let Some(live) = live {
        p.image_campaigns.extend(paused(live, account));
    }
}

/// The `Campaign type` cell. None for a Search campaign.
fn campaign_type(kind: CampaignKind) -> Option<&'static str> {
    match kind {
        CampaignKind::Search => None,
        CampaignKind::PerformanceMax => Some("Performance Max"),
        CampaignKind::DemandGen => Some("Demand Gen"),
        CampaignKind::AppInstalls => Some("App"),
    }
}

/// Performance Max and Demand Gen show the business name and logo. App ads use the app's.
fn has_logo(kind: CampaignKind) -> bool {
    matches!(kind, CampaignKind::PerformanceMax | CampaignKind::DemandGen)
}

/// The run copies the logo to `editor/images/logo.<ext>`. The package keeps it at
/// `Images/logo.<ext>`.
fn logo_name(rel: &str) -> &str {
    rel.strip_prefix("images/").unwrap_or(rel)
}

/// App id and store: `[app]` first, then `google_ads.app_id` with the store its shape implies.
/// The store names match `Settings.app_store`, which the script already reads.
fn app(input: &Input) -> Option<(String, &'static str)> {
    if let Some(app) = &input.app {
        let store = match app.store {
            AppStore::GooglePlay => "Google Play",
            AppStore::AppStore => "Apple App Store",
        };
        let id = app.id.trim();
        return (!id.is_empty()).then(|| (id.to_string(), store));
    }
    let id = input.google_ads.app_id.trim();
    (!id.is_empty()).then(|| (id.to_string(), store_of(id).0))
}

/// `value` unless blank, else `fallback`.
fn or(value: Option<&String>, fallback: &str) -> String {
    value
        .filter(|v| !v.trim().is_empty())
        .map_or_else(|| fallback.to_string(), Clone::clone)
}

fn row(input: &Input, live: Option<&Live>, c: &Campaign, kind: &str) -> ImageCampaignRow {
    let business = &input.business;
    let first = c.asset_groups.first();
    let (app_id, app_store) = match c.kind {
        CampaignKind::AppInstalls => app(input).unwrap_or_default(),
        _ => (String::new(), ""),
    };
    let (bid_strategy, max_cpc) = match c.bid_strategy {
        BidStrategy::ManualCpc => ("manual_cpc", String::new()),
        BidStrategy::MaximizeClicks { max_cpc } => {
            ("maximize_clicks", max_cpc.map(money).unwrap_or_default())
        }
        BidStrategy::MaximizeConversions => ("maximize_conversions", String::new()),
    };
    ImageCampaignRow {
        key: c.slug.clone(),
        name: c.name.clone(),
        campaign_type: kind.into(),
        budget: money(c.daily_budget),
        bid_strategy: bid_strategy.into(),
        max_cpc,
        language: business.language_primary().into(),
        location: business.locations.first().cloned().unwrap_or_default(),
        final_url: or(first.map(|g| &g.final_url), &business.url),
        business_name: or(first.map(|g| &g.business_name), &business.name),
        app_id,
        app_store: app_store.into(),
        action: if live.is_some_and(|l| is_live(l, &c.name)) {
            StructAction::Edit
        } else {
            StructAction::Add
        },
    }
}

/// Briefs that became a file, with the file's path in `editor/`.
fn pictures(g: &AssetGroup) -> impl Iterator<Item = (AspectRatio, &str)> {
    g.images.iter().filter_map(|b| {
        let file = b.file.as_deref().filter(|f| !f.trim().is_empty())?;
        Some((b.ratio, file))
    })
}

fn picture_field(ratio: AspectRatio) -> &'static str {
    match ratio {
        AspectRatio::Landscape => "MARKETING_IMAGE",
        AspectRatio::Square => "SQUARE_MARKETING_IMAGE",
        AspectRatio::Portrait => "PORTRAIT_MARKETING_IMAGE",
        AspectRatio::Vertical => "TALL_PORTRAIT_MARKETING_IMAGE",
    }
}

/// Logo file in `Images/` and the business name to fall back on.
struct Brand<'a> {
    logo: &'a str,
    name: &'a str,
}

/// Rows of one asset group: texts, then business name and logo, then one per picture.
fn assets(
    c: &Campaign,
    g: &AssetGroup,
    brand: Option<&Brand>,
    rows: &mut Vec<ImageAssetRow>,
    copies: &mut Vec<PictureCopy>,
) {
    let row = |field: &str, text: &str, image_file: &str| ImageAssetRow {
        campaign_key: c.slug.clone(),
        group: g.name.clone(),
        field: field.into(),
        text: text.into(),
        image_file: image_file.into(),
    };
    for (field, values) in [
        ("HEADLINE", &g.headlines),
        ("LONG_HEADLINE", &g.long_headlines),
        ("DESCRIPTION", &g.descriptions),
    ] {
        rows.extend(values.iter().map(|v| row(field, v, "")));
    }
    if c.kind == CampaignKind::PerformanceMax {
        rows.extend(g.search_themes.iter().map(|v| row("SEARCH_THEME", v, "")));
    }
    if let Some(brand) = brand {
        rows.push(row(
            "BUSINESS_NAME",
            &or(Some(&g.business_name), brand.name),
            "",
        ));
        rows.push(row("LOGO", "", brand.logo));
    }
    for (ratio, file) in pictures(g) {
        let name = file.rsplit('/').next().unwrap_or(file);
        let to = format!("{}/{name}", c.slug);
        rows.push(row(picture_field(ratio), "", &to));
        push_once(
            copies,
            PictureCopy {
                from: file.to_string(),
                to,
            },
        );
    }
}

fn push_once(copies: &mut Vec<PictureCopy>, copy: PictureCopy) {
    if !copies.contains(&copy) {
        copies.push(copy);
    }
}

/// `Pause` rows for the image campaigns the live account serves and the new plan dropped. Only
/// the key, the name and the type: the row pauses the campaign and changes nothing else.
fn paused(live: &Live, account: &Account) -> Vec<ImageCampaignRow> {
    dropped_items(live, account)
        .into_iter()
        .filter_map(|d| match d {
            Dropped::Campaign { name } => paused_row(live, name),
            _ => None,
        })
        .collect()
}

/// None for a campaign the baseline lacks or a Search campaign: `live::dropped` owns those.
fn paused_row(live: &Live, name: &str) -> Option<ImageCampaignRow> {
    let old = live
        .baseline
        .campaigns
        .iter()
        .find(|c| same_name(&c.name, name))?;
    Some(ImageCampaignRow {
        key: old.slug.clone(),
        name: name.into(),
        campaign_type: campaign_type(old.kind)?.into(),
        budget: String::new(),
        bid_strategy: String::new(),
        max_cpc: String::new(),
        language: String::new(),
        location: String::new(),
        final_url: String::new(),
        business_name: String::new(),
        app_id: String::new(),
        app_store: String::new(),
        action: StructAction::Pause,
    })
}

/// True when the live baseline holds `name` as an image campaign.
pub(super) fn is_image_campaign(live: &Live, name: &str) -> bool {
    live.baseline
        .campaigns
        .iter()
        .find(|c| same_name(&c.name, name))
        .is_some_and(|c| campaign_type(c.kind).is_some())
}

#[cfg(test)]
mod tests;
