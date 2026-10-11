//! `0. Feed.xlsx`: the single source every other workbook mirrors.

use super::columns::{
    DESCRIPTIONS, FeedColumn, HEADLINES, account_status, auto_negatives, campaigns, catalog, fixed,
    image_assets, image_campaigns, negatives, settings, templates,
};
use super::model::{FixedRow, Package, StructAction};
use super::next_step;
use super::sheet::{Cell, FIRST_ROW, Sheet, col_letter, workbook};
use crate::google::ExportError;

const SETTINGS: &str = "Settings";
const CATALOG: &str = "Catalog";
const CAMPAIGNS: &str = "Campaigns";
const AD_TEMPLATES: &str = "Ad Templates";
const ACCOUNT_STATUS: &str = "Account Status";

/// Rows the script may write to `Account Status`, and the bound of every lookup range.
const LOOKUP_ROWS: usize = 1000;

pub(crate) fn feed_workbook(p: &Package) -> Result<Vec<u8>, ExportError> {
    if p.catalog.len() > p.capacity {
        return Err(ExportError::Capacity(format!(
            "{} catalog rows over a feed capacity of {}",
            p.catalog.len(),
            p.capacity
        )));
    }
    workbook(&[
        settings_sheet(p),
        catalog_sheet(p),
        campaigns_sheet(p),
        fixed_sheet(p),
        templates_sheet(p),
        negatives_sheet(p),
        feed_sheet(p),
        account_status_sheet(),
        auto_negatives_sheet(),
        updated_sheet(),
        image_campaigns_sheet(p),
        image_assets_sheet(p),
    ])
}

fn text(s: &str) -> Cell {
    if s.is_empty() {
        Cell::Empty
    } else {
        Cell::Text(s.to_string())
    }
}

fn yes_no(b: bool) -> Cell {
    text(if b { "Yes" } else { "No" })
}

/// `Paused` for a structural row that adds or pauses, empty for an edit that keeps the status.
fn struct_status(action: StructAction) -> Cell {
    match action {
        StructAction::Add | StructAction::Pause => text("Paused"),
        StructAction::Edit => Cell::Empty,
    }
}

/// A row of `width` empty cells with `cells` placed at their column index.
fn row_of(width: usize, cells: Vec<(usize, Cell)>) -> Vec<Cell> {
    let mut row = vec![Cell::Empty; width];
    for (i, cell) in cells {
        if let Some(slot) = row.get_mut(i) {
            *slot = cell;
        }
    }
    row
}

/// A tab name as a formula reference: quoted when it holds a space.
fn tab_ref(tab: &str) -> String {
    if tab.contains(' ') {
        format!("'{tab}'")
    } else {
        tab.to_string()
    }
}

/// Absolute reference to a `Settings` value.
fn setting(offset: usize) -> String {
    format!(
        "{SETTINGS}!${}${}",
        col_letter(settings::VALUE),
        FIRST_ROW + offset
    )
}

/// Reference to column `col` of tab `tab` on Excel row `row`, row relative.
fn cell_ref(tab: &str, col: usize, row: usize) -> String {
    format!("{}!${}{row}", tab_ref(tab), col_letter(col))
}

/// `$X$5:$X$1004` of column `col` in tab `tab`.
fn lookup_range(tab: &str, col: usize) -> String {
    let c = col_letter(col);
    let last = FIRST_ROW + LOOKUP_ROWS - 1;
    format!("{}!${c}${FIRST_ROW}:${c}${last}", tab_ref(tab))
}

/// The value in column `value` of the row of `tab` whose column `key` equals `key_expr`, or
/// `""` when no row matches. `&""` keeps an empty cell empty: a bare reference to one reads as
/// `0`, and a `0` Max CPC pasted into Editor is a zero bid.
fn lookup(tab: &str, key: usize, value: usize, key_expr: &str) -> String {
    format!(
        "IFERROR(INDEX({},MATCH({key_expr},{},0))&\"\",\"\")",
        lookup_range(tab, value),
        lookup_range(tab, key)
    )
}

/// `$X{row}` of a Feed column.
fn feed_ref(col: FeedColumn, row: usize) -> String {
    format!("${}{row}", col.letter())
}

fn template_lookup(col: usize, row: usize) -> String {
    lookup(
        AD_TEMPLATES,
        templates::KEY,
        col,
        &feed_ref(FeedColumn::CampaignKey, row),
    )
}

