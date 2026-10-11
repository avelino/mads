use super::super::model::StructAction;
use super::super::next_step;
use super::super::testutil::sample_package;
use super::*;
use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use std::io::Cursor;

type Book = Xlsx<Cursor<Vec<u8>>>;

const TABS: [&str; 12] = [
    "Settings",
    "Catalog",
    "Campaigns",
    "Fixed Keywords",
    "Ad Templates",
    "Negatives",
    "Feed",
    "Account Status",
    "Auto Negatives",
    "Updated",
    "Image Campaigns",
    "Image Assets",
];

fn open_package(p: &Package) -> Book {
    let bytes = feed_workbook(p).expect("written");
    open_workbook_from_rs(Cursor::new(bytes)).expect("valid xlsx")
}

fn open() -> Book {
    open_package(&sample_package())
}

fn value(book: &mut Book, tab: &str, row: u32, col: u32) -> Data {
    let range = book.worksheet_range(tab).expect("tab");
    range.get_value((row, col)).cloned().unwrap_or(Data::Empty)
}

fn formula(book: &mut Book, tab: &str, row: u32, col: u32) -> String {
    let range = book.worksheet_formula(tab).expect("formulas");
    range.get_value((row, col)).cloned().unwrap_or_default()
}

fn text(s: &str) -> Data {
    Data::String(s.into())
}

#[test]
fn tabs_come_in_spec_order() {
    assert_eq!(open().sheet_names(), TABS);
}

#[test]
fn every_tab_has_its_header_on_row_4() {
    let mut book = open();
    let first = [
        ("Settings", "Setting"),
        ("Catalog", "item_id"),
        ("Campaigns", "campaign_key"),
        ("Fixed Keywords", "action"),
        ("Ad Templates", "campaign_key"),
        ("Negatives", "Level"),
        ("Feed", "action"),
        ("Account Status", "Campaign"),
        ("Auto Negatives", "Campaign"),
        ("Updated", "Updated at"),
        ("Image Campaigns", "campaign_key"),
        ("Image Assets", "campaign_key"),
    ];
    for (tab, header) in first {
        assert_eq!(value(&mut book, tab, 3, 0), text(header), "{tab}");
    }
    assert_eq!(value(&mut book, "Feed", 3, 37), text("next_step"));
    assert_eq!(value(&mut book, "Ad Templates", 3, 21), text("Path 2"));
    assert_eq!(value(&mut book, "Updated", 3, 1), text("Source"));
}

#[test]
fn feed_row_5_formulas() {
    let mut book = open();
    let mut at = |col| formula(&mut book, "Feed", 4, col);
    assert_eq!(
        at(0),
        "IF(AND(Catalog!$B5<>\"\",Catalog!$E5=\"Yes\"),\"Add\",\"-\")"
    );
    assert_eq!(
        at(7),
        "IFERROR(INDEX(Campaigns!$C$5:$C$1004,MATCH($B5,Campaigns!$A$5:$A$1004,0))&\"\",\"\")"
    );
    assert_eq!(at(8), "IF(Catalog!$B5=\"\",\"\",Catalog!$B5)");
    assert_eq!(
        at(15),
        "IF(Catalog!$H5<>\"\",Catalog!$H5,\
         IFERROR(INDEX('Ad Templates'!$B$5:$B$1004,MATCH($B5,'Ad Templates'!$A$5:$A$1004,0))&\"\",\"\"))"
    );
    assert_eq!(
        at(16),
        "IFERROR(INDEX('Ad Templates'!$C$5:$C$1004,MATCH($B5,'Ad Templates'!$A$5:$A$1004,0))&\"\",\"\")"
    );
    assert_eq!(
        at(36),
        "IFERROR(INDEX('Account Status'!$C$5:$C$1004,\
         MATCH($H5&\"|\"&$I5,'Account Status'!$D$5:$D$1004,0))&\"\",\"\")"
    );
    assert_eq!(at(37), next_step::formula(5).trim_start_matches('='));
    assert_eq!(at(10), "IF(Catalog!$B5=\"\",\"\",Settings!$B$9&\"\")");
    assert_eq!(at(11), "IF(Catalog!$B5=\"\",\"\",Settings!$B$8&\"\")");
    assert_eq!(at(35), "\"\"");
    assert_eq!(value(&mut book, "Feed", 4, 2), text("Yes"));
    assert_eq!(value(&mut book, "Feed", 4, 3), text("SKAG"));
}

#[test]
fn the_automated_block_spans_the_capacity() {
    let p = sample_package();
    let mut book = open();
    let last = (FIRST_ROW - 1 + p.capacity - 1) as u32;
    let row = last + 1;
    assert_eq!(
        formula(&mut book, "Feed", last, 0),
        format!("IF(AND(Catalog!$B{row}<>\"\",Catalog!$E{row}=\"Yes\"),\"Add\",\"-\")")
    );
}

