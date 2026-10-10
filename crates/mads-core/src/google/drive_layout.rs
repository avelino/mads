//! Vaz's structural folder, written next to the run and pasted by hand into Google Ads Editor.
//!
//! Nothing here uploads to Drive. B1 creates campaigns paused. B2 turns them on. Editor matches
//! campaigns and ad groups by name, so these files carry no ids.
//!
//! B1, B3, B4 and B5 use the headers of Vaz's sheets. B2, B6, B7 and B8 use headers from
//! [CSV file columns](https://support.google.com/google-ads/editor/answer/57747) and, for the
//! account id, [Make changes to multiple accounts](https://support.google.com/google-ads/editor/answer/7412706).
//! B8 also follows [About app assets](https://support.google.com/google-ads/answer/2402582).

use super::editor::{Row, editor_sheet, money};
use super::export::effective_negatives;
use super::{Account, Campaign, CampaignKind, CsvFile, ExportError, Issue, MatchType, merge_rsa};
use crate::{google::char_len, input::Input};
use serde::{Deserialize, Serialize};

/// Which CSV set `generate` and `export` write. `Bulk` is the default. Saved in `run.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportLayout {
    #[default]
    Bulk,
    DriveFolders,
}

const B1: &str = "drive/B - Estrutural/B1-campanhas.csv";
const B2: &str = "drive/B - Estrutural/B2-status-campanha.csv";
const B3: &str = "drive/B - Estrutural/B3-grupos.csv";
const B4: &str = "drive/B - Estrutural/B4-keywords.csv";
const B5: &str = "drive/B - Estrutural/B5-anuncios.csv";
const B6: &str = "drive/B - Estrutural/B6-utm.csv";
const B7: &str = "drive/B - Estrutural/B7-negativas.csv";
const B8: &str = "drive/B - Estrutural/B8-extensao-app.csv";
const README: &str = "drive/B - Estrutural/LEIA-ME.md";

/// The structural track: B1 to B7, B8 when `google_ads.app_id` is set, then `LEIA-ME.md`.
pub fn export_drive_layout(input: &Input, account: &Account) -> Result<Vec<CsvFile>, ExportError> {
    let search_ads = account
        .campaigns
        .iter()
        .any(|c| c.kind == CampaignKind::Search && !c.ad_groups.is_empty());
    if search_ads && account.brand_kit.is_none() {
        return Err(ExportError::MissingBrandKit);
    }
    let kit = account.brand_kit.as_ref();
    let id = customer_cell(&input.google_ads.customer_id);
    let labels = input.google_ads.labels.join(";");
    let devices = input.google_ads.devices.trim();
    let app_id = input.google_ads.app_id.trim();

    let mut campaigns = Vec::new();
    let mut statuses = Vec::new();
    let mut groups = Vec::new();
    let mut keywords = Vec::new();
    let mut ads = Vec::new();
    let mut utm = Vec::new();
    let mut negatives = Vec::new();
    let mut apps = Vec::new();

    for c in account
        .campaigns
        .iter()
        .filter(|c| c.kind == CampaignKind::Search)
    {
        let template = tracking_template(input, &c.slug);
        campaigns.push(campaign_row(c, input, id, devices, &labels, &template));
        statuses.push(
            Row::default()
                .set("Action", "Edit")
                .set("Customer ID", id)
                .set("Campaign", &c.name)
                .set("Campaign status", "Enabled"),
        );
        for ag in &c.ad_groups {
            groups.push(
                Row::default()
                    .set("Action", "Add")
                    .set("Customer ID", id)
                    .set("Campaign", &c.name)
                    .set("Ad group", &ag.name)
                    .set("Label", &labels)
                    .set("Ad group status", "Enabled")
                    .set("Max CPC", money(ag.default_cpc)),
            );
            for k in &ag.keywords {
                keywords.push(
                    Row::default()
                        .set("Action", "Add")
                        .set("Customer ID", id)
                        .set("Campaign", &c.name)
                        .set("Ad group", &ag.name)
                        .set("Keyword", &k.text)
                        .set("Match Type", match_label(k.match_type))
                        .set("Status", "Paused")
                        .set("Final URL", &ag.final_url),
                );
            }
            if let Some(kit) = kit {
                let ad = merge_rsa(&ag.rsa, kit);
                ads.push(
                    Row::default()
                        .set("Action", "Add")
                        .set("Ad status", "Paused")
                        .set("Customer ID", id)
                        .set("Campaign", &c.name)
                        .set("Ad group", &ag.name)
                        .set("Ad type", "Responsive search ad")
                        .set("Label", &labels)
                        .numbered("Headline", 1, &ad.headlines)
                        .numbered("Description", 1, &ad.descriptions)
                        .set("Headline 1 position", "1")
                        .set("Path 1", ad.path1.unwrap_or_default())
                        .set("Path 2", ad.path2.unwrap_or_default())
                        .set("Final URL", &ag.final_url),
                );
            }
            utm.push(
                Row::default()
                    .set("Action", "Add")
                    .set("Customer ID", id)
                    .set("Campaign", &c.name)
                    .set("Ad group", &ag.name)
                    .set("Tracking template", &template),
            );
            for n in effective_negatives(c, ag) {
                negatives.push(
                    Row::default()
                        .set("Action", "Add")
                        .set("Customer ID", id)
                        .set("Campaign", &c.name)
                        .set("Ad group", &ag.name)
                        .set("Keyword", &n.text)
                        .set("Criterion Type", negative_label(n.match_type)),
                );
            }
        }
        if !app_id.is_empty() {
            let (store, url) = store_of(app_id);
            apps.push(
                Row::default()
                    .set("Action", "Add")
                    .set("Customer ID", id)
                    .set("Campaign", &c.name)
                    .set("Link Text", link_text(&input.business.name))
                    .set("App ID / Package name", app_id)
                    .set("App store", store)
                    .set("Final URL", url),
            );
        }
    }

    let mut files = vec![
        editor_sheet(B1, B1_HEADER, campaigns),
        editor_sheet(B2, B2_HEADER, statuses),
        editor_sheet(B3, B3_HEADER, groups),
        editor_sheet(B4, B4_HEADER, keywords),
        editor_sheet(B5, &b5_columns(), ads),
        editor_sheet(B6, B6_HEADER, utm),
        editor_sheet(B7, B7_HEADER, negatives),
    ];
    if !app_id.is_empty() {
        files.push(editor_sheet(B8, B8_HEADER, apps));
    }
    files.push(CsvFile {
        name: README,
        bytes: readme(app_id.is_empty()).into_bytes(),
    });
    Ok(files)
}

