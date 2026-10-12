//! The automation package that `--layout drive-folders` writes: one feed workbook, the daily and
//! structural workbooks over it, a README, the Google Ads script and the pictures of the image
//! campaigns the script creates.
//! Specs: `docs/superpowers/specs/2026-10-10-search-automation-package-design.md` and
//! `docs/superpowers/specs/2026-10-10-image-campaigns-by-script-design.md`.

mod columns;
mod feed;
mod model;
mod next_step;
mod outputs;
mod readme;
mod script;
mod sheet;
#[cfg(test)]
mod testutil;

use serde::{Deserialize, Serialize};

use super::{Account, CsvFile, ExportError, Issue};
use crate::{input::Input, perf::Live};
use model::customer_cell;

pub use model::image_campaign_gaps;

/// Which file set `generate` and `export` write. `Bulk` is the default. Saved in `run.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportLayout {
    #[default]
    Bulk,
    DriveFolders,
}

/// Directory of the package under `google-ads/`.
const ROOT: &str = "automation";
/// Directory of the Editor files under `google-ads/`, where the pictures are generated.
const EDITOR: &str = "editor";
/// Folder of the pictures inside the package.
const IMAGES: &str = "Images";

/// What `export_automation` builds: the files to write and the pictures to copy.
#[derive(Debug, Clone, Default)]
pub struct AutomationExport {
    pub files: Vec<CsvFile>,
    /// `(from, to)`, both relative to `google-ads/`: an existing picture under `editor/` and its
    /// copy under `automation/<folder>/Images/`.
    pub pictures: Vec<(String, String)>,
}

/// The whole package, built in memory: `0. Feed.xlsx`, `README.xlsx`, `daily-sync.js`, then the
/// `Daily/` and `Structure/` workbooks, each under `automation/<folder>/`, plus the pictures the
/// script reads from `Images/`. One failure returns an error and no file.
///
/// With `live`, the structural rows edit that account: `Edit` for what it has, `Add` for what is
/// new, `Pause` for what the new plan dropped.
pub fn export_automation(
    input: &Input,
    account: &Account,
    live: Option<&Live>,
) -> Result<AutomationExport, ExportError> {
    let p = model::package(input, account, live)?;
    let dir = package_dir(&p);
    let file = |name: &str, bytes: Vec<u8>| CsvFile {
        name: format!("{dir}/{name}"),
        bytes,
    };
    let mut files = vec![
        file("0. Feed.xlsx", feed::feed_workbook(&p)?),
        file("README.xlsx", readme::readme_workbook(&p)?),
        file("daily-sync.js", script_text(&p).into_bytes()),
    ];
    for (path, bytes) in outputs::output_files(&p)? {
        files.push(file(&path, bytes));
    }
    Ok(AutomationExport {
        files,
        pictures: picture_paths(&p),
    })
}

/// `automation/<folder>`, relative to `google-ads/`.
fn package_dir(p: &model::Package) -> String {
    format!("{ROOT}/{}", p.folder)
}

/// The package's picture copies with both ends relative to `google-ads/`.
fn picture_paths(p: &model::Package) -> Vec<(String, String)> {
    let dir = package_dir(p);
    p.pictures
        .iter()
        .map(|c| {
            (
                format!("{EDITOR}/{}", c.from),
                format!("{dir}/{IMAGES}/{}", c.to),
            )
        })
        .collect()
}

/// `daily-sync.js` for `p`. A converted upload drops `.xlsx`, so the script looks files up by
/// their name without it.
fn script_text(p: &model::Package) -> String {
    let mirrors: Vec<(&str, &str, String)> = outputs::mirror_ranges(p)
        .into_iter()
        .map(|(file, tab, range)| (file.strip_suffix(".xlsx").unwrap_or(file), tab, range))
        .collect();
    script::daily_sync_js(p, &mirrors)
}

