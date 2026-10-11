//! The automation package through the public API: file paths, workbooks that open, tab names,
//! live structural actions, the folder name the script looks up and the picture copies.

use std::io::Cursor;

use calamine::{Reader, Xlsx, open_workbook_from_rs};
use mads_core::google::{
    Account, AdGroup, AspectRatio, AssetGroup, BidStrategy, BrandKit, Campaign, CampaignKind,
    CsvFile, ImageBrief, Intent, Keyword, MatchType, PlannedAdGroup, Rsa, automation_warnings,
    export_automation,
};
use mads_core::input::{
    Budget, Business, CatalogItem, ExportConfig, ExportStatus, GoogleAds, Input,
};
use mads_core::money::Cents;
use mads_core::perf::{CampaignPerf, Live, LiveStatus, Performance};

const DIR: &str = "automation/Vinellu Ads Automation";
const LONG: &str = "Bodega Catena Zapata Malbec Argentino 2019";

fn item(id: &str, name: &str) -> CatalogItem {
    CatalogItem {
        id: id.into(),
        name: name.into(),
        url: format!("https://vinellu.com/w/{id}"),
        category: "red".into(),
        aliases: vec![],
        third_party: false,
        notes: String::new(),
        image: None,
    }
}

fn input(google_ads: GoogleAds) -> Input {
    Input {
        business: Business {
            name: "Vinellu".into(),
            url: "https://vinellu.com".into(),
            language: "pt-BR".into(),
            locations: vec!["Brazil".into()],
            goal: "cadastros".into(),
            description: "App social de vinhos com reviews e safras.".into(),
            conversion_tracking: false,
            restricted: vec![],
            brand_terms: vec!["vinellu".into()],
            competitors: vec![],
            avoid: vec![],
            pages: vec![],
        },
        budget: Budget {
            daily: Cents(5000),
            currency: "BRL".into(),
            max_cpc: Some(Cents(300)),
        },
        export: ExportConfig {
            status: ExportStatus::Enabled,
            url_suffix: "utm_source=google&utm_medium=cpc".into(),
            eu_political_ads: false,
            decimal_comma: true,
        },
        research: String::new(),
        catalog: vec![
            item("alamos", "Alamos Malbec"),
            item("catena", LONG),
            item("velho", "Velho Tinto"),
        ],
        logo: None,
        formats: Vec::new(),
        design: String::new(),
        focus: None,
        app: None,
        google_ads,
    }
}

fn filled_ads() -> GoogleAds {
    GoogleAds {
        customer_id: "123-456-7890".into(),
        tracking_template: "{lpurl}?utm_campaign={mads_campaign}&utm_term={keyword}".into(),
        devices: "Mobile;Desktop;Tablet".into(),
        labels: vec!["estrutural".into()],
        app_id: String::new(),
    }
}

fn kw(text: &str, match_type: MatchType) -> Keyword {
    Keyword {
        text: text.into(),
        match_type,
    }
}

fn group(name: &str, keywords: Vec<Keyword>) -> AdGroup {
    AdGroup {
        name: name.into(),
        default_cpc: Cents(150),
        cpc_rationale: String::new(),
        final_url: "https://vinellu.com/w/alamos".into(),
        keywords,
        negatives: vec![kw("vinagre", MatchType::Phrase)],
        rsa: Rsa {
            headlines: vec!["Alamos Malbec".into()],
            descriptions: vec!["Nota e safras".into()],
            path1: Some("vinhos".into()),
            path2: None,
        },
    }
}

fn campaign(name: &str, intent: Intent, groups: Vec<AdGroup>) -> Campaign {
    Campaign {
        name: name.into(),
        slug: mads_core::input::slugify(name),
        kind: CampaignKind::Search,
        intent,
        daily_budget: Cents(5000),
        bid_strategy: BidStrategy::ManualCpc,
        rationale: String::new(),
        planned_ad_groups: vec![],
        ad_groups: groups,
        asset_groups: vec![],
        negatives: vec![kw("emprego", MatchType::Phrase)],
        assets: None,
    }
}