/// Column C of `Settings`, indexed like `settings::LABELS`.
const SETTING_NOTES: [&str; 7] = [
    "Google Ads account ID, 000-000-0000.",
    "Site the ads point to.",
    "Tracking template of new campaigns.",
    "Max CPC of automated ad groups.",
    "Match type of automated keywords.",
    "App ID or package name for the app extension.",
    "Google Play or Apple App Store.",
];

fn settings_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        SETTINGS,
        "Settings",
        "Every tab and every file of the package reads these cells.",
        &settings::HEADER,
    );
    sheet.note = text("Fill the yellow cells before going live.");
    let s = &p.settings;
    let values = [
        (settings::CUSTOMER_ID, &s.customer_id),
        (settings::DOMAIN, &s.domain),
        (settings::TRACKING_TEMPLATE, &s.tracking_template),
        (settings::DEFAULT_MAX_CPC, &s.default_max_cpc),
        (settings::MATCH_TYPE, &s.match_type),
        (settings::APP_ID, &s.app_id),
        (settings::APP_STORE, &s.app_store),
    ];
    let mut rows = vec![Vec::new(); settings::LABELS.len()];
    for (offset, value) in values {
        let label = settings::LABELS.get(offset).copied().unwrap_or_default();
        let note = SETTING_NOTES.get(offset).copied().unwrap_or_default();
        if let Some(row) = rows.get_mut(offset) {
            *row = vec![text(label), text(value), text(note)];
        }
    }
    sheet.rows = rows;
    sheet.pending = pending(p);
    sheet
}

/// Settings cells a person must fill before going live.
fn pending(p: &Package) -> Vec<(usize, usize)> {
    let s = &p.settings;
    let missing = [
        (settings::CUSTOMER_ID, s.customer_id.is_empty()),
        (settings::DEFAULT_MAX_CPC, s.default_max_cpc.is_empty()),
        (
            settings::APP_ID,
            !p.automated_campaigns.is_empty() && s.app_id.is_empty(),
        ),
    ];
    let mut out: Vec<(usize, usize)> = missing
        .iter()
        .filter(|(_, empty)| *empty)
        .map(|&(offset, _)| (offset, settings::VALUE))
        .collect();
    out.sort_unstable();
    out
}

fn catalog_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        CATALOG,
        "Catalog",
        "One row per item of the automated campaigns. The Feed reads it row by row.",
        &catalog::HEADER,
    );
    sheet.note = text(&format!(
        "Rows 5 to {}. New rows fill item, campaign_key, active, keyword and final_url.",
        FIRST_ROW + p.capacity - 1
    ));
    let width = catalog::HEADER.len();
    sheet.rows = p
        .catalog
        .iter()
        .map(|c| {
            row_of(
                width,
                vec![
                    (catalog::ITEM_ID, text(&c.item_id)),
                    (catalog::ITEM, text(&c.item)),
                    (catalog::CATEGORY, text(&c.category)),
                    (catalog::CAMPAIGN_KEY, text(&c.campaign_key)),
                    (catalog::ACTIVE, yes_no(c.active)),
                    (catalog::KEYWORD, text(&c.keyword)),
                    (catalog::FINAL_URL, text(&c.final_url)),
                    (catalog::HEADLINE_1, text(&c.headline_1)),
                ],
            )
        })
        .collect();
    sheet
}

fn campaigns_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        CAMPAIGNS,
        "Campaigns",
        "One row per Search campaign. Every other tab reads the campaign name from here.",
        &campaigns::HEADER,
    );
    let width = campaigns::HEADER.len();
    sheet.rows = p
        .campaigns
        .iter()
        .map(|c| {
            row_of(
                width,
                vec![
                    (campaigns::KEY, text(&c.key)),
                    (campaigns::AUTOMATED, yes_no(c.automated)),
                    (campaigns::CAMPAIGN, text(&c.name)),
                    (campaigns::CAMPAIGN_TYPE, text(&c.campaign_type)),
                    (campaigns::NETWORKS, text(&c.networks)),
                    (campaigns::BUDGET, text(&c.budget)),
                    (campaigns::BUDGET_TYPE, text(&c.budget_type)),
                    (campaigns::BID_STRATEGY, text(&c.bid_strategy)),
                    (campaigns::LANGUAGE, text(&c.language)),
                    (campaigns::LOCATION, text(&c.location)),
                    (campaigns::DEVICES, text(&c.devices)),
                    (campaigns::TRACKING_TEMPLATE, text(&c.tracking_template)),
                    (campaigns::LABEL, text(&c.label)),
                    (campaigns::ACTION, text(c.action.label())),
                ],
            )
        })
        .collect();
    sheet
}

