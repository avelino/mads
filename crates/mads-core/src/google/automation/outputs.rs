//! The daily (D1 to D7) and structural (S1 to S8) workbooks, every output cell a formula over a mirror of the feed.

use super::columns::{FeedColumn, auto_negatives, campaigns, negatives, settings};
use super::model::{Package, StructAction};
use super::next_step::NextStep;
use super::sheet::{Cell, FIRST_ROW, Sheet, col_letter, workbook};
use crate::google::ExportError;

/// `(path, output tab)` of every output file, in package order. S1 and S7 name their output tab
/// `New Campaigns` and `Negative Keywords` because `Campaigns` and `Negatives` are their mirror
/// tabs, and a workbook cannot hold two tabs with one name.
pub(crate) const OUTPUT_FILES: [(&str, &str); 15] = [
    ("Daily/D1. New Ad Groups.xlsx", "Ad Groups"),
    ("Daily/D2. Pause Ad Groups.xlsx", "Ad Groups"),
    ("Daily/D3. Enable Ad Groups.xlsx", "Ad Groups"),
    ("Daily/D4. New Keywords.xlsx", "Keywords"),
    ("Daily/D5. New Ads.xlsx", "Ads"),
    ("Daily/D6. Tracking Templates.xlsx", "Tracking Templates"),
    ("Daily/D7. Ad Group Negatives.xlsx", "Ad Group Negatives"),
    ("Structure/S1. New Campaigns.xlsx", "New Campaigns"),
    ("Structure/S2. Campaign Status.xlsx", "Campaign Status"),
    ("Structure/S3. Ad Groups.xlsx", "Ad Groups"),
    ("Structure/S4. Keywords.xlsx", "Keywords"),
    ("Structure/S5. Ads.xlsx", "Ads"),
    (
        "Structure/S6. Tracking Templates.xlsx",
        "Tracking Templates",
    ),
    ("Structure/S7. Negatives.xlsx", "Negative Keywords"),
    ("Structure/S8. App Extension.xlsx", "App Extension"),
];

/// Rows of the `Auto Negatives` tab, the cap `daily-sync.js` enforces.
const AUTO_NEGATIVE_ROWS: usize = 2000;

/// Placeholder `linkSheets()` in `daily-sync.js` replaces with the id of `0. Feed`.
const FEED_ID: &str = "FEED_ID";

/// `automated` of the automated and of the fixed Feed rows.
const AUTOMATED: &str = "Yes";
const FIXED: &str = "No";

/// A tab of `0. Feed` that an output workbook mirrors under the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Settings,
    Feed,
    Campaigns,
    Negatives,
    AutoNegatives,
}

impl Source {
    fn tab(self) -> &'static str {
        match self {
            Source::Settings => "Settings",
            Source::Feed => "Feed",
            Source::Campaigns => "Campaigns",
            Source::Negatives => "Negatives",
            Source::AutoNegatives => "Auto Negatives",
        }
    }

    fn header(self) -> Vec<String> {
        let owned = |h: &[&str]| h.iter().map(|s| s.to_string()).collect();
        match self {
            Source::Settings => owned(&settings::HEADER),
            Source::Feed => FeedColumn::all().iter().map(|c| c.header()).collect(),
            Source::Campaigns => owned(&campaigns::HEADER),
            Source::Negatives => owned(&negatives::HEADER),
            Source::AutoNegatives => owned(&auto_negatives::HEADER),
        }
    }

    /// Data rows of the source tab, which is also the row count of the output tab over it.
    fn rows(self, p: &Package) -> usize {
        match self {
            Source::Settings => settings::LABELS.len(),
            Source::Feed => p.feed_rows(),
            Source::Campaigns => p.campaigns.len(),
            Source::Negatives => p.negatives.len(),
            Source::AutoNegatives => AUTO_NEGATIVE_ROWS,
        }
    }

    /// The `IMPORTRANGE` range, at least one row so an empty tab still gives a valid range.
    fn range(self, p: &Package) -> String {
        let last_col = col_letter(self.header().len().saturating_sub(1));
        let last_row = FIRST_ROW + self.rows(p).max(1) - 1;
        format!("{}!A{FIRST_ROW}:{last_col}{last_row}", quoted(self.tab()))
    }

    fn mirror(self, p: &Package) -> Sheet {
        let mut sheet = Sheet::new(
            self.tab(),
            &format!("{} (mirror of 0. Feed)", self.tab()),
            "Do not edit. linkSheets() in daily-sync.js fills it from 0. Feed.",
            &[],
        );
        sheet.header = self.header();
        sheet.rows = vec![vec![Cell::Formula(format!(
            "=IMPORTRANGE(\"{FEED_ID}\",\"{}\")",
            self.range(p)
        ))]];
        sheet
    }
}

