//! `README.xlsx`: the routine, how actions are decided, the files, linking and what is pending.

use super::model::{Package, StructAction};
use super::next_step::{NextStep, TABLE};
use super::sheet::{Cell, Sheet, workbook};
use crate::google::ExportError;

const FEED_SETTINGS: &str = "0. Feed, tab Settings";

/// The `README.xlsx` bytes of `p`.
pub(crate) fn readme_workbook(p: &Package) -> Result<Vec<u8>, ExportError> {
    workbook(&[routine(p), decided(), files(p), linking(p), pending(p)])
}

fn row(cells: &[&str]) -> Vec<Cell> {
    cells.iter().map(|c| Cell::Text((*c).to_string())).collect()
}

fn routine(p: &Package) -> Sheet {
    let mut s = Sheet::new(
        "Routine",
        &format!("{} Ads | two tracks", p.business),
        "The Daily files run every day. The Structure files are pasted once.",
        &["When", "What happens", "Who"],
    );
    s.rows.push(row(&[
        "Once, at setup",
        "Do the steps in the Linking tab. Then paste S1 to S8 into Google Ads Editor, in that order. They create the campaigns, the fixed ad groups, keywords, ads and negatives. All of it starts paused.",
        "You",
    ]));
    if p.live {
        s.rows.push(row(&[
            "Once, over a live account",
            "Paste the Structure files once. They edit the campaigns already in the account and pause what the new plan dropped.",
            "You",
        ]));
    }
    if !p.image_campaigns.is_empty() {
        s.rows.push(row(&[
            "Once, after linkSheets",
            "setupImageCampaigns creates the Performance Max, Demand Gen and App campaigns from the Image Campaigns and Image Assets tabs. They are created paused. Enable them in Google Ads after review. After that the script only updates their budget. It never edits their assets.",
            "Google Ads script",
        ]));
    }
    s.rows.push(row(&[
        "Every night",
        "daily-sync.js reads the account and rewrites two tabs of 0. Feed: Account Status (what the account has today) and Auto Negatives (the isolation between ad groups, rebuilt from the feed). The script only reads. It never changes the account.",
        "Google Ads script",
    ]));
    s.rows.push(row(&[
        "Every morning",
        "The Feed compares what it wants with what the account has and fills next_step. Open D1, D2 and D3 and paste into Editor only the files that have rows. If D1 has rows, paste D4, D5, D6 and D7 too.",
        "You, in 2 minutes",
    ]));
    s.rows.push(row(&[
        "When prices, landing pages or budget change",
        "Edit only the Settings or Campaigns tab of 0. Feed. Every other file follows.",
        "You",
    ]));
    s.rows.push(row(&[
        "An empty day",
        "If D1, D2 and D3 have no rows, there is nothing to do. That is normal on most days.",
        "",
    ]));
    s
}

/// The daily files a step lands in. Every daily `CREATE` lands in the same four files.
fn daily_files(step: Option<NextStep>) -> &'static str {
    match step {
        Some(NextStep::Create) => "D1, D4, D5, D6",
        Some(NextStep::Enable) => "D3",
        Some(NextStep::Pause) => "D2",
        None => "none, the account already matches",
    }
}

fn decided() -> Sheet {
    let mut s = Sheet::new(
        "How actions are decided",
        "The next_step column of 0. Feed",
        "It compares what the feed wants with what the account has and emits only the difference.",
        &[
            "Feed wants (action)",
            "Account has (current_status)",
            "next_step",
            "Lands in",
        ],
    );
    for (action, status, step) in TABLE {
        let status = if status.is_empty() {
            "ad group not in the account"
        } else {
            status
        };
        let label = step.map_or("(empty)", NextStep::label);
        s.rows
            .push(row(&[action, status, label, daily_files(step)]));
    }
    s
}