/// What the drive files leave out: a blank Customer ID (W11) and every campaign that is not
/// Search (W12). The files hold the Search structure only.
pub fn drive_warnings(input: &Input, account: &Account) -> Vec<Issue> {
    let mut out = Vec::new();
    if customer_cell(&input.google_ads.customer_id).is_empty() {
        out.push(Issue::warning(
            "W11",
            "google_ads.customer_id",
            "Customer ID is empty or the 000-000-0000 placeholder, so the drive files leave that column blank. Set google_ads.customer_id to the real account id before import.",
        ));
    }
    for (i, c) in account.campaigns.iter().enumerate() {
        if c.kind != CampaignKind::Search {
            out.push(Issue::warning(
                "W12",
                format!("campaigns[{i}]"),
                format!(
                    "{} campaign '{}' is not in the drive files, which hold Search only. Export it with --layout bulk.",
                    c.kind.label(),
                    c.name
                ),
            ));
        }
    }
    out
}

const B1_HEADER: &[&str] = &[
    "Action",
    "Campaign status",
    "Customer ID",
    "Campaign",
    "Campaign type",
    "Networks",
    "Budget",
    "Budget type",
    "Bid Strategy Type",
    "Campaign start date",
    "Language",
    "Location",
    "Devices",
    "Tracking template",
    "Label",
];

/// `Campaign status` is `Enabled`, `Paused` or `Removed`.
/// <https://support.google.com/google-ads/editor/answer/57747>
///
/// `Customer ID` is the account column of a multi-account import.
/// <https://support.google.com/google-ads/editor/answer/7412706>
const B2_HEADER: &[&str] = &["Action", "Customer ID", "Campaign", "Campaign status"];

const B3_HEADER: &[&str] = &[
    "Action",
    "Customer ID",
    "Campaign",
    "Ad group",
    "Label",
    "Ad group status",
    "Max CPC",
];

const B4_HEADER: &[&str] = &[
    "Action",
    "Customer ID",
    "Campaign",
    "Ad group",
    "Keyword",
    "Match Type",
    "Status",
    "Max CPC",
    "Final URL",
];

/// `Tracking template` holds one URL template.
/// <https://support.google.com/google-ads/editor/answer/57747>
const B6_HEADER: &[&str] = &[
    "Action",
    "Customer ID",
    "Campaign",
    "Ad group",
    "Tracking template",
];

/// `Criterion Type` is the column Editor calls `Type` (`Negative Phrase`, `Negative Exact`).
/// <https://support.google.com/google-ads/editor/answer/57747>
const B7_HEADER: &[&str] = &[
    "Action",
    "Customer ID",
    "Campaign",
    "Ad group",
    "Keyword",
    "Criterion Type",
];

/// `App ID / Package name`, `App store` and `Link Text` are Editor columns. The store URL is the
/// page the app asset links to.
/// <https://support.google.com/google-ads/editor/answer/57747>
/// <https://support.google.com/google-ads/answer/2402582>
const B8_HEADER: &[&str] = &[
    "Action",
    "Customer ID",
    "Campaign",
    "Link Text",
    "App ID / Package name",
    "App store",
    "Final URL",
];

fn b5_columns() -> Vec<String> {
    let mut cols: Vec<String> = [
        "Action",
        "Ad status",
        "Customer ID",
        "Campaign",
        "Ad group",
        "Ad type",
        "Label",
    ]
    .map(String::from)
    .into();
    cols.extend((1..=15).map(|i| format!("Headline {i}")));
    cols.extend((1..=4).map(|i| format!("Description {i}")));
    cols.extend(["Headline 1 position", "Path 1", "Path 2", "Final URL"].map(String::from));
    cols
}