/// One output file. The discriminant is its index in `OUTPUT_FILES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
}

const KINDS: [Kind; 15] = [
    Kind::D1,
    Kind::D2,
    Kind::D3,
    Kind::D4,
    Kind::D5,
    Kind::D6,
    Kind::D7,
    Kind::S1,
    Kind::S2,
    Kind::S3,
    Kind::S4,
    Kind::S5,
    Kind::S6,
    Kind::S7,
    Kind::S8,
];

/// The output tab of one Excel row: the condition that blanks the row and every
/// `(header, value)` pair. Each cell becomes `=IF(<skip>,"",<value>)`.
struct Output {
    skip: String,
    cols: Vec<(String, String)>,
}

impl Kind {
    fn path(self) -> &'static str {
        OUTPUT_FILES[self as usize].0
    }

    fn tab(self) -> &'static str {
        OUTPUT_FILES[self as usize].1
    }

    fn source(self) -> Source {
        match self {
            Kind::D7 => Source::AutoNegatives,
            Kind::S1 | Kind::S2 | Kind::S8 => Source::Campaigns,
            Kind::S7 => Source::Negatives,
            _ => Source::Feed,
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Kind::D1 => "Ad groups of catalog items to create today.",
            Kind::D2 => "Live ad groups of items no longer active in Catalog.",
            Kind::D3 => "Paused ad groups of items active again in Catalog.",
            Kind::D4 => "Keywords of the ad groups created today.",
            Kind::D5 => "Responsive search ads of the ad groups created today.",
            Kind::D6 => "Tracking template of the ad groups created today.",
            Kind::D7 => "Negatives the script found between automated ad groups.",
            Kind::S1 => "Every Search campaign of the plan. New campaigns start paused.",
            Kind::S2 => "Enables the new campaigns once their ads are reviewed.",
            Kind::S3 => "Ad groups of the non-automated campaigns.",
            Kind::S4 => "Keywords of the non-automated campaigns.",
            Kind::S5 => "Ads of the new non-automated ad groups.",
            Kind::S6 => "Tracking template of the new non-automated ad groups.",
            Kind::S7 => "Campaign and ad group negatives of the plan.",
            Kind::S8 => "App asset of every new campaign.",
        }
    }

    /// The title: the file name without folder and extension.
    fn title(self) -> &'static str {
        let path = self.path();
        let name = path.rsplit('/').next().unwrap_or(path);
        name.strip_suffix(".xlsx").unwrap_or(name)
    }

    fn output(self, r: usize) -> Output {
        let x = Refs { r };
        let add = StructAction::Add.label();
        let edit = StructAction::Edit.label();
        let automated_not = |step: NextStep| {
            format!(
                "{}<>{},{}<>{}",
                x.feed(FeedColumn::Automated),
                lit(AUTOMATED),
                x.feed(FeedColumn::NextStep),
                lit(step.label())
            )
        };
        let new_group = format!(
            "OR({},{})",
            automated_not(NextStep::Create),
            x.repeated_group()
        );
        let fixed = format!("{}<>{}", x.feed(FeedColumn::Automated), lit(FIXED));
        let feed_action = x.feed(FeedColumn::Action);
        let new_fixed_group = format!(
            "OR({fixed},{feed_action}<>{},{})",
            lit(add),
            x.repeated_group()
        );
        let campaign_key = x.cell(Source::Campaigns, campaigns::KEY);
        let campaign_action = x.cell(Source::Campaigns, campaigns::ACTION);
        match self {
            Kind::D1 => Output {
                skip: new_group,
                cols: x.ad_group_cols(lit(add), lit("Enabled")),
            },
            Kind::D2 => Output {
                skip: format!("OR({})", automated_not(NextStep::Pause)),
                cols: x.status_cols("Paused", "Item no longer active in Catalog"),
            },
            Kind::D3 => Output {
                skip: format!("OR({})", automated_not(NextStep::Enable)),
                cols: x.status_cols("Enabled", "Item active again in Catalog"),
            },
            Kind::D4 => Output {
                skip: format!("OR({})", automated_not(NextStep::Create)),
                cols: x.keyword_cols(lit(add), lit("Enabled")),
            },
            Kind::D5 => Output {
                skip: new_group,
                cols: x.ad_cols("Enabled"),
            },
            Kind::D6 => Output {
                skip: new_group,
                cols: x.tracking_cols(),
            },
            Kind::D7 => Output {
                skip: format!(
                    "{}=\"\"",
                    x.cell(Source::AutoNegatives, auto_negatives::KEYWORD)
                ),
                cols: x.negative_cols(
                    Source::AutoNegatives,
                    [
                        auto_negatives::CAMPAIGN,
                        auto_negatives::AD_GROUP,
                        auto_negatives::KEYWORD,
                        auto_negatives::MATCH_TYPE,
                        auto_negatives::REASON,
                    ],
                    None,
                ),
            },
            Kind::S1 => Output {
                skip: format!("{campaign_key}=\"\""),
                cols: x.campaign_cols(&campaign_action),
            },
            Kind::S2 => Output {
                skip: format!("OR({campaign_key}=\"\",{campaign_action}<>{})", lit(add)),
                cols: vec![
                    col("Action", lit(edit)),
                    col("Customer ID", x.customer_id()),
                    col(
                        "Campaign",
                        val(&x.cell(Source::Campaigns, campaigns::CAMPAIGN)),
                    ),
                    col("Campaign status", lit("Enabled")),
                ],
            },
            Kind::S3 => Output {
                skip: format!("OR({fixed},{})", x.repeated_group()),
                cols: x.ad_group_cols(struct_action(&feed_action), x.fixed_group_status()),
            },
            Kind::S4 => Output {
                skip: format!("OR({fixed},{}=\"\")", x.feed(FeedColumn::Keyword)),
                cols: x.keyword_cols(
                    struct_action(&feed_action),
                    paused_unless_edit(&feed_action),
                ),
            },
            Kind::S5 => Output {
                skip: new_fixed_group.clone(),
                cols: x.ad_cols("Paused"),
            },
            Kind::S6 => Output {
                skip: new_fixed_group,
                cols: x.tracking_cols(),
            },
            Kind::S7 => Output {
                skip: format!("{}=\"\"", x.cell(Source::Negatives, negatives::KEYWORD)),
                cols: x.negative_cols(
                    Source::Negatives,
                    [
                        negatives::CAMPAIGN,
                        negatives::AD_GROUP,
                        negatives::KEYWORD,
                        negatives::MATCH_TYPE,
                        negatives::REASON,
                    ],
                    Some(negatives::LEVEL),
                ),
            },
            Kind::S8 => Output {
                skip: format!(
                    "OR({campaign_key}=\"\",{campaign_action}<>{},{}=\"\")",
                    lit(add),
                    x.setting(settings::APP_ID)
                ),
                cols: vec![
                    col("Action", lit(add)),
                    col("Customer ID", x.customer_id()),
                    col(
                        "Campaign",
                        val(&x.cell(Source::Campaigns, campaigns::CAMPAIGN)),
                    ),
                    col("Link text", lit("Get the app")),
                    col("App ID / Package name", val(&x.setting(settings::APP_ID))),
                    col("App store", val(&x.setting(settings::APP_STORE))),
                    col("Final URL", val(&x.setting(settings::DOMAIN))),
                ],
            },
        }
    }

    fn sheet(self, p: &Package) -> Sheet {
        let header: Vec<String> = self
            .output(FIRST_ROW)
            .cols
            .into_iter()
            .map(|(h, _)| h)
            .collect();
        let header: Vec<&str> = header.iter().map(String::as_str).collect();
        let mut sheet = Sheet::new(self.tab(), self.title(), self.subtitle(), &header);
        sheet.note = Cell::Text(
            "Every cell is a formula over the mirror tabs. Filter out blank rows before pasting into Google Ads Editor."
                .into(),
        );
        sheet.rows = (0..self.source().rows(p))
            .map(|i| {
                let out = self.output(FIRST_ROW + i);
                out.cols
                    .into_iter()
                    .map(|(_, v)| Cell::Formula(format!("=IF({},\"\",{v})", out.skip)))
                    .collect()
            })
            .collect();
        sheet
    }
}