/// An automated catalog campaign listing two items and a fixed brand campaign.
fn account() -> Account {
    let mut rotulos = campaign(
        "Vinellu - Rotulos",
        Intent::Catalog,
        vec![group(
            "Alamos Malbec",
            vec![kw("alamos malbec", MatchType::Phrase)],
        )],
    );
    rotulos.planned_ad_groups = vec![PlannedAdGroup {
        name: "Alamos Malbec".into(),
        theme: String::new(),
        entity_ids: vec!["alamos".into(), "catena".into()],
        final_url: "https://vinellu.com/w/alamos".into(),
    }];
    let marca = campaign(
        "Vinellu - Marca",
        Intent::Generic,
        vec![group("marca", vec![kw("vinellu", MatchType::Exact)])],
    );
    Account {
        brand_kit: Some(BrandKit {
            headlines: vec!["Kit headline".into()],
            descriptions: vec!["Kit description".into()],
        }),
        campaigns: vec![rotulos, marca],
    }
}

/// The account the reports show: `Rotulos` serves `Alamos Malbec` and `Velho Tinto`, and the
/// new plan dropped `Velho Tinto`.
fn live() -> Live {
    let rotulos = campaign(
        "Vinellu - Rotulos",
        Intent::Catalog,
        vec![
            group(
                "Alamos Malbec",
                vec![kw("alamos malbec", MatchType::Phrase)],
            ),
            group("Velho Tinto", vec![kw("velho tinto", MatchType::Phrase)]),
        ],
    );
    let mut performance = Performance::default();
    performance.campaigns.push(CampaignPerf {
        name: "Vinellu - Rotulos".into(),
        live_status: Some(LiveStatus::Enabled),
        ..Default::default()
    });
    Live {
        baseline: Account {
            brand_kit: None,
            campaigns: vec![rotulos],
        },
        performance,
    }
}

fn names(files: &[CsvFile]) -> Vec<String> {
    files
        .iter()
        .map(|f| {
            f.name
                .strip_prefix(&format!("{DIR}/"))
                .unwrap_or(&f.name)
                .to_string()
        })
        .collect()
}

fn find<'a>(files: &'a [CsvFile], name: &str) -> &'a CsvFile {
    files
        .iter()
        .find(|f| f.name == format!("{DIR}/{name}"))
        .unwrap_or_else(|| panic!("missing {name}"))
}

type Book = Xlsx<Cursor<Vec<u8>>>;

fn open(f: &CsvFile) -> Book {
    open_workbook_from_rs(Cursor::new(f.bytes.clone()))
        .unwrap_or_else(|e| panic!("{} does not open: {e}", f.name))
}

/// Data rows (Excel row 5 onward) of `tab`, as text.
fn rows(book: &mut Book, tab: &str) -> Vec<Vec<String>> {
    let range = book.worksheet_range(tab).expect("tab");
    range
        .rows()
        .skip(4)
        .map(|r| r.iter().map(ToString::to_string).collect())
        .collect()
}

const SPREADSHEETS: [&str; 16] = [
    "0. Feed.xlsx",
    "README.xlsx",
    "Daily/D1. New Ad Groups.xlsx",
    "Daily/D2. Pause Ad Groups.xlsx",
    "Daily/D3. Enable Ad Groups.xlsx",
    "Daily/D4. New Keywords.xlsx",
    "Daily/D5. New Ads.xlsx",
    "Daily/D6. Tracking Templates.xlsx",
    "Daily/D7. Ad Group Negatives.xlsx",
    "Structure/S1. New Campaigns.xlsx",
    "Structure/S2. Campaign Status.xlsx",
    "Structure/S3. Ad Groups.xlsx",
    "Structure/S4. Keywords.xlsx",
    "Structure/S5. Ads.xlsx",
    "Structure/S6. Tracking Templates.xlsx",
    "Structure/S7. Negatives.xlsx",
];