/// `(file, track, purpose)` of every file in the package, in folder order.
fn file_list(p: &Package) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut out = vec![
        (
            "0. Feed",
            "both",
            "The single source. Settings, Catalog, Campaigns, Fixed Keywords, Ad Templates, Negatives, Feed, Account Status, Auto Negatives, Image Campaigns and Image Assets.",
        ),
        ("README", "both", "This file."),
        (
            "daily-sync.js",
            "Daily",
            "The script that runs in the account. main writes Account Status and Auto Negatives: schedule it daily, at night. setupImageCampaigns creates the image campaigns once.",
        ),
        (
            "D1. New Ad Groups",
            "Daily",
            "Ad groups whose item is new on the site.",
        ),
        (
            "D2. Pause Ad Groups",
            "Daily",
            "Ad groups whose item left the site and are still enabled.",
        ),
        (
            "D3. Enable Ad Groups",
            "Daily",
            "Ad groups whose item came back and are paused.",
        ),
        (
            "D4. New Keywords",
            "Daily",
            "The keyword of each new ad group. One per group.",
        ),
        (
            "D5. New Ads",
            "Daily",
            "The responsive search ad of each new ad group.",
        ),
        (
            "D6. Tracking Templates",
            "Daily",
            "The tracking template of each new ad group.",
        ),
        (
            "D7. Ad Group Negatives",
            "Daily",
            "Reads Auto Negatives, which the script rebuilds every night. Paste it whenever D1 has rows. Editor ignores repeated negatives.",
        ),
        (
            "S1. New Campaigns",
            "Structure",
            "Every Search campaign, paused.",
        ),
        (
            "S2. Campaign Status",
            "Structure",
            "Enables the new campaigns when you go live.",
        ),
        (
            "S3. Ad Groups",
            "Structure",
            "The ad groups of the fixed campaigns.",
        ),
        (
            "S4. Keywords",
            "Structure",
            "The keywords of the fixed ad groups.",
        ),
        (
            "S5. Ads",
            "Structure",
            "The ads of the fixed ad groups, paused.",
        ),
        (
            "S6. Tracking Templates",
            "Structure",
            "The tracking template of each fixed ad group.",
        ),
        (
            "S7. Negatives",
            "Structure",
            "The structural negatives at campaign and ad group level.",
        ),
    ];
    if !p.pictures.is_empty() {
        out.insert(
            3,
            (
                "Images/",
                "Structure",
                "The pictures and the logo of the image campaigns. setupImageCampaigns reads them from Drive by name. Do not rename them.",
            ),
        );
    }
    if p.has_app {
        out.push((
            "S8. App Extension",
            "Structure",
            "The app extension on the new campaigns.",
        ));
    }
    out
}

fn files(p: &Package) -> Sheet {
    let mut s = Sheet::new(
        "Files",
        "What each file does",
        "The Daily files read 0. Feed every day. The Structure files read it once, at setup.",
        &["File", "Track", "Purpose"],
    );
    s.rows = file_list(p)
        .into_iter()
        .map(|(file, track, purpose)| row(&[file, track, purpose]))
        .collect();
    s
}

fn linking(p: &Package) -> Sheet {
    let mut s = Sheet::new(
        "Linking",
        "Linking the files in Google Sheets",
        "Uploading alone links nothing: every mirror tab keeps FEED_ID until linkSheets runs. Do this once. Never edit the mirror tabs (Settings, Feed, Campaigns, Negatives, Auto Negatives) in the D and S files. They copy 0. Feed.",
        &["Step", "Where", "What to do"],
    );
    let mut steps: Vec<[&str; 2]> = vec![
        [
            "Google Drive",
            "Turn on Settings > Convert uploads to Google Sheets. Then upload the whole folder.",
        ],
        [
            "Google Ads > Tools > Scripts",
            "Create a script and paste daily-sync.js.",
        ],
        [
            "Google Ads > Tools > Scripts",
            "Run linkSheets once. It finds the folder by name and points every file at 0. Feed. The log lists each file it linked.",
        ],
    ];
    if !p.image_campaigns.is_empty() {
        steps.push([
            "Google Ads > Tools > Scripts",
            "Run setupImageCampaigns in Preview, read the log, then run it. The log has one line per campaign: created, budget updated, paused, unchanged or FAILED with the Google Ads error. A failed campaign leaves nothing behind.",
        ]);
    }
    steps.extend([
        [
            "Each D and S file",
            "Open it and click Allow access once. Google asks once per file, not once per formula.",
        ],
        [
            "Google Ads > Tools > Scripts",
            "Schedule main daily, at night. Without it the files work on yesterday's account.",
        ],
        [
            "Each D and S file",
            "Check cell A5 of each mirror tab. It must hold data, not FEED_ID, #REF! or Loading. If an agent uploaded the folder, it still has to run linkSheets and click Allow access, or this check will fail.",
        ],
    ]);
    s.rows = steps
        .iter()
        .enumerate()
        .map(|(i, [where_, what])| row(&[&(i + 1).to_string(), where_, what]))
        .collect();
    s
}