fn fixed_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        "Fixed Keywords",
        "Fixed Keywords",
        "Keywords of the campaigns that are not automated.",
        &fixed::HEADER,
    );
    let width = fixed::HEADER.len();
    sheet.rows = p
        .fixed
        .iter()
        .map(|f| {
            row_of(
                width,
                vec![
                    (fixed::ACTION, text(f.action.label())),
                    (fixed::CAMPAIGN_KEY, text(&f.campaign_key)),
                    (fixed::AD_GROUP, text(&f.ad_group)),
                    (fixed::KEYWORD, text(&f.keyword)),
                    (fixed::MATCH_TYPE, text(&f.match_type)),
                    (fixed::MAX_CPC, text(&f.max_cpc)),
                    (fixed::FINAL_URL, text(&f.final_url)),
                    (fixed::STATUS, struct_status(f.action)),
                ],
            )
        })
        .collect();
    sheet
}

fn templates_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        AD_TEMPLATES,
        "Ad Templates",
        "Ad text per campaign. Changing a cell changes every ad of that campaign.",
        &templates::HEADER,
    );
    sheet.note = text("Headline 1 is the fallback for items and ad groups without one.");
    let width = templates::HEADER.len();
    sheet.rows = p
        .templates
        .iter()
        .map(|t| {
            let mut cells = vec![
                (templates::KEY, text(&t.campaign_key)),
                (templates::PATH_1, text(&t.path1)),
                (templates::PATH_2, text(&t.path2)),
            ];
            let h = t.headlines.iter().take(HEADLINES as usize).enumerate();
            cells.extend(h.map(|(i, s)| (templates::headline(i + 1), text(s))));
            let d = t
                .descriptions
                .iter()
                .take(DESCRIPTIONS as usize)
                .enumerate();
            cells.extend(d.map(|(i, s)| (templates::description(i + 1), text(s))));
            row_of(width, cells)
        })
        .collect();
    sheet
}

fn negatives_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        "Negatives",
        "Negatives",
        "Structural negative keywords, at campaign or ad group level.",
        &negatives::HEADER,
    );
    let width = negatives::HEADER.len();
    sheet.rows = p
        .negatives
        .iter()
        .map(|n| {
            row_of(
                width,
                vec![
                    (negatives::LEVEL, text(&n.level)),
                    (negatives::CAMPAIGN, text(&n.campaign)),
                    (negatives::AD_GROUP, text(&n.ad_group)),
                    (negatives::KEYWORD, text(&n.keyword)),
                    (negatives::MATCH_TYPE, text(&n.match_type)),
                    (negatives::REASON, text(&n.reason)),
                ],
            )
        })
        .collect();
    sheet
}

fn feed_sheet(p: &Package) -> Sheet {
    let columns = FeedColumn::all();
    let mut sheet = Sheet::new(
        "Feed",
        "Feed",
        "One row per ad group and keyword. Do not edit: change Catalog, Fixed Keywords or Ad Templates.",
        &[],
    );
    sheet.header = columns.iter().map(|c| c.header()).collect();
    sheet.note = text("next_step compares what the feed wants with what the account has.");
    let width = columns.len();
    sheet.rows = (0..p.capacity)
        .map(|i| automated_row(FIRST_ROW + i, width))
        .chain(
            p.fixed
                .iter()
                .enumerate()
                .map(|(i, f)| fixed_row(f, FIRST_ROW + p.capacity + i, width)),
        )
        .collect();
    sheet
}