/// Cell references of one output row.
struct Refs {
    r: usize,
}

impl Refs {
    fn feed(&self, c: FeedColumn) -> String {
        format!("Feed!${}{}", c.letter(), self.r)
    }

    fn cell(&self, source: Source, col: usize) -> String {
        format!("{}!${}{}", quoted(source.tab()), col_letter(col), self.r)
    }

    /// The value of a `Settings` row in the mirror.
    fn setting(&self, offset: usize) -> String {
        format!(
            "Settings!${}${}",
            col_letter(settings::VALUE),
            FIRST_ROW + offset
        )
    }

    fn customer_id(&self) -> String {
        val(&self.setting(settings::CUSTOMER_ID))
    }

    /// True on every row but the first of its campaign and ad group.
    fn repeated_group(&self) -> String {
        let r = self.r;
        let c = FeedColumn::Campaign.letter();
        let g = FeedColumn::AdGroup.letter();
        format!(
            "COUNTIFS(Feed!${c}${FIRST_ROW}:${c}{r},Feed!${c}{r},Feed!${g}${FIRST_ROW}:${g}{r},Feed!${g}{r})>1"
        )
    }

    fn feed_val(&self, c: FeedColumn) -> String {
        val(&self.feed(c))
    }

    /// S3: a pause row with a keyword pauses that keyword only, not its ad group.
    fn fixed_group_status(&self) -> String {
        let a = self.feed(FeedColumn::Action);
        format!(
            "IF({a}={},\"Enabled\",IF(AND({a}={},{}=\"\"),\"Paused\",\"\"))",
            lit(StructAction::Add.label()),
            lit(StructAction::Pause.label()),
            self.feed(FeedColumn::Keyword)
        )
    }