fn pending(p: &Package) -> Sheet {
    let mut s = Sheet::new(
        "Pending",
        "What is left before going live",
        "Every yellow cell in the folder has a row here.",
        &["Pending", "Where", "Why"],
    );
    if p.settings.customer_id.is_empty() {
        s.rows.push(row(&[
            "Customer ID",
            FEED_SETTINGS,
            "Editor matches every row to the account by this ID.",
        ]));
    }
    if !p.automated_campaigns.is_empty() && p.settings.app_id.is_empty() {
        s.rows.push(row(&[
            "App ID",
            FEED_SETTINGS,
            "The package name in the store. Leave it empty if there is no app.",
        ]));
    }
    if p.settings.default_max_cpc.is_empty() {
        s.rows.push(row(&[
            "Default Max CPC",
            FEED_SETTINGS,
            "The campaigns use Manual CPC. Without a bid Editor rejects the ad group.",
        ]));
    }
    for item in p.catalog.iter().filter(|c| c.headline_1.is_empty()) {
        s.rows.push(row(&[
            &format!("Headline 1 of {}", item.item),
            "0. Feed, tab Catalog, column headline_1",
            "The name is over 30 characters. The ad uses the campaign's fallback Headline 1 from Ad Templates until you write a shorter one.",
        ]));
    }
    if p.image_campaigns
        .iter()
        .any(|c| c.action == StructAction::Add)
    {
        s.rows.push(row(&[
            "Enable the image campaigns after review",
            "Google Ads > Campaigns",
            "setupImageCampaigns creates them paused. Check their assets and budget, then enable them.",
        ]));
    }
    s.rows.push(row(&[
        "Schedule main daily",
        "Google Ads > Tools > Scripts",
        "It must run every night. Otherwise the files work on yesterday's account and D7 goes stale.",
    ]));
    s.rows.push(row(&[
        "Allow access in each file",
        "Each D and S file",
        "Google asks once per file the first time it reads 0. Feed. No API can do it.",
    ]));
    s
}

#[cfg(test)]
mod tests {
    use super::super::next_step::TABLE;
    use super::super::testutil::sample_package;
    use super::*;
    use calamine::{Data, Range, Reader, Xlsx, open_workbook_from_rs};
    use std::io::Cursor;

    fn open(p: &Package) -> Xlsx<Cursor<Vec<u8>>> {
        let bytes = readme_workbook(p).expect("written");
        open_workbook_from_rs(Cursor::new(bytes)).expect("valid xlsx")
    }

    fn tab(p: &Package, name: &str) -> Range<Data> {
        open(p).worksheet_range(name).expect("tab")
    }

    /// Data rows (from Excel row 5) as text, one `Vec<String>` per row.
    fn data_rows(range: &Range<Data>) -> Vec<Vec<String>> {
        range
            .rows()
            .skip(4)
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect()
    }

    fn first_column(range: &Range<Data>) -> Vec<String> {
        data_rows(range)
            .into_iter()
            .map(|r| r.first().cloned().unwrap_or_default())
            .collect()
    }

    #[test]
    fn tab_names_in_order() {
        assert_eq!(
            open(&sample_package()).sheet_names(),
            [
                "Routine",
                "How actions are decided",
                "Files",
                "Linking",
                "Pending"
            ]
        );
    }

    #[test]
    fn decided_actions_render_every_table_entry() {
        let rows = data_rows(&tab(&sample_package(), "How actions are decided"));
        assert_eq!(rows.len(), TABLE.len());
        for (row, (action, _, step)) in rows.iter().zip(TABLE) {
            assert_eq!(row[0], action);
            let label = step.map_or("(empty)", |s| s.label());
            assert_eq!(row[2], label);
        }
        assert!(rows[0][3].contains("D1"), "{:?}", rows[0]);
    }

    #[test]
    fn files_list_every_file_and_s8_only_with_an_app() {
        let mut p = sample_package();
        let names = first_column(&tab(&p, "Files"));
        assert!(names.contains(&"0. Feed".to_string()));
        assert!(names.contains(&"daily-sync.js".to_string()));
        assert!(names.contains(&"D7. Ad Group Negatives".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("S8")));
        p.has_app = true;
        let names = first_column(&tab(&p, "Files"));
        assert!(names.contains(&"S8. App Extension".to_string()));
    }

    #[test]
    fn pending_lists_customer_id_when_empty() {
        let pending = first_column(&tab(&sample_package(), "Pending"));
        assert!(pending.contains(&"Customer ID".to_string()), "{pending:?}");
        assert!(pending.contains(&"App ID".to_string()), "{pending:?}");
        assert!(pending.contains(&"Schedule main daily".to_string()));
        assert!(pending.contains(&"Allow access in each file".to_string()));
    }

    #[test]
    fn pending_has_no_customer_id_row_when_set() {
        let mut p = sample_package();
        p.settings.customer_id = "123-456-7890".into();
        p.settings.app_id = "com.vinellu".into();
        let pending = first_column(&tab(&p, "Pending"));
        assert!(!pending.contains(&"Customer ID".to_string()), "{pending:?}");
        assert!(!pending.contains(&"App ID".to_string()), "{pending:?}");
        assert!(!pending.contains(&"Default Max CPC".to_string()));
    }