/// Feed row `row` of the automated block: formulas over `Catalog` row `row`.
fn automated_row(row: usize, width: usize) -> Vec<Cell> {
    use FeedColumn as F;
    let cat = |col| cell_ref(CATALOG, col, row);
    let item = cat(catalog::ITEM);
    let passthrough = |col| format!("=IF({}=\"\",\"\",{})", cat(col), cat(col));
    let when_item = |expr: String| format!("=IF({item}=\"\",\"\",{expr})");
    let key = feed_ref(F::CampaignKey, row);
    let campaign = |col| format!("={}", lookup(CAMPAIGNS, campaigns::KEY, col, &key));
    let headline_1 = format!(
        "=IF({h}<>\"\",{h},{})",
        template_lookup(templates::HEADLINE_1, row),
        h = cat(catalog::HEADLINE_1)
    );
    let mut cells = vec![
        (
            F::Action,
            format!(
                "=IF(AND({item}<>\"\",{}=\"Yes\"),\"Add\",\"-\")",
                cat(catalog::ACTIVE)
            ),
        ),
        (F::CampaignKey, passthrough(catalog::CAMPAIGN_KEY)),
        (F::Item, passthrough(catalog::ITEM)),
        (F::Category, passthrough(catalog::CATEGORY)),
        (F::Active, passthrough(catalog::ACTIVE)),
        (F::Campaign, campaign(campaigns::CAMPAIGN)),
        (F::AdGroup, passthrough(catalog::ITEM)),
        (F::Keyword, passthrough(catalog::KEYWORD)),
        (
            F::MatchType,
            when_item(format!("{}&\"\"", setting(settings::MATCH_TYPE))),
        ),
        (
            F::MaxCpc,
            when_item(format!("{}&\"\"", setting(settings::DEFAULT_MAX_CPC))),
        ),
        (F::FinalUrl, passthrough(catalog::FINAL_URL)),
        (F::Headline(1), headline_1),
        (F::Label, campaign(campaigns::LABEL)),
        (F::Status, "=\"\"".to_string()),
        (F::CurrentStatus, current_status_formula(row)),
        (F::NextStep, next_step::formula(row)),
    ];
    cells.extend(template_formulas(row));
    let mut out: Vec<(usize, Cell)> = cells
        .into_iter()
        .map(|(c, f)| (c.index(), Cell::Formula(f)))
        .collect();
    out.push((F::Automated.index(), text("Yes")));
    out.push((F::Structure.index(), text("SKAG")));
    row_of(width, out)
}

/// Paths, Headline 2 to 15 and the descriptions of Feed row `row`, from `Ad Templates`.
fn template_formulas(row: usize) -> Vec<(FeedColumn, String)> {
    let template = |col| format!("={}", template_lookup(col, row));
    let mut out = vec![
        (FeedColumn::Path1, template(templates::PATH_1)),
        (FeedColumn::Path2, template(templates::PATH_2)),
    ];
    for n in 2..=HEADLINES {
        out.push((
            FeedColumn::Headline(n),
            template(templates::headline(n as usize)),
        ));
    }
    for n in 1..=DESCRIPTIONS {
        out.push((
            FeedColumn::Description(n),
            template(templates::description(n as usize)),
        ));
    }
    out
}

/// Status of the Feed row's `Campaign|Ad group` in `Account Status`, empty when absent.
fn current_status_formula(row: usize) -> String {
    let key = format!(
        "{}&\"|\"&{}",
        feed_ref(FeedColumn::Campaign, row),
        feed_ref(FeedColumn::AdGroup, row)
    );
    let status = lookup(
        ACCOUNT_STATUS,
        account_status::KEY,
        account_status::STATUS,
        &key,
    );
    format!("={status}")
}

/// Feed row `row` of the fixed block: literal values from the plan.
fn fixed_row(f: &FixedRow, row: usize, width: usize) -> Vec<Cell> {
    use FeedColumn as F;
    let mut cells = vec![
        (F::Action, text(f.action.label())),
        (F::CampaignKey, text(&f.campaign_key)),
        (F::Automated, text("No")),
        (F::Structure, text("Thematic")),
        (F::Campaign, text(&f.campaign)),
        (F::AdGroup, text(&f.ad_group)),
        (F::Keyword, text(&f.keyword)),
        (F::MatchType, text(&f.match_type)),
        (F::MaxCpc, text(&f.max_cpc)),
        (F::FinalUrl, text(&f.final_url)),
        (F::Path1, text(&f.path1)),
        (F::Path2, text(&f.path2)),
        (F::Label, text(&f.label)),
        (F::Status, struct_status(f.action)),
    ];
    for (i, h) in f.headlines.iter().take(HEADLINES as usize).enumerate() {
        cells.push((F::Headline(i as u8 + 1), text(h)));
    }
    if f.headlines.first().is_none_or(|h| h.is_empty()) {
        let fallback = template_lookup(templates::HEADLINE_1, row);
        cells.push((F::Headline(1), Cell::Formula(format!("={fallback}"))));
    }
    for (i, d) in f
        .descriptions
        .iter()
        .take(DESCRIPTIONS as usize)
        .enumerate()
    {
        cells.push((F::Description(i as u8 + 1), text(d)));
    }
    row_of(
        width,
        cells.into_iter().map(|(c, v)| (c.index(), v)).collect(),
    )
}

