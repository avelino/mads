//! Structural Google Ads Editor files in Vaz's folder layout.
//!
//! Local only. Nothing here uploads to Drive. Campaigns are created paused.
//! A second file turns them on. Editor matches by name, so id columns stay out.

use std::collections::BTreeMap;

use mads_core::google::{
    Account, AdGroup, BidStrategy, BrandKit, Campaign, CampaignKind, Intent, Keyword, MatchType,
    Rsa, drive_warnings, export_drive_layout, read_editor,
};
use mads_core::input::{Budget, Business, ExportConfig, ExportStatus, GoogleAds, Input};
use mads_core::money::Cents;

const DIR: &str = "drive/B - Estrutural";

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
        catalog: vec![],
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

fn account() -> Account {
    Account {
        brand_kit: Some(BrandKit {
            headlines: vec!["Kit headline".into()],
            descriptions: vec!["Kit description".into()],
        }),
        campaigns: vec![Campaign {
            name: "Vinellu - Rotulos".into(),
            slug: "vinellu-rotulos".into(),
            kind: CampaignKind::Search,
            intent: Intent::Catalog,
            daily_budget: Cents(5000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![AdGroup {
                name: "rotulo-alamos".into(),
                default_cpc: Cents(150),
                cpc_rationale: String::new(),
                final_url: "https://vinellu.com/w/alamos".into(),
                keywords: vec![
                    kw("alamos malbec", MatchType::Phrase),
                    kw("alamos", MatchType::Exact),
                ],
                negatives: vec![kw("vinagre", MatchType::Phrase)],
                rsa: Rsa {
                    headlines: vec!["Alamos Malbec".into()],
                    descriptions: vec!["Nota e safras".into()],
                    path1: Some("vinhos".into()),
                    path2: Some("alamos".into()),
                },
            }],
            asset_groups: vec![],
            negatives: vec![kw("emprego", MatchType::Phrase)],
            assets: None,
        }],
    }
}

fn file<'a>(files: &'a [mads_core::google::CsvFile], name: &str) -> &'a [u8] {
    files
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .bytes
        .as_slice()
}

fn rows(bytes: &[u8]) -> Vec<BTreeMap<String, String>> {
    read_editor(bytes).expect("editor csv")
}

fn cell(row: &BTreeMap<String, String>, key: &str) -> String {
    row.get(key).cloned().unwrap_or_default()
}

#[test]
fn structural_files_follow_the_paste_order_and_editor_encoding() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        [
            format!("{DIR}/B1-campanhas.csv"),
            format!("{DIR}/B2-status-campanha.csv"),
            format!("{DIR}/B3-grupos.csv"),
            format!("{DIR}/B4-keywords.csv"),
            format!("{DIR}/B5-anuncios.csv"),
            format!("{DIR}/B6-utm.csv"),
            format!("{DIR}/B7-negativas.csv"),
            format!("{DIR}/LEIA-ME.md"),
        ]
    );
    let b1 = file(&files, &format!("{DIR}/B1-campanhas.csv"));
    assert_eq!(&b1[..2], &[0xFF, 0xFE], "UTF-16 LE BOM");
    let text = decode(b1);
    assert!(text.contains('\t'), "{text}");
    assert!(!text.contains('\r'), "Editor files use LF, {text}");
    assert!(
        text.starts_with(
            "Action\tCampaign status\tCustomer ID\tCampaign\tCampaign type\tNetworks\tBudget\tBudget type\tBid Strategy Type\tCampaign start date\tLanguage\tLocation\tDevices\tTracking template\tLabel\n"
        ),
        "{text}"
    );
    assert!(!text.contains("000-000-0000"), "{text}");
}