#[test]
fn the_package_has_every_file_and_s8_only_with_an_app() {
    let files = export_automation(&input(filled_ads()), &account(), None)
        .expect("package")
        .files;
    let mut expected: Vec<String> = SPREADSHEETS.iter().map(|s| s.to_string()).collect();
    expected.push("daily-sync.js".into());
    let mut got = names(&files);
    got.sort();
    expected.sort();
    assert_eq!(got, expected);

    let ads = GoogleAds {
        app_id: "com.vinellu.app".into(),
        ..filled_ads()
    };
    let with_app = export_automation(&input(ads), &account(), None)
        .expect("package")
        .files;
    assert!(
        names(&with_app).contains(&"Structure/S8. App Extension.xlsx".to_string()),
        "{:?}",
        names(&with_app)
    );
    assert_eq!(with_app.len(), files.len() + 1);
}

#[test]
fn every_workbook_opens_with_the_expected_tabs() {
    let files = export_automation(&input(filled_ads()), &account(), None)
        .expect("package")
        .files;
    let feed = open(find(&files, "0. Feed.xlsx")).sheet_names();
    assert_eq!(
        feed,
        [
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
            "Image Assets"
        ]
    );
    let readme = open(find(&files, "README.xlsx")).sheet_names();
    assert_eq!(
        readme,
        [
            "Routine",
            "How actions are decided",
            "Files",
            "Linking",
            "Pending"
        ]
    );
    let tabs = |name: &str| open(find(&files, name)).sheet_names();
    assert_eq!(
        tabs("Daily/D1. New Ad Groups.xlsx"),
        ["Settings", "Feed", "Ad Groups"]
    );
    assert_eq!(
        tabs("Daily/D7. Ad Group Negatives.xlsx"),
        ["Settings", "Auto Negatives", "Ad Group Negatives"]
    );
    assert_eq!(
        tabs("Structure/S1. New Campaigns.xlsx"),
        ["Settings", "Campaigns", "New Campaigns"]
    );
    assert_eq!(
        tabs("Structure/S7. Negatives.xlsx"),
        ["Settings", "Negatives", "Negative Keywords"]
    );
    for name in &SPREADSHEETS[2..] {
        assert_eq!(tabs(name).len(), 3, "{name}");
    }
}

#[test]
fn the_catalog_lists_the_planned_items_and_the_campaigns_are_added() {
    let files = export_automation(&input(filled_ads()), &account(), None)
        .expect("package")
        .files;
    let mut feed = open(find(&files, "0. Feed.xlsx"));
    let catalog = rows(&mut feed, "Catalog");
    let items: Vec<&str> = catalog
        .iter()
        .filter(|r| !r[1].is_empty())
        .map(|r| r[1].as_str())
        .collect();
    assert_eq!(items, ["Alamos Malbec", LONG]);
    let campaigns = rows(&mut feed, "Campaigns");
    let actions: Vec<(&str, &str)> = campaigns
        .iter()
        .map(|r| (r[2].as_str(), r.last().map_or("", String::as_str)))
        .collect();
    assert_eq!(
        actions,
        [("Vinellu - Rotulos", "Add"), ("Vinellu - Marca", "Add")]
    );
}