    fn ad_group_cols(&self, action: String, status: String) -> Vec<(String, String)> {
        vec![
            col("Action", action),
            col("Customer ID", self.customer_id()),
            col("Campaign", self.feed_val(FeedColumn::Campaign)),
            col("Ad group", self.feed_val(FeedColumn::AdGroup)),
            col("Label", self.feed_val(FeedColumn::Label)),
            col("Ad group status", status),
            col("Max CPC", self.feed_val(FeedColumn::MaxCpc)),
        ]
    }

    fn status_cols(&self, status: &str, reason: &str) -> Vec<(String, String)> {
        vec![
            col("Action", lit(StructAction::Edit.label())),
            col("Customer ID", self.customer_id()),
            col("Campaign", self.feed_val(FeedColumn::Campaign)),
            col("Ad group", self.feed_val(FeedColumn::AdGroup)),
            col("Ad group status", lit(status)),
            col("Reason", lit(reason)),
        ]
    }

    fn keyword_cols(&self, action: String, status: String) -> Vec<(String, String)> {
        vec![
            col("Action", action),
            col("Customer ID", self.customer_id()),
            col("Campaign", self.feed_val(FeedColumn::Campaign)),
            col("Ad group", self.feed_val(FeedColumn::AdGroup)),
            col("Keyword", self.feed_val(FeedColumn::Keyword)),
            col("Match Type", self.feed_val(FeedColumn::MatchType)),
            col("Status", status),
            col("Max CPC", self.feed_val(FeedColumn::MaxCpc)),
            col("Final URL", self.feed_val(FeedColumn::FinalUrl)),
        ]
    }