#[test]
fn campaigns_are_created_paused_and_b2_enables_them_by_name() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let b1 = rows(file(&files, &format!("{DIR}/B1-campanhas.csv")));
    assert_eq!(b1.len(), 1);
    assert_eq!(cell(&b1[0], "Action"), "Add");
    assert_eq!(cell(&b1[0], "Campaign status"), "Paused");
    assert_eq!(cell(&b1[0], "Customer ID"), "123-456-7890");
    assert_eq!(cell(&b1[0], "Campaign"), "Vinellu - Rotulos");
    assert_eq!(cell(&b1[0], "Campaign type"), "Search");
    assert_eq!(cell(&b1[0], "Networks"), "Google search");
    assert_eq!(cell(&b1[0], "Budget"), "50.00");
    assert_eq!(cell(&b1[0], "Budget type"), "Daily");
    assert_eq!(cell(&b1[0], "Bid Strategy Type"), "Manual CPC");
    assert_eq!(cell(&b1[0], "Language"), "pt");
    assert_eq!(cell(&b1[0], "Location"), "Brazil");
    assert_eq!(cell(&b1[0], "Devices"), "Mobile;Desktop;Tablet");
    assert_eq!(
        cell(&b1[0], "Tracking template"),
        "{lpurl}?utm_campaign=vinellu-rotulos&utm_term={keyword}"
    );
    assert_eq!(cell(&b1[0], "Label"), "estrutural");
    assert!(
        !b1[0].contains_key("Campaign ID") && !b1[0].contains_key("Campaign start date"),
        "blank id and start date stay out of the row: {b1:?}"
    );

    let b2_bytes = file(&files, &format!("{DIR}/B2-status-campanha.csv"));
    let b2_text = decode(b2_bytes);
    assert!(
        b2_text.starts_with("Action\tCustomer ID\tCampaign\tCampaign status\n"),
        "{b2_text}"
    );
    let b2 = rows(b2_bytes);
    assert_eq!(cell(&b2[0], "Action"), "Edit");
    assert_eq!(cell(&b2[0], "Campaign"), "Vinellu - Rotulos");
    assert_eq!(cell(&b2[0], "Campaign status"), "Enabled");
    assert!(
        !decode(file(&files, &format!("{DIR}/B2-status-campanha.csv"))).contains("Campaign ID")
    );
}

#[test]
fn one_ad_group_keeps_every_keyword() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let groups_bytes = file(&files, &format!("{DIR}/B3-grupos.csv"));
    let groups_text = decode(groups_bytes);
    assert!(
        groups_text.starts_with(
            "Action\tCustomer ID\tCampaign\tAd group\tLabel\tAd group status\tMax CPC\n"
        ),
        "{groups_text}"
    );
    let groups = rows(groups_bytes);
    assert_eq!(groups.len(), 1, "do not split into one group per keyword");
    assert_eq!(cell(&groups[0], "Ad group"), "rotulo-alamos");
    assert_eq!(cell(&groups[0], "Ad group status"), "Enabled");
    assert_eq!(cell(&groups[0], "Max CPC"), "1.50");
    assert_eq!(cell(&groups[0], "Label"), "estrutural");

    let text = decode(file(&files, &format!("{DIR}/B4-keywords.csv")));
    assert!(
        text.starts_with(
            "Action\tCustomer ID\tCampaign\tAd group\tKeyword\tMatch Type\tStatus\tMax CPC\tFinal URL\n"
        ),
        "{text}"
    );
    let kws = rows(file(&files, &format!("{DIR}/B4-keywords.csv")));
    assert_eq!(kws.len(), 2);
    assert!(kws.iter().all(|r| cell(r, "Ad group") == "rotulo-alamos"));
    assert_eq!(cell(&kws[0], "Keyword"), "alamos malbec");
    assert_eq!(cell(&kws[0], "Match Type"), "Phrase");
    assert_eq!(cell(&kws[1], "Keyword"), "alamos");
    assert_eq!(cell(&kws[1], "Match Type"), "Exact");
    assert_eq!(cell(&kws[0], "Status"), "Paused");
    assert_eq!(cell(&kws[0], "Final URL"), "https://vinellu.com/w/alamos");
    assert!(
        !kws[0].contains_key("Max CPC"),
        "the bid lives on the ad group"
    );
}