/// What the package leaves out or changes: a blank Customer ID (W11), every image campaign that
/// lacks what its type needs (W12) and every catalog item whose name is too long for headline 1
/// (W13).
pub fn automation_warnings(input: &Input, account: &Account) -> Vec<Issue> {
    let mut out = Vec::new();
    if customer_cell(&input.google_ads.customer_id).is_empty() {
        out.push(Issue::warning(
            "W11",
            "google_ads.customer_id",
            "Customer ID is empty or the 000-000-0000 placeholder, so the automation package leaves it blank. Set google_ads.customer_id to the real account id before import.",
        ));
    }
    for (i, reason) in model::image_campaign_gaps(input, account) {
        let Some(c) = account.campaigns.get(i) else {
            continue;
        };
        out.push(Issue::warning(
            "W12",
            format!("campaigns[{i}]"),
            format!(
                "{} campaign '{}' is left out of the automation package: {}.",
                c.kind.label(),
                c.name,
                reason.trim_end_matches('.')
            ),
        ));
    }
    out.extend(long_items(input, account));
    out
}

/// W13 per catalog row that lost its `headline_1`. A plan the package cannot build gets none:
/// the export reports that error itself.
fn long_items(input: &Input, account: &Account) -> Vec<Issue> {
    let Ok(p) = model::package(input, account, None) else {
        return Vec::new();
    };
    p.catalog
        .iter()
        .filter(|r| r.headline_1.is_empty() && !r.item.is_empty())
        .map(|r| {
            let campaign = p
                .campaigns
                .iter()
                .find(|c| c.key == r.campaign_key)
                .map_or(r.campaign_key.as_str(), |c| c.name.as_str());
            Issue::warning(
                "W13",
                "catalog",
                format!(
                    "Catalog item '{}' of campaign '{campaign}' is over 30 characters, so headline 1 of its ad falls back to the campaign's template headline. Fill headline_1 in the Catalog tab to change it.",
                    r.item
                ),
            )
        })
        .collect()
}

/// Characters Drive and the desktop file systems reject in a file or folder name.
const INVALID: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Name of the package folder: the business name without characters invalid in a file name,
/// then ` Ads Automation`. The script finds the folder by this exact name.
pub fn folder_name(business: &str) -> String {
    let kept: String = business
        .chars()
        .filter(|c| !INVALID.contains(c) && !c.is_control())
        .collect();
    let name = kept
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>();
    if name.is_empty() {
        return "Ads Automation".into();
    }
    format!("{} Ads Automation", name.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_name_appends_the_suffix() {
        assert_eq!(folder_name("Vinellu"), "Vinellu Ads Automation");
    }

    #[test]
    fn folder_name_drops_invalid_characters() {
        assert_eq!(
            folder_name("A/B\\C:D*E?F\"G<H>I|J"),
            "ABCDEFGHIJ Ads Automation"
        );
        assert_eq!(folder_name("Tab\there\u{7}"), "Tabhere Ads Automation");
    }

    #[test]
    fn folder_name_trims_and_collapses_spaces() {
        assert_eq!(
            folder_name("  Don : Melchor  Wines "),
            "Don Melchor Wines Ads Automation"
        );
    }

    #[test]
    fn pictures_go_from_editor_to_the_package_images_folder() {
        let p = super::testutil::sample_package();
        assert_eq!(
            picture_paths(&p),
            [
                (
                    "editor/images/pmax-wines/toast-landscape.jpg".to_string(),
                    "automation/Vinellu Ads Automation/Images/pmax-wines/toast-landscape.jpg"
                        .to_string()
                ),
                (
                    "editor/images/logo.png".to_string(),
                    "automation/Vinellu Ads Automation/Images/logo.png".to_string()
                ),
            ]
        );
    }

    #[test]
    fn folder_name_of_an_empty_name_is_the_suffix() {
        assert_eq!(folder_name(" / "), "Ads Automation");
    }
}