    fn ad_cols(&self, status: &str) -> Vec<(String, String)> {
        let mut cols = vec![
            col("Action", lit(StructAction::Add.label())),
            col("Ad status", lit(status)),
            col("Customer ID", self.customer_id()),
            col("Campaign", self.feed_val(FeedColumn::Campaign)),
            col("Ad group", self.feed_val(FeedColumn::AdGroup)),
            col("Ad type", lit("Responsive search ad")),
            col("Label", self.feed_val(FeedColumn::Label)),
        ];
        for c in FeedColumn::all() {
            if matches!(c, FeedColumn::Headline(_) | FeedColumn::Description(_)) {
                cols.push((c.header(), self.feed_val(c)));
            }
        }
        cols.extend([
            col("Headline 1 position", "1".into()),
            col("Path 1", self.feed_val(FeedColumn::Path1)),
            col("Path 2", self.feed_val(FeedColumn::Path2)),
            col("Final URL", self.feed_val(FeedColumn::FinalUrl)),
        ]);
        cols
    }

    fn tracking_cols(&self) -> Vec<(String, String)> {
        vec![
            col("Action", lit(StructAction::Edit.label())),
            col("Customer ID", self.customer_id()),
            col("Campaign", self.feed_val(FeedColumn::Campaign)),
            col("Ad group", self.feed_val(FeedColumn::AdGroup)),
            col(
                "Tracking template",
                val(&self.setting(settings::TRACKING_TEMPLATE)),
            ),
        ]
    }

    fn campaign_cols(&self, action: &str) -> Vec<(String, String)> {
        let v = |c: usize| val(&self.cell(Source::Campaigns, c));
        vec![
            col("Action", struct_action(action)),
            col("Campaign status", paused_unless_edit(action)),
            col("Customer ID", self.customer_id()),
            col("Campaign", v(campaigns::CAMPAIGN)),
            col("Campaign type", v(campaigns::CAMPAIGN_TYPE)),
            col("Networks", v(campaigns::NETWORKS)),
            col("Budget", v(campaigns::BUDGET)),
            col("Budget type", v(campaigns::BUDGET_TYPE)),
            col("Bid Strategy Type", v(campaigns::BID_STRATEGY)),
            col("Language", v(campaigns::LANGUAGE)),
            col("Location", v(campaigns::LOCATION)),
            col("Devices", v(campaigns::DEVICES)),
            col("Tracking template", v(campaigns::TRACKING_TEMPLATE)),
            col("Label", v(campaigns::LABEL)),
        ]
    }

    /// `[campaign, ad group, keyword, match type, reason]` columns of `source`. With a level
    /// column, a `Campaign` level row leaves the ad group empty.
    fn negative_cols(
        &self,
        source: Source,
        [campaign, ad_group, keyword, match_type, reason]: [usize; 5],
        level: Option<usize>,
    ) -> Vec<(String, String)> {
        let v = |c: usize| val(&self.cell(source, c));
        let group = match level {
            Some(l) => format!(
                "IF({}=\"Campaign\",\"\",{})",
                self.cell(source, l),
                v(ad_group)
            ),
            None => v(ad_group),
        };
        vec![
            col("Action", lit(StructAction::Add.label())),
            col("Customer ID", self.customer_id()),
            col("Campaign", v(campaign)),
            col("Ad group", group),
            col("Negative keyword", v(keyword)),
            col("Match Type", v(match_type)),
            col("Reason", v(reason)),
        ]
    }
}

fn col(header: &str, value: String) -> (String, String) {
    (header.to_string(), value)
}

/// `Pause` is written as `Edit` with a `Paused` status; `Add` and `Edit` pass through.
fn struct_action(x: &str) -> String {
    format!(
        "IF({x}={},{},{x})",
        lit(StructAction::Pause.label()),
        lit(StructAction::Edit.label())
    )
}

/// Status of a structural row: `Paused` for `Add` and `Pause`, empty (unchanged) for `Edit`.
fn paused_unless_edit(x: &str) -> String {
    format!(
        "IF({x}={},\"Paused\",IF({x}={},\"Paused\",\"\"))",
        lit(StructAction::Add.label()),
        lit(StructAction::Pause.label())
    )
}

/// A reference that stays blank when the source cell is blank.
fn val(e: &str) -> String {
    format!("IF({e}=\"\",\"\",{e})")
}