#[test]
fn a_fixed_row_holds_literal_values() {
    let p = sample_package();
    let mut book = open();
    let r = (FIRST_ROW - 1 + p.capacity) as u32;
    let expect = [
        (0, "Add"),
        (1, "brand"),
        (2, "No"),
        (3, "Thematic"),
        (7, "Brand"),
        (8, "Brand terms"),
        (9, "vinellu"),
        (10, "Exact"),
        (11, "1.50"),
        (12, "https://vinellu.com"),
        (13, "app"),
        (15, "Vinellu"),
        (16, "Wine Reviews"),
        (30, "Real ratings from people who drank it."),
        (34, "mads"),
        (35, "Paused"),
    ];
    for (col, v) in expect {
        assert_eq!(value(&mut book, "Feed", r, col), text(v), "col {col}");
        assert_eq!(formula(&mut book, "Feed", r, col), "", "col {col}");
    }
    assert_eq!(value(&mut book, "Feed", r, 36), Data::Empty);
    assert_eq!(value(&mut book, "Feed", r, 37), Data::Empty);
    assert_eq!(value(&mut book, "Feed", r + 1, 9), text("vinellu app"));
}

#[test]
fn an_edit_fixed_row_has_no_status() {
    let mut p = sample_package();
    p.fixed[0].action = StructAction::Edit;
    let mut book = open_package(&p);
    let r = (FIRST_ROW - 1 + p.capacity) as u32;
    assert_eq!(value(&mut book, "Feed", r, 0), text("Edit"));
    assert_eq!(value(&mut book, "Feed", r, 35), Data::Empty);
    assert_eq!(value(&mut book, "Fixed Keywords", 4, 7), Data::Empty);
    assert_eq!(value(&mut book, "Fixed Keywords", 5, 7), text("Paused"));
}

#[test]
fn a_fixed_row_without_headline_1_falls_back_to_its_template() {
    let mut p = sample_package();
    p.fixed[0].headlines[0].clear();
    let mut book = open_package(&p);
    let r = FIRST_ROW + p.capacity;
    assert_eq!(
        formula(&mut book, "Feed", (r - 1) as u32, 15),
        format!(
            "IFERROR(INDEX('Ad Templates'!$B$5:$B$1004,\
             MATCH($B{r},'Ad Templates'!$A$5:$A$1004,0))&\"\",\"\")"
        )
    );
}

#[test]
fn a_quoted_catalog_name_stays_a_literal_and_is_never_inlined() {
    let p = sample_package();
    let mut book = open();
    assert_eq!(
        value(&mut book, "Catalog", 5, 1),
        text("Don \"Melchor\", 2019")
    );
    assert_eq!(value(&mut book, "Catalog", 5, 4), text("Yes"));
    let formulas = book.worksheet_formula("Feed").expect("formulas");
    let mut seen = 0;
    for row in 4..(4 + p.feed_rows() as u32) {
        for col in 0..38 {
            let f = formulas.get_value((row, col)).cloned().unwrap_or_default();
            assert!(!f.contains("Melchor"), "({row},{col}) inlines the name");
            seen += usize::from(!f.is_empty());
        }
    }
    assert!(seen > 0);
}

#[test]
fn settings_tab_holds_the_values() {
    let mut book = open();
    assert_eq!(value(&mut book, "Settings", 4, 0), text("Customer ID"));
    assert_eq!(value(&mut book, "Settings", 4, 1), Data::Empty);
    assert_eq!(
        value(&mut book, "Settings", 5, 1),
        text("https://vinellu.com")
    );
    assert_eq!(value(&mut book, "Settings", 7, 1), text("1.00"));
    assert_eq!(value(&mut book, "Settings", 8, 1), text("Phrase"));
    assert_eq!(value(&mut book, "Settings", 10, 0), text("App store"));
}

#[test]
fn pending_cells_follow_the_settings() {
    let p = sample_package();
    assert_eq!(
        pending(&p),
        vec![
            (settings::CUSTOMER_ID, settings::VALUE),
            (settings::APP_ID, settings::VALUE)
        ]
    );
    let mut filled = sample_package();
    filled.settings.customer_id = "123-456-7890".into();
    filled.automated_campaigns.clear();
    filled.settings.default_max_cpc.clear();
    assert_eq!(
        pending(&filled),
        vec![(settings::DEFAULT_MAX_CPC, settings::VALUE)]
    );
    filled.automated_campaigns = vec!["Catalog - Wines".into()];
    filled.settings.app_id = "com.vinellu".into();
    filled.settings.default_max_cpc = "1.00".into();
    assert!(pending(&filled).is_empty());
}