fn account_status_sheet() -> Sheet {
    let mut sheet = Sheet::new(
        ACCOUNT_STATUS,
        "Account Status",
        "Written every night by daily-sync.js. Do not edit: the script rewrites the whole block.",
        &account_status::HEADER,
    );
    sheet.note = text("Empty means an empty account: every automated ad group gets CREATE.");
    let width = account_status::HEADER.len();
    let campaign = col_letter(account_status::CAMPAIGN);
    let ad_group = col_letter(account_status::AD_GROUP);
    sheet.rows = (FIRST_ROW..FIRST_ROW + LOOKUP_ROWS)
        .map(|r| {
            let key = format!("=IF(${campaign}{r}=\"\",\"\",${campaign}{r}&\"|\"&${ad_group}{r})");
            row_of(width, vec![(account_status::KEY, Cell::Formula(key))])
        })
        .collect();
    sheet
}

fn auto_negatives_sheet() -> Sheet {
    let mut sheet = Sheet::new(
        "Auto Negatives",
        "Auto Negatives",
        "Ad group negatives the feed needs today, written every night by daily-sync.js.",
        &auto_negatives::HEADER,
    );
    sheet.note = text("Do not edit: the script rewrites the whole block. D7 reads this tab.");
    sheet
}

fn updated_sheet() -> Sheet {
    let mut sheet = Sheet::new(
        "Updated",
        "Updated",
        "When the catalog was last refreshed.",
        &["Updated at", "Source"],
    );
    sheet.note = text("Filled by the person or the app that rewrites Catalog.");
    sheet
}

fn image_campaigns_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        "Image Campaigns",
        "Image Campaigns",
        "Performance Max, Demand Gen and App campaigns. setupImageCampaigns() in daily-sync.js creates them.",
        &image_campaigns::HEADER,
    );
    sheet.note = text("New campaigns start paused. An existing one only gets its budget updated.");
    let width = image_campaigns::HEADER.len();
    sheet.rows = p
        .image_campaigns
        .iter()
        .map(|c| {
            use image_campaigns as C;
            row_of(
                width,
                vec![
                    (C::KEY, text(&c.key)),
                    (C::CAMPAIGN, text(&c.name)),
                    (C::CAMPAIGN_TYPE, text(&c.campaign_type)),
                    (C::BUDGET, text(&c.budget)),
                    (C::BID_STRATEGY, text(&c.bid_strategy)),
                    (C::MAX_CPC, text(&c.max_cpc)),
                    (C::LANGUAGE, text(&c.language)),
                    (C::LOCATION, text(&c.location)),
                    (C::FINAL_URL, text(&c.final_url)),
                    (C::BUSINESS_NAME, text(&c.business_name)),
                    (C::APP_ID, text(&c.app_id)),
                    (C::APP_STORE, text(&c.app_store)),
                    (C::ACTION, text(c.action.label())),
                ],
            )
        })
        .collect();
    sheet
}

fn image_assets_sheet(p: &Package) -> Sheet {
    let mut sheet = Sheet::new(
        "Image Assets",
        "Image Assets",
        "Texts and pictures of every asset group or ad group of the image campaigns.",
        &image_assets::HEADER,
    );
    sheet.note = text("Image file is relative to the package's Images folder.");
    let width = image_assets::HEADER.len();
    sheet.rows = p
        .image_assets
        .iter()
        .map(|a| {
            use image_assets as A;
            row_of(
                width,
                vec![
                    (A::KEY, text(&a.campaign_key)),
                    (A::GROUP, text(&a.group)),
                    (A::FIELD, text(&a.field)),
                    (A::TEXT, text(&a.text)),
                    (A::IMAGE_FILE, text(&a.image_file)),
                ],
            )
        })
        .collect();
    sheet
}

#[cfg(test)]
mod tests;