/// A formula string literal.
fn lit(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A tab name as a formula or range prefix: quoted when it has a space.
fn quoted(tab: &str) -> String {
    if tab.contains(' ') {
        format!("'{tab}'")
    } else {
        tab.to_string()
    }
}

/// The output files present in this package: S8 only with an app.
fn kinds(p: &Package) -> impl Iterator<Item = Kind> + '_ {
    KINDS.into_iter().filter(|k| *k != Kind::S8 || p.has_app)
}

/// Every output workbook as `(path under the package folder, bytes)`, in `OUTPUT_FILES` order.
pub(crate) fn output_files(p: &Package) -> Result<Vec<(String, Vec<u8>)>, ExportError> {
    kinds(p)
        .map(|k| {
            let sheets = [Source::Settings.mirror(p), k.source().mirror(p), k.sheet(p)];
            Ok((k.path().to_string(), workbook(&sheets)?))
        })
        .collect()
}

/// `(file path, mirror tab, source range)` of every mirror tab, for `linkSheets()`.
pub(crate) fn mirror_ranges(p: &Package) -> Vec<(&'static str, &'static str, String)> {
    kinds(p)
        .flat_map(|k| [Source::Settings, k.source()].map(|s| (k.path(), s.tab(), s.range(p))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::testutil::sample_package;
    use super::*;
    use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
    use std::io::Cursor;

    type Book = Xlsx<Cursor<Vec<u8>>>;

    fn files(has_app: bool) -> Vec<(String, Vec<u8>)> {
        let mut p = sample_package();
        p.has_app = has_app;
        output_files(&p).expect("written")
    }

    fn open_file(path: &str) -> Book {
        let bytes = files(true)
            .into_iter()
            .find(|(p, _)| p == path)
            .map(|(_, b)| b)
            .expect("file present");
        open_workbook_from_rs(Cursor::new(bytes)).expect("valid xlsx")
    }

    fn header(book: &mut Book, tab: &str) -> Vec<String> {
        let range = book.worksheet_range(tab).expect("tab");
        let width = range.end().map_or(0, |(_, c)| c + 1);
        (0..width)
            .map(|c| match range.get_value((3, c)) {
                Some(Data::String(s)) => s.clone(),
                _ => String::new(),
            })
            .collect()
    }

    /// Formula of `tab` at Excel row `row`, zero-based column `col`, without the leading `=`.
    fn formula(path: &str, tab: &str, row: u32, col: u32) -> String {
        let mut book = open_file(path);
        let f = book.worksheet_formula(tab).expect("formulas");
        f.get_value((row - 1, col)).cloned().unwrap_or_default()
    }

    fn expected_mirror(path: &str) -> &'static str {
        if path.contains("D7.") {
            "Auto Negatives"
        } else if path.contains("S7.") {
            "Negatives"
        } else if path.contains("S1.") || path.contains("S2.") || path.contains("S8.") {
            "Campaigns"
        } else {
            "Feed"
        }
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn file_list_without_app_has_no_s8() {
        let names: Vec<String> = files(false).into_iter().map(|(p, _)| p).collect();
        let expected: Vec<&str> = OUTPUT_FILES[..14].iter().map(|(p, _)| *p).collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn file_list_with_app_has_all_fifteen() {
        let names: Vec<String> = files(true).into_iter().map(|(p, _)| p).collect();
        let expected: Vec<&str> = OUTPUT_FILES.iter().map(|(p, _)| *p).collect();
        assert_eq!(names, expected);
        assert_eq!(names[14], "Structure/S8. App Extension.xlsx");
    }

    #[test]
    fn every_workbook_has_mirrors_first_then_the_output_tab() {
        for (path, tab) in OUTPUT_FILES {
            let book = open_file(path);
            assert_eq!(
                book.sheet_names(),
                ["Settings", expected_mirror(path), tab],
                "{path}"
            );
        }
    }

    #[test]
    fn output_headers_follow_the_spec() {
        let d1 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Label",
            "Ad group status",
            "Max CPC",
        ]);
        let d2 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Ad group status",
            "Reason",
        ]);
        let d4 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Keyword",
            "Match Type",
            "Status",
            "Max CPC",
            "Final URL",
        ]);
        let mut d5 = strings(&[
            "Action",
            "Ad status",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Ad type",
            "Label",
        ]);
        d5.extend((1..=15).map(|n| format!("Headline {n}")));
        d5.extend((1..=4).map(|n| format!("Description {n}")));
        d5.extend(strings(&[
            "Headline 1 position",
            "Path 1",
            "Path 2",
            "Final URL",
        ]));
        let d6 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Tracking template",
        ]);
        let d7 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Ad group",
            "Negative keyword",
            "Match Type",
            "Reason",
        ]);
        let s1 = strings(&[
            "Action",
            "Campaign status",
            "Customer ID",
            "Campaign",
            "Campaign type",
            "Networks",
            "Budget",
            "Budget type",
            "Bid Strategy Type",
            "Language",
            "Location",
            "Devices",
            "Tracking template",
            "Label",
        ]);
        let s2 = strings(&["Action", "Customer ID", "Campaign", "Campaign status"]);
        let s8 = strings(&[
            "Action",
            "Customer ID",
            "Campaign",
            "Link text",
            "App ID / Package name",
            "App store",
            "Final URL",
        ]);
        let expected: [Vec<String>; 15] = [
            d1.clone(),
            d2.clone(),
            d2,
            d4.clone(),
            d5.clone(),
            d6.clone(),
            d7.clone(),
            s1,
            s2,
            d1,
            d4,
            d5,
            d6,
            d7,
            s8,
        ];
        for ((path, tab), want) in OUTPUT_FILES.iter().zip(expected) {
            let mut book = open_file(path);
            assert_eq!(header(&mut book, tab), want, "{path}");
        }
    }

    #[test]
    fn d2_action_pauses_automated_rows_whose_step_is_pause() {
        assert_eq!(
            formula("Daily/D2. Pause Ad Groups.xlsx", "Ad Groups", 5, 0),
            "IF(OR(Feed!$C5<>\"Yes\",Feed!$AL5<>\"PAUSE\"),\"\",\"Edit\")"
        );
    }

    #[test]
    fn d1_keeps_only_the_first_row_of_each_ad_group() {
        let f = formula("Daily/D1. New Ad Groups.xlsx", "Ad Groups", 5, 0);
        assert!(f.contains("Feed!$AL5<>\"CREATE\""), "{f}");
        assert!(
            f.contains("COUNTIFS(Feed!$H$5:$H5,Feed!$H5,Feed!$I$5:$I5,Feed!$I5)>1"),
            "{f}"
        );
    }

    #[test]
    fn customer_id_reads_the_settings_mirror() {
        assert_eq!(
            formula("Daily/D1. New Ad Groups.xlsx", "Ad Groups", 5, 1),
            "IF(OR(Feed!$C5<>\"Yes\",Feed!$AL5<>\"CREATE\",\
             COUNTIFS(Feed!$H$5:$H5,Feed!$H5,Feed!$I$5:$I5,Feed!$I5)>1),\"\",\
             IF(Settings!$B$5=\"\",\"\",Settings!$B$5))"
        );
    }

    #[test]
    fn s1_maps_the_structural_action() {
        let path = "Structure/S1. New Campaigns.xlsx";
        assert_eq!(
            formula(path, "New Campaigns", 5, 0),
            "IF(Campaigns!$A5=\"\",\"\",IF(Campaigns!$N5=\"Pause\",\"Edit\",Campaigns!$N5))"
        );
        assert_eq!(
            formula(path, "New Campaigns", 5, 1),
            "IF(Campaigns!$A5=\"\",\"\",\
             IF(Campaigns!$N5=\"Add\",\"Paused\",IF(Campaigns!$N5=\"Pause\",\"Paused\",\"\")))"
        );
    }

    #[test]
    fn s4_skips_rows_without_a_keyword() {
        let f = formula("Structure/S4. Keywords.xlsx", "Keywords", 5, 0);
        assert!(f.contains("Feed!$C5<>\"No\""), "{f}");
        assert!(f.contains("Feed!$J5=\"\""), "{f}");
    }

    #[test]
    fn s7_blanks_the_ad_group_of_a_campaign_negative() {
        let f = formula("Structure/S7. Negatives.xlsx", "Negative Keywords", 5, 3);
        assert!(f.contains("IF(Negatives!$A5=\"Campaign\",\"\""), "{f}");
    }

    #[test]
    fn d7_reason_comes_from_auto_negatives() {
        let f = formula(
            "Daily/D7. Ad Group Negatives.xlsx",
            "Ad Group Negatives",
            5,
            6,
        );
        assert!(f.contains("'Auto Negatives'!$E5"), "{f}");
    }

    #[test]
    fn output_rows_match_the_mirrored_rows() {
        let p = sample_package();
        let last_row = |path: &str, tab: &str| {
            let mut book = open_file(path);
            let f = book.worksheet_formula(tab).expect("formulas");
            f.end().map_or(0, |(r, _)| r as usize + 1)
        };
        assert_eq!(
            last_row("Daily/D1. New Ad Groups.xlsx", "Ad Groups"),
            4 + p.feed_rows()
        );
        assert_eq!(
            last_row("Structure/S1. New Campaigns.xlsx", "New Campaigns"),
            4 + p.campaigns.len()
        );
        assert_eq!(
            last_row("Structure/S7. Negatives.xlsx", "Negative Keywords"),
            4 + p.negatives.len()
        );
        assert_eq!(
            last_row("Daily/D7. Ad Group Negatives.xlsx", "Ad Group Negatives"),
            4 + 2000
        );
    }

    #[test]
    fn mirror_a5_imports_the_feed_range() {
        let p = sample_package();
        let mut book = open_file("Daily/D1. New Ad Groups.xlsx");
        let feed = book.worksheet_formula("Feed").expect("formulas");
        assert_eq!(
            feed.get_value((4, 0)).cloned().unwrap_or_default(),
            format!(
                "IMPORTRANGE(\"FEED_ID\",\"Feed!A5:AL{}\")",
                4 + p.feed_rows()
            )
        );
        let settings = book.worksheet_formula("Settings").expect("formulas");
        assert_eq!(
            settings.get_value((4, 0)).cloned().unwrap_or_default(),
            "IMPORTRANGE(\"FEED_ID\",\"Settings!A5:C11\")"
        );
        let range = book.worksheet_range("Feed").expect("tab");
        assert_eq!(
            range.get_value((3, 0)),
            Some(&Data::String("action".into()))
        );
    }

    #[test]
    fn mirror_ranges_cover_every_mirror_tab_of_every_file() {
        let mut p = sample_package();
        p.has_app = true;
        let ranges = mirror_ranges(&p);
        for (path, bytes) in output_files(&p).expect("written") {
            let mut book: Book = open_workbook_from_rs(Cursor::new(bytes)).expect("xlsx");
            let names = book.sheet_names();
            for mirror in &names[..names.len() - 1] {
                let found: Vec<&String> = ranges
                    .iter()
                    .filter(|(f, t, _)| *f == path && t == mirror)
                    .map(|(_, _, r)| r)
                    .collect();
                assert_eq!(found.len(), 1, "{path} {mirror}");
                let a5 = book.worksheet_formula(mirror).expect("formulas");
                assert_eq!(
                    a5.get_value((4, 0)).cloned().unwrap_or_default(),
                    format!("IMPORTRANGE(\"FEED_ID\",\"{}\")", found[0]),
                    "{path} {mirror}"
                );
            }
        }
        assert_eq!(ranges.len(), 30);
        assert!(ranges.contains(&(
            "Daily/D7. Ad Group Negatives.xlsx",
            "Auto Negatives",
            "'Auto Negatives'!A5:E2004".to_string()
        )));
        assert!(ranges.contains(&(
            "Structure/S1. New Campaigns.xlsx",
            "Campaigns",
            "Campaigns!A5:N6".to_string()
        )));
    }

    #[test]
    fn mirror_ranges_skip_s8_without_app() {
        let ranges = mirror_ranges(&sample_package());
        assert_eq!(ranges.len(), 28);
        assert!(ranges.iter().all(|(f, _, _)| !f.contains("S8.")));
    }
}