#[test]
fn over_a_live_account_campaigns_are_edited_and_a_dropped_item_goes_inactive() {
    let files = export_automation(&input(filled_ads()), &account(), Some(&live()))
        .expect("package")
        .files;
    let mut feed = open(find(&files, "0. Feed.xlsx"));
    let campaigns = rows(&mut feed, "Campaigns");
    let rotulos = campaigns
        .iter()
        .find(|r| r[2] == "Vinellu - Rotulos")
        .expect("live campaign row");
    assert_eq!(rotulos.last().map(String::as_str), Some("Edit"));
    let marca = campaigns
        .iter()
        .find(|r| r[2] == "Vinellu - Marca")
        .expect("new campaign row");
    assert_eq!(marca.last().map(String::as_str), Some("Add"));

    let catalog = rows(&mut feed, "Catalog");
    let velho = catalog
        .iter()
        .find(|r| r[1] == "Velho Tinto")
        .expect("dropped item stays in Catalog");
    assert_eq!(velho[4], "No", "{velho:?}");
    let alamos = catalog
        .iter()
        .find(|r| r[1] == "Alamos Malbec")
        .expect("kept item");
    assert_eq!(alamos[4], "Yes");
}

#[test]
fn a_business_name_with_invalid_characters_gives_the_folder_the_script_looks_up() {
    let mut i = input(filled_ads());
    i.business.name = "Vin/ellu \"Wines\"".into();
    let files = export_automation(&i, &account(), None)
        .expect("package")
        .files;
    let dir = "automation/Vinellu Wines Ads Automation";
    assert!(
        files.iter().all(|f| f.name.starts_with(&format!("{dir}/"))),
        "{:?}",
        files.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    let js = files
        .iter()
        .find(|f| f.name == format!("{dir}/daily-sync.js"))
        .expect("script");
    let js = String::from_utf8(js.bytes.clone()).expect("utf-8");
    assert!(
        js.contains("var FOLDER_NAME = \"Vinellu Wines Ads Automation\";"),
        "{js}"
    );
}

#[test]
fn the_script_looks_files_up_without_the_xlsx_suffix() {
    let files = export_automation(&input(filled_ads()), &account(), None)
        .expect("package")
        .files;
    let js = String::from_utf8(find(&files, "daily-sync.js").bytes.clone()).expect("utf-8");
    let mirrors = js
        .lines()
        .find(|l| l.starts_with("var MIRRORS = "))
        .expect("MIRRORS line");
    assert!(mirrors.contains("\"Daily/D1. New Ad Groups\""), "{mirrors}");
    assert!(!mirrors.contains(".xlsx"), "{mirrors}");
}

#[test]
fn warnings_name_the_blank_customer_id_incomplete_image_campaigns_and_long_items() {
    let mut acc = account();
    let mut pmax = acc.campaigns[1].clone();
    pmax.name = "Vinellu - PMax".into();
    pmax.slug = "vinellu-pmax".into();
    pmax.kind = CampaignKind::PerformanceMax;
    pmax.ad_groups.clear();
    acc.campaigns.push(pmax);
    let ads = GoogleAds {
        customer_id: "000-000-0000".into(),
        ..filled_ads()
    };
    let warnings = automation_warnings(&input(ads), &acc);
    let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
    assert_eq!(codes, ["W11", "W12", "W13"], "{warnings:?}");
    assert_eq!(warnings[1].path, "campaigns[2]");
    assert!(
        warnings[1].message.starts_with(
            "Performance Max campaign 'Vinellu - PMax' is left out of the automation package: "
        ),
        "{warnings:?}"
    );
    assert!(warnings[1].message.contains("picture"), "{warnings:?}");
    assert!(warnings[1].message.contains("logo"), "{warnings:?}");
    assert!(warnings[1].message.ends_with('.'), "{warnings:?}");
    assert_eq!(warnings[2].path, "catalog");
    assert!(warnings[2].message.contains(LONG), "{warnings:?}");
    assert!(
        warnings[2].message.contains("Vinellu - Rotulos"),
        "{warnings:?}"
    );

    assert!(
        automation_warnings(&input(filled_ads()), &account())
            .iter()
            .all(|w| w.code == "W13")
    );
}

/// A Demand Gen campaign with one asset group whose two pictures exist, and a logo.
fn with_image_campaign() -> (Input, Account) {
    let mut i = input(filled_ads());
    i.logo = Some("/brand/logo.png".into());
    let mut acc = account();
    let brief = |id: &str, ratio| ImageBrief {
        id: id.into(),
        ratio,
        prompt: "Friends sharing red wine at dinner".into(),
        reference: None,
        file: Some(format!("images/vinellu-feed/tintos-{id}.jpg")),
    };
    acc.campaigns.push(Campaign {
        name: "Vinellu - Feed".into(),
        slug: "vinellu-feed".into(),
        kind: CampaignKind::DemandGen,
        intent: Intent::Generic,
        daily_budget: Cents(2000),
        bid_strategy: BidStrategy::MaximizeClicks { max_cpc: None },
        rationale: String::new(),
        planned_ad_groups: vec![],
        ad_groups: vec![],
        asset_groups: vec![AssetGroup {
            name: "tintos".into(),
            final_url: "https://vinellu.com/tintos".into(),
            business_name: "Vinellu".into(),
            headlines: vec!["Reviews reais".into()],
            long_headlines: vec!["Veja a nota antes de abrir a garrafa".into()],
            descriptions: vec!["Notas de quem bebeu".into()],
            search_themes: vec![],
            images: vec![
                brief("jantar", AspectRatio::Landscape),
                brief("garrafa", AspectRatio::Square),
            ],
        }],
        negatives: vec![],
        assets: None,
    });
    (i, acc)
}

#[test]
fn an_image_campaign_gets_its_feed_rows_and_its_picture_copies() {
    let (i, acc) = with_image_campaign();
    let out = export_automation(&i, &acc, None).expect("package");
    let mut feed = open(find(&out.files, "0. Feed.xlsx"));
    let campaigns = rows(&mut feed, "Image Campaigns");
    assert_eq!(campaigns.len(), 1, "{campaigns:?}");
    assert_eq!(campaigns[0][0], "vinellu-feed");
    assert_eq!(campaigns[0][1], "Vinellu - Feed");
    assert_eq!(campaigns[0][2], "Demand Gen");
    assert_eq!(campaigns[0][3], "20.00");
    assert_eq!(campaigns[0][12], "Add");

    let assets = rows(&mut feed, "Image Assets");
    let fields: Vec<(&str, &str)> = assets
        .iter()
        .map(|r| (r[2].as_str(), r[4].as_str()))
        .collect();
    assert!(fields.contains(&("LOGO", "logo.png")), "{fields:?}");
    assert!(
        fields.contains(&("MARKETING_IMAGE", "vinellu-feed/tintos-jantar.jpg")),
        "{fields:?}"
    );
    assert!(
        fields.contains(&("SQUARE_MARKETING_IMAGE", "vinellu-feed/tintos-garrafa.jpg")),
        "{fields:?}"
    );

    let mut pictures = out.pictures.clone();
    pictures.sort();
    let to = |f: &str| format!("{DIR}/Images/{f}");
    assert_eq!(
        pictures,
        [
            ("editor/images/logo.png".to_string(), to("logo.png")),
            (
                "editor/images/vinellu-feed/tintos-garrafa.jpg".to_string(),
                to("vinellu-feed/tintos-garrafa.jpg")
            ),
            (
                "editor/images/vinellu-feed/tintos-jantar.jpg".to_string(),
                to("vinellu-feed/tintos-jantar.jpg")
            ),
        ]
    );
    assert!(
        automation_warnings(&i, &acc)
            .iter()
            .all(|w| w.code != "W12"),
        "a complete image campaign is in the package"
    );
}

#[test]
fn a_search_only_package_copies_no_picture() {
    let out = export_automation(&input(filled_ads()), &account(), None).expect("package");
    assert!(out.pictures.is_empty(), "{:?}", out.pictures);
    let mut feed = open(find(&out.files, "0. Feed.xlsx"));
    assert!(rows(&mut feed, "Image Campaigns").is_empty());
    assert!(rows(&mut feed, "Image Assets").is_empty());
}