#[test]
fn ads_carry_fifteen_headline_columns_and_the_merged_rsa() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let text = decode(file(&files, &format!("{DIR}/B5-anuncios.csv")));
    assert!(
        text.starts_with("Action\tAd status\tCustomer ID\tCampaign\tAd group\tAd type\tLabel\t")
    );
    assert!(text.contains("\tHeadline 15\tDescription 1\tDescription 2\tDescription 3\tDescription 4\tHeadline 1 position\tPath 1\tPath 2\tFinal URL\n"));
    let ads = rows(file(&files, &format!("{DIR}/B5-anuncios.csv")));
    assert_eq!(ads.len(), 1);
    assert_eq!(cell(&ads[0], "Ad status"), "Paused");
    assert_eq!(cell(&ads[0], "Ad type"), "Responsive search ad");
    assert_eq!(cell(&ads[0], "Headline 1"), "Alamos Malbec");
    assert_eq!(cell(&ads[0], "Headline 2"), "Kit headline");
    assert_eq!(cell(&ads[0], "Description 1"), "Nota e safras");
    assert_eq!(cell(&ads[0], "Description 2"), "Kit description");
    assert_eq!(cell(&ads[0], "Path 1"), "vinhos");
    assert_eq!(cell(&ads[0], "Path 2"), "alamos");
    assert_eq!(cell(&ads[0], "Final URL"), "https://vinellu.com/w/alamos");
    assert_eq!(cell(&ads[0], "Headline 1 position"), "1");
}

#[test]
fn utm_and_negatives_use_editor_columns_and_expand_campaign_negatives() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let utm = decode(file(&files, &format!("{DIR}/B6-utm.csv")));
    assert!(
        utm.starts_with("Action\tCustomer ID\tCampaign\tAd group\tTracking template\n"),
        "{utm}"
    );
    let utm_rows = rows(file(&files, &format!("{DIR}/B6-utm.csv")));
    assert_eq!(
        cell(&utm_rows[0], "Tracking template"),
        "{lpurl}?utm_campaign=vinellu-rotulos&utm_term={keyword}"
    );

    let neg = decode(file(&files, &format!("{DIR}/B7-negativas.csv")));
    assert!(
        neg.starts_with("Action\tCustomer ID\tCampaign\tAd group\tKeyword\tCriterion Type\n"),
        "{neg}"
    );
    let neg_rows = rows(file(&files, &format!("{DIR}/B7-negativas.csv")));
    assert_eq!(
        neg_rows
            .iter()
            .map(|r| (cell(r, "Keyword"), cell(r, "Criterion Type")))
            .collect::<Vec<_>>(),
        [
            ("emprego".into(), "Negative Phrase".into()),
            ("vinagre".into(), "Negative Phrase".into()),
        ]
    );
    assert!(
        neg_rows
            .iter()
            .all(|r| cell(r, "Ad group") == "rotulo-alamos")
    );
}

#[test]
fn a_missing_customer_id_stays_blank_and_b8_needs_an_app_id() {
    let files = export_drive_layout(&input(GoogleAds::default()), &account()).unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.name).collect();
    assert!(names.iter().all(|n| !n.contains("B8")), "{names:?}");
    let b1_bytes = file(&files, &format!("{DIR}/B1-campanhas.csv"));
    let b1 = rows(b1_bytes);
    assert!(!b1[0].contains_key("Customer ID"));
    let text = decode(b1_bytes);
    assert!(!text.contains("000-000-0000"));
    assert_eq!(
        cell(&b1[0], "Tracking template"),
        "{lpurl}?utm_source=google&utm_medium=cpc",
        "without google_ads.tracking_template the URL suffix becomes the template"
    );

    let mut ads = filled_ads();
    ads.app_id = "com.vinellu.app".into();
    let files = export_drive_layout(&input(ads), &account()).unwrap();
    let b8 = decode(file(&files, &format!("{DIR}/B8-extensao-app.csv")));
    assert!(
        b8.starts_with(
            "Action\tCustomer ID\tCampaign\tLink Text\tApp ID / Package name\tApp store\tFinal URL\n"
        ),
        "{b8}"
    );
    let app = rows(file(&files, &format!("{DIR}/B8-extensao-app.csv")));
    assert_eq!(cell(&app[0], "App ID / Package name"), "com.vinellu.app");
    assert_eq!(cell(&app[0], "App store"), "Google Play");
    assert_eq!(
        cell(&app[0], "Final URL"),
        "https://play.google.com/store/apps/details?id=com.vinellu.app"
    );
    assert_eq!(cell(&app[0], "Link Text"), "Baixar o app Vinellu");
    assert!(!app[0].contains_key("Ad group"), "campaign-level asset");
}