    #[test]
    fn pending_names_an_item_without_headline_1() {
        let mut p = sample_package();
        let long = "Chateau Something Very Long Reserve 2015";
        p.catalog[1].item = long.into();
        p.catalog[1].headline_1 = String::new();
        let rows = data_rows(&tab(&p, "Pending"));
        let hits: Vec<_> = rows.iter().filter(|r| r[0].contains(long)).collect();
        assert_eq!(hits.len(), 1, "{rows:?}");
        assert!(!rows.iter().any(|r| r[0].contains("Alamos")));
    }

    fn no_image_campaigns() -> Package {
        let mut p = sample_package();
        p.image_campaigns.clear();
        p.image_assets.clear();
        p.pictures.clear();
        p
    }

    fn has_cell(range: &Range<Data>, needle: &str) -> bool {
        data_rows(range)
            .iter()
            .any(|r| r.iter().any(|c| c.contains(needle)))
    }

    #[test]
    fn files_list_the_images_folder_and_the_image_tabs() {
        let p = sample_package();
        let files = tab(&p, "Files");
        assert!(first_column(&files).contains(&"Images/".to_string()));
        assert!(has_cell(&files, "Image Campaigns and Image Assets"));
        assert!(has_cell(&files, "setupImageCampaigns"));
        let names = first_column(&tab(&no_image_campaigns(), "Files"));
        assert!(!names.contains(&"Images/".to_string()), "{names:?}");
    }

    #[test]
    fn linking_runs_setup_image_campaigns_in_preview_after_link_sheets() {
        let rows = data_rows(&tab(&sample_package(), "Linking"));
        let link = rows.iter().position(|r| r[2].contains("Run linkSheets"));
        let setup = rows
            .iter()
            .position(|r| r[2].contains("setupImageCampaigns"));
        assert_eq!(setup, link.map(|i| i + 1), "{rows:?}");
        let step = &rows[setup.unwrap_or_default()];
        assert!(step[2].contains("Preview"), "{step:?}");
        assert!(step[2].contains("read the log"), "{step:?}");
        let numbers: Vec<String> = rows.iter().map(|r| r[0].clone()).collect();
        let expected: Vec<String> = (1..=rows.len()).map(|n| n.to_string()).collect();
        assert_eq!(numbers, expected);
        let without = tab(&no_image_campaigns(), "Linking");
        assert!(!has_cell(&without, "setupImageCampaigns"));
    }

    #[test]
    fn linking_warns_that_an_upload_alone_links_nothing_and_ends_with_a_check() {
        let linking = tab(&sample_package(), "Linking");
        let intro: Vec<String> = linking
            .rows()
            .take(3)
            .flat_map(|r| r.iter().map(|c| c.to_string()))
            .collect();
        assert!(
            intro
                .iter()
                .any(|c| c.contains("Uploading alone links nothing")),
            "{intro:?}"
        );
        let rows = data_rows(&linking);
        let last = rows.last().expect("steps");
        assert!(last[2].contains("FEED_ID"), "{last:?}");
        assert!(last[2].contains("#REF!"), "{last:?}");
        assert!(last[2].contains("A5"), "{last:?}");
    }

    #[test]
    fn linking_runs_setup_image_campaigns_for_a_pause_row_alone() {
        let mut p = sample_package();
        p.image_campaigns[0].action = StructAction::Pause;
        assert!(has_cell(&tab(&p, "Linking"), "setupImageCampaigns"));
    }

    #[test]
    fn routine_says_image_campaigns_start_paused_and_only_get_budget_updates() {
        let routine = tab(&sample_package(), "Routine");
        assert!(has_cell(&routine, "created paused"));
        assert!(has_cell(&routine, "only updates their budget"));
        assert!(!has_cell(
            &tab(&no_image_campaigns(), "Routine"),
            "created paused"
        ));
    }

    #[test]
    fn pending_asks_to_enable_image_campaigns_only_when_one_is_added() {
        let enable = "Enable the image campaigns after review";
        let pending = first_column(&tab(&sample_package(), "Pending"));
        assert!(pending.contains(&enable.to_string()), "{pending:?}");
        let mut p = sample_package();
        p.image_campaigns[0].action = StructAction::Edit;
        let pending = first_column(&tab(&p, "Pending"));
        assert!(!pending.contains(&enable.to_string()), "{pending:?}");
    }

    #[test]
    fn routine_mentions_live_structure_only_over_a_live_account() {
        let has_live_row = |p: &Package| {
            data_rows(&tab(p, "Routine")).iter().any(|r| {
                r.iter()
                    .any(|c| c.contains("pause what the new plan dropped"))
            })
        };
        let mut p = sample_package();
        assert!(!has_live_row(&p));
        p.live = true;
        assert!(has_live_row(&p));
    }
}