fn campaign_row(
    c: &Campaign,
    input: &Input,
    id: &str,
    devices: &str,
    labels: &str,
    template: &str,
) -> Row {
    let mut row = Row::default()
        .set("Action", "Add")
        .set("Campaign status", "Paused")
        .set("Customer ID", id)
        .set("Campaign", &c.name)
        .set("Campaign type", c.kind.label())
        .set("Budget", money(c.daily_budget))
        .set("Budget type", "Daily")
        .set("Bid Strategy Type", c.bid_strategy.label())
        .set("Language", input.business.language_primary())
        .set(
            "Location",
            input.business.locations.first().map_or("", String::as_str),
        )
        .set("Devices", devices)
        .set("Tracking template", template)
        .set("Label", labels);
    if c.kind == CampaignKind::Search {
        row = row.set("Networks", "Google search");
    }
    row
}

/// Blank when the id is missing or is the `000-000-0000` placeholder. Never write that placeholder.
fn customer_cell(id: &str) -> &str {
    let id = id.trim();
    let digits: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.chars().all(|c| c == '0') {
        ""
    } else {
        id
    }
}

/// `google_ads.tracking_template` when set. Otherwise `{lpurl}?` plus `export.url_suffix`.
fn tracking_template(input: &Input, slug: &str) -> String {
    let explicit = input.google_ads.tracking_template.trim();
    let suffix = input.export.url_suffix.trim();
    let raw = if !explicit.is_empty() {
        explicit.to_string()
    } else if suffix.is_empty() || suffix.starts_with("{lpurl}") {
        suffix.to_string()
    } else {
        format!("{{lpurl}}?{suffix}")
    };
    raw.replace("{mads_campaign}", slug)
}

fn match_label(m: MatchType) -> &'static str {
    match m {
        MatchType::Phrase => "Phrase",
        MatchType::Exact => "Exact",
    }
}

fn negative_label(m: MatchType) -> &'static str {
    match m {
        MatchType::Phrase => "Negative Phrase",
        MatchType::Exact => "Negative Exact",
    }
}

/// A package name goes to Google Play. Digits only go to the Apple App Store.
fn store_of(app_id: &str) -> (&'static str, String) {
    if app_id.chars().all(|c| c.is_ascii_digit()) {
        (
            "Apple App Store",
            format!("https://apps.apple.com/app/id{app_id}"),
        )
    } else {
        (
            "Google Play",
            format!("https://play.google.com/store/apps/details?id={app_id}"),
        )
    }
}

/// App asset link text is 25 characters. A long business name keeps the short form.
fn link_text(name: &str) -> String {
    let full = format!("Baixar o app {name}");
    if char_len(&full) <= 25 {
        full
    } else {
        "Baixar o app".into()
    }
}

fn readme(skip_b8: bool) -> String {
    let b8 = if skip_b8 {
        "8. `B8-extensao-app.csv` não foi gerado. Preencha `google_ads.app_id` e exporte de novo para criar a extensão de app."
    } else {
        "8. `B8-extensao-app.csv` cria a extensão de app nas campanhas de Search. O link aponta para a ficha da loja."
    };
    format!(
        r#"# Trilha B, estrutural

Estes arquivos ficam na pasta do run, no seu computador. O mads não envia nada ao Google Drive. Você cola cada um no Google Ads Editor, na mão.

Cole uma vez, na largada, nesta ordem. No Editor, Account, Import, From file. Revise a prévia e só depois poste.

1. `B1-campanhas.csv` cria as campanhas, todas pausadas.
2. `B2-status-campanha.csv` muda o status delas para Enabled. Cole este arquivo quando for ao ar. Se colar agora, as campanhas deixam de estar pausadas.
3. `B3-grupos.csv` cria os grupos de anúncio. Um grupo pode ter várias keywords.
4. `B4-keywords.csv` cria as keywords de cada grupo.
5. `B5-anuncios.csv` cria o anúncio responsivo de cada grupo.
6. `B6-utm.csv` grava o tracking template de cada grupo.
7. `B7-negativas.csv` grava as negativas da campanha em cada grupo, mais as do próprio grupo.
{b8}

As negativas de campanha são copiadas para cada grupo. Um grupo criado depois no Editor não herda essas negativas.

O B1 não traz a coluna `EU political ads`. Confira isso na prévia do Editor antes de postar.

O Editor casa campanha e grupo pelo nome. Estes arquivos não trazem Campaign ID nem Ad group ID.

Antes de ir ao ar, confira.

- Customer ID. Se a coluna veio vazia, preencha com o id real da conta. O mads não escreve o placeholder 000-000-0000.
- Max CPC. As campanhas usam Manual CPC. Sem lance o Editor recusa o grupo.
- Orçamento de cada campanha.
- Final URL das keywords e dos anúncios.
- App id e URL da loja, quando for usar o B8.
"#
    )
}