#[test]
fn literal_tabs_hold_the_package() {
    let mut book = open();
    assert_eq!(value(&mut book, "Campaigns", 4, 2), text("Catalog - Wines"));
    assert_eq!(value(&mut book, "Campaigns", 4, 1), text("Yes"));
    assert_eq!(value(&mut book, "Campaigns", 5, 1), text("No"));
    assert_eq!(value(&mut book, "Campaigns", 4, 13), text("Add"));
    assert_eq!(
        value(&mut book, "Ad Templates", 4, 0),
        text("catalog-wines")
    );
    assert_eq!(value(&mut book, "Ad Templates", 4, 1), text("Wine Ratings"));
    assert_eq!(
        value(&mut book, "Ad Templates", 4, 16),
        text("See the rating before you open the bottle.")
    );
    assert_eq!(value(&mut book, "Ad Templates", 4, 20), text("wines"));
    assert_eq!(value(&mut book, "Negatives", 4, 3), text("job"));
    assert_eq!(
        value(&mut book, "Fixed Keywords", 4, 2),
        text("Brand terms")
    );
}

#[test]
fn account_status_has_a_key_formula_on_every_row() {
    let mut book = open();
    assert_eq!(
        formula(&mut book, "Account Status", 4, 3),
        "IF($A5=\"\",\"\",$A5&\"|\"&$B5)"
    );
    assert_eq!(
        formula(&mut book, "Account Status", 1003, 3),
        "IF($A1004=\"\",\"\",$A1004&\"|\"&$B1004)"
    );
    assert_eq!(formula(&mut book, "Account Status", 1004, 3), "");
}

#[test]
fn a_catalog_larger_than_the_capacity_is_an_error() {
    let mut p = sample_package();
    p.capacity = 1;
    assert!(matches!(feed_workbook(&p), Err(ExportError::Capacity(_))));
}

#[test]
fn image_campaigns_tab_holds_one_literal_row_per_campaign() {
    let mut book = open();
    assert_eq!(value(&mut book, "Image Campaigns", 3, 12), text("action"));
    let expect = [
        (0, "pmax-wines"),
        (1, "PMax - Wines"),
        (2, "Performance Max"),
        (3, "40.00"),
        (4, "maximize_conversions"),
        (6, "pt"),
        (7, "Brazil"),
        (8, "https://vinellu.com"),
        (9, "Vinellu"),
        (12, "Add"),
    ];
    for (col, v) in expect {
        assert_eq!(
            value(&mut book, "Image Campaigns", 4, col),
            text(v),
            "col {col}"
        );
        assert_eq!(
            formula(&mut book, "Image Campaigns", 4, col),
            "",
            "col {col}"
        );
    }
    for col in [5, 10, 11] {
        assert_eq!(value(&mut book, "Image Campaigns", 4, col), Data::Empty);
    }
    assert_eq!(value(&mut book, "Image Campaigns", 5, 0), Data::Empty);
}

#[test]
fn a_dropped_image_campaign_reads_back_as_pause() {
    let mut p = sample_package();
    p.image_campaigns[0].action = StructAction::Pause;
    let mut book = open_package(&p);
    assert_eq!(value(&mut book, "Image Campaigns", 4, 12), text("Pause"));
}

#[test]
fn image_assets_tab_holds_one_row_per_asset() {
    let p = sample_package();
    let mut book = open();
    assert_eq!(value(&mut book, "Image Assets", 3, 4), text("Image file"));
    assert_eq!(value(&mut book, "Image Assets", 4, 0), text("pmax-wines"));
    assert_eq!(value(&mut book, "Image Assets", 4, 1), text("Wine lovers"));
    assert_eq!(value(&mut book, "Image Assets", 4, 2), text("HEADLINE"));
    assert_eq!(value(&mut book, "Image Assets", 4, 3), text("Wine Ratings"));
    assert_eq!(value(&mut book, "Image Assets", 4, 4), Data::Empty);
    let last = (FIRST_ROW - 1 + p.image_assets.len() - 1) as u32;
    assert_eq!(value(&mut book, "Image Assets", last, 2), text("LOGO"));
    assert_eq!(value(&mut book, "Image Assets", last, 3), Data::Empty);
    assert_eq!(value(&mut book, "Image Assets", last, 4), text("logo.png"));
    assert_eq!(
        value(&mut book, "Image Assets", last - 1, 4),
        text("pmax-wines/toast-landscape.jpg")
    );
    assert_eq!(value(&mut book, "Image Assets", last + 1, 0), Data::Empty);
}
