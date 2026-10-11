//! Structural actions against a live account. What exists and what was dropped is decided by
//! `editor.rs` (`dropped_items`, `is_live`, `same_name`): this file only maps it to rows.

use super::images::is_image_campaign;
use super::{
    CampaignRow, CatalogRow, DESCRIPTIONS, FixedRow, HEADLINES, Package, StructAction, catalog_row,
    headline_1, is_automated, match_label,
};
use crate::google::editor::{Dropped, dropped_items, is_live, same_name};
use crate::google::{Account, AdGroup, Campaign, CampaignKind, Keyword};
use crate::input::Input;
use crate::perf::Live;

/// `Edit` for a campaign the live account serves, `Add` otherwise.
pub(super) fn campaign_action(live: Option<&Live>, c: &Campaign) -> StructAction {
    if live.is_some_and(|l| is_live(l, &c.name)) {
        StructAction::Edit
    } else {
        StructAction::Add
    }
}

/// `Edit` when the live ad group already has the keyword with that match type, `Add` otherwise.
pub(super) fn keyword_action(
    live: Option<&Live>,
    c: &Campaign,
    ag: &AdGroup,
    k: &Keyword,
) -> StructAction {
    let exists = live.and_then(|l| live_group(l, c, ag)).is_some_and(|g| {
        g.keywords
            .iter()
            .any(|o| o.match_type == k.match_type && same_name(&o.text, &k.text))
    });
    if exists {
        StructAction::Edit
    } else {
        StructAction::Add
    }
}

/// The live ad group `ag` matches by name. None when the campaign or the group is new.
fn live_group<'a>(live: &'a Live, c: &Campaign, ag: &AdGroup) -> Option<&'a AdGroup> {
    if !is_live(live, &c.name) {
        return None;
    }
    live.baseline
        .campaigns
        .iter()
        .find(|o| same_name(&o.name, &c.name))?
        .ad_groups
        .iter()
        .find(|g| same_name(&g.name, &ag.name))
}

/// Pause rows for what the live account serves and the new plan dropped. A dropped item of an
/// automated campaign stays in `Catalog` as inactive, so the daily track pauses its ad group.
pub(super) fn dropped(live: &Live, account: &Account, input: &Input, p: &mut Package) {
    for d in dropped_items(live, account) {
        match d {
            // `images.rs` pauses the image campaigns.
            Dropped::Campaign { name } if !is_image_campaign(live, name) => {
                p.campaigns.push(paused_campaign(live, name));
            }
            Dropped::AdGroup { campaign, name } if is_automated(campaign) => {
                let listed = p
                    .catalog
                    .iter()
                    .any(|r| r.campaign_key == campaign.slug && same_name(&r.item, name));
                if !listed {
                    p.catalog.push(inactive_item(input, campaign, name));
                }
            }
            Dropped::AdGroup { campaign, name } if campaign.kind == CampaignKind::Search => {
                p.fixed.push(paused_fixed(campaign, name, "", ""));
            }
            Dropped::Keyword {
                campaign,
                group,
                text,
                match_type,
            } if campaign.kind == CampaignKind::Search && !is_automated(campaign) => {
                p.fixed.push(paused_fixed(
                    campaign,
                    &group.name,
                    text,
                    match_label(match_type),
                ));
            }
            Dropped::Campaign { .. }
            | Dropped::AdGroup { .. }
            | Dropped::Keyword { .. }
            | Dropped::AssetGroup { .. } => {}
        }
    }
}

/// Only the key and the name: the row pauses the campaign and changes nothing else.
fn paused_campaign(live: &Live, name: &str) -> CampaignRow {
    let key = live
        .baseline
        .campaigns
        .iter()
        .find(|c| same_name(&c.name, name))
        .map(|c| c.slug.clone())
        .unwrap_or_default();
    CampaignRow {
        key,
        automated: false,
        name: name.into(),
        campaign_type: String::new(),
        networks: String::new(),
        budget: String::new(),
        budget_type: String::new(),
        bid_strategy: String::new(),
        language: String::new(),
        location: String::new(),
        devices: String::new(),
        tracking_template: String::new(),
        label: String::new(),
        action: StructAction::Pause,
    }
}

/// An empty `keyword` pauses the whole ad group.
fn paused_fixed(c: &Campaign, ad_group: &str, keyword: &str, match_type: &str) -> FixedRow {
    FixedRow {
        action: StructAction::Pause,
        campaign_key: c.slug.clone(),
        campaign: c.name.clone(),
        ad_group: ad_group.into(),
        keyword: keyword.into(),
        match_type: match_type.into(),
        max_cpc: String::new(),
        final_url: String::new(),
        path1: String::new(),
        path2: String::new(),
        headlines: vec![String::new(); HEADLINES],
        descriptions: vec![String::new(); DESCRIPTIONS],
        label: String::new(),
    }
}

/// The catalog item the dropped ad group was named after. A group with no such item keeps its
/// name, which is what the feed matches against `Account Status`.
fn inactive_item(input: &Input, c: &Campaign, name: &str) -> CatalogRow {
    match input.catalog.iter().find(|i| same_name(&i.name, name)) {
        Some(item) => catalog_row(item, c, false, item.name.to_lowercase()),
        None => CatalogRow {
            item_id: String::new(),
            item: name.into(),
            category: String::new(),
            campaign_key: c.slug.clone(),
            active: false,
            keyword: name.to_lowercase(),
            final_url: String::new(),
            headline_1: headline_1(name),
        },
    }
}