#[test]
fn readme_is_portuguese_and_lists_the_paste_order_and_the_pending_items() {
    let files = export_drive_layout(&input(filled_ads()), &account()).unwrap();
    let readme = String::from_utf8(file(&files, &format!("{DIR}/LEIA-ME.md")).to_vec()).unwrap();
    assert!(readme.starts_with("# "));
    for needle in [
        "B1-campanhas.csv",
        "B2-status-campanha.csv",
        "B3-grupos.csv",
        "B4-keywords.csv",
        "B5-anuncios.csv",
        "B6-utm.csv",
        "B7-negativas.csv",
        "B8-extensao-app.csv",
        "Customer ID",
        "Max CPC",
        "000-000-0000",
        "Google Ads Editor",
        "Um grupo criado depois no Editor não herda essas negativas.",
        "EU political ads",
    ] {
        assert!(
            readme.contains(needle),
            "LEIA-ME misses {needle}:\n{readme}"
        );
    }
    assert!(!readme.contains('\u{2014}'), "no em dash in the readme");
}

#[test]
fn only_search_campaigns_are_written_and_the_rest_are_named_in_w11() {
    let mut acc = account();
    let mut pmax = acc.campaigns[0].clone();
    pmax.name = "Vinellu - PMax".into();
    pmax.slug = "vinellu-pmax".into();
    pmax.kind = CampaignKind::PerformanceMax;
    pmax.ad_groups.clear();
    acc.campaigns.push(pmax);
    let files = export_drive_layout(&input(filled_ads()), &acc).unwrap();
    for name in ["B1-campanhas.csv", "B2-status-campanha.csv"] {
        let rows = rows(file(&files, &format!("{DIR}/{name}")));
        assert_eq!(rows.len(), 1, "{name}: {rows:?}");
        assert_eq!(cell(&rows[0], "Campaign"), "Vinellu - Rotulos");
    }
    let warnings = drive_warnings(&input(filled_ads()), &acc);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].code, "W12");
    assert_eq!(warnings[0].path, "campaigns[1]");
    assert!(
        warnings[0].message.contains("Vinellu - PMax"),
        "{warnings:?}"
    );
}

#[test]
fn w10_fires_whenever_the_customer_id_cell_comes_out_blank() {
    let codes = |customer_id: &str| -> Vec<String> {
        let ads = GoogleAds {
            customer_id: customer_id.into(),
            ..filled_ads()
        };
        drive_warnings(&input(ads), &account())
            .into_iter()
            .map(|w| w.code)
            .collect()
    };
    assert_eq!(codes(""), vec!["W11"]);
    assert_eq!(
        codes("000-000-0000"),
        vec!["W11"],
        "a placeholder from workspace.json"
    );
    assert!(codes("123-456-7890").is_empty());
}

fn decode(bytes: &[u8]) -> String {
    let body = bytes.strip_prefix(&[0xFF, 0xFE]).expect("BOM");
    let (pairs, _) = body.as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|b| u16::from_le_bytes(*b)).collect();
    String::from_utf16(&units).expect("utf-16")
}
