//! The Editor file against a real Google Ads Editor 2.13.3 export of the same account.
//! `fixtures/editor-vinellu.tsv` holds the rows of two campaigns of that export (the Search
//! campaign of `data/` and an App campaign), reduced to the columns mads writes.

use std::collections::BTreeMap;

use mads_core::google::{
    Account, AssetGroup, BidStrategy, Campaign, CampaignKind, Intent, export_editor, read_editor,
};
use mads_core::input::{App, AppStore, Budget, Business, ExportConfig, ExportStatus, Input, Page};
use mads_core::money::Cents;

type Row = BTreeMap<String, String>;

fn input() -> Input {
    Input {
        business: Business {
            name: "Vinellu".into(),
            url: "https://vinellu.com".into(),
            language: "pt-BR".into(),
            locations: vec!["Brasil".into()],
            goal: "cadastros".into(),
            description: "App social de vinhos com reviews e safras.".into(),
            conversion_tracking: true,
            restricted: vec![],
            brand_terms: vec!["vinellu".into()],
            competitors: vec![],
            avoid: vec![],
            pages: Vec::<Page>::new(),
        },
        budget: Budget { daily: Cents(64000), currency: "BRL".into(), max_cpc: None },
        export: ExportConfig {
            status: ExportStatus::Enabled,
            url_suffix: "utm_source=google&utm_medium=cpc&utm_campaign=rotulos&utm_content={adgroupid}&utm_term={keyword}".into(),
            eu_political_ads: false,
            decimal_comma: true,
        },
        research: String::new(),
        catalog: vec![],
        logo: None,
        formats: Vec::new(),
        design: String::new(),
        focus: None,
        app: Some(App { store: AppStore::GooglePlay, id: "com.vinellu.app".into() }),
        google_ads: Default::default(),
    }
}

fn app_campaign() -> Campaign {
    Campaign {
        name: "vinellu android".into(),
        slug: "vinellu-android".into(),
        kind: CampaignKind::AppInstalls,
        intent: Intent::Generic,
        daily_budget: Cents(39000),
        bid_strategy: BidStrategy::MaximizeConversions,
        rationale: String::new(),
        planned_ad_groups: vec![],
        ad_groups: vec![],
        asset_groups: vec![
            AssetGroup {
                name: "Ad group 2".into(),
                final_url: "https://play.google.com/store/apps/details?id=com.vinellu.app".into(),
                business_name: String::new(),
                headlines: vec![
                    "qual seu vinho preferido?".into(),
                    "sua adega virtual".into(),
                    "recomendação de vinhos".into(),
                ],
                long_headlines: vec![],
                descriptions: vec![
                    "Conheça o rotulo de vinho que está tomando".into(),
                    "Tenha controle da sua adega, na palma da sua mão".into(),
                ],
                search_themes: vec![],
                images: vec![],
            },
            install_group(),
        ],
        negatives: vec![],
        assets: None,
    }
}

fn install_group() -> AssetGroup {
    let t = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    AssetGroup {
        name: "install 1".into(),
        final_url: "https://play.google.com/store/apps/details?id=com.vinellu.app".into(),
        business_name: String::new(),
        headlines: t(&[
            "Escaneie o rótulo e descubra",
            "Anote todo vinho que provar",
            "81 mil rótulos na sua mão",
            "Vinho bom sem anúncio pago",
            "Sua adega, no seu bolso",
        ]),
        long_headlines: vec![],
        descriptions: t(&[
            "Escaneie o rótulo, veja avaliações e monte seu histórico de vinhos. De graça.",
            "Base curada com 81 mil rótulos e safras separadas. Sem achismo, sem vinho errado.",
            "Chega de esquecer o vinho que você gostou. Uma foto e ele fica registrado.",
            "Siga quem entende de vinho e veja o que essa gente está bebendo de verdade.",
            "Descoberta por gente real, não por loja empurrando o estoque parado.",
        ]),
        search_themes: vec![],
        images: vec![],
    }
}

fn account() -> Account {
    let mut a: Account =
        serde_json::from_str(include_str!("fixtures/vinellu.account.json")).expect("fixture");
    a.campaigns[0].daily_budget = Cents(25000);
    a.campaigns.push(app_campaign());
    a
}

fn expected() -> Vec<Row> {
    let text = include_str!("fixtures/editor-vinellu.tsv");
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(text.as_bytes());
    let header: Vec<String> = rdr
        .headers()
        .expect("header")
        .iter()
        .map(String::from)
        .collect();
    rdr.records()
        .map(|r| {
            let r = r.expect("row");
            header
                .iter()
                .zip(r.iter())
                .filter(|(_, v)| !v.is_empty())
                .map(|(h, v)| (h.clone(), v.to_string()))
                .collect()
        })
        .collect()
}

/// What makes a row one entity: its campaign, group, keyword, ad type or location.
fn key(r: &Row) -> String {
    let kind = if r.contains_key("Campaign Type") {
        "campaign"
    } else if r.contains_key("Location") {
        "location"
    } else if r.contains_key("Ad type") {
        "ad"
    } else if r.contains_key("Keyword") {
        "keyword"
    } else {
        "group"
    };
    let get = |c: &str| r.get(c).cloned().unwrap_or_default();
    format!(
        "{kind}|{}|{}|{}|{}",
        get("Campaign"),
        get("Ad Group"),
        get("Keyword"),
        get("Criterion Type")
    )
}

/// Values the real export has that mads does not decide: defaults Editor shows on every
/// campaign, whatever its type.
fn ignored(row: &Row, col: &str) -> bool {
    let search = row.get("Campaign").is_some_and(|c| c.ends_with("Search"));
    search && matches!(col, "App campaign store" | "Campaign optimization")
}

#[test]
fn search_and_app_rows_match_a_real_editor_export() {
    let file = export_editor(&input(), &account()).expect("a file");
    assert_eq!(file.name, "editor/account.csv");
    assert_eq!(&file.bytes[..2], &[0xFF, 0xFE], "UTF-16 LE with a BOM");
    let produced = read_editor(&file.bytes).expect("readable");
    let by_key: BTreeMap<String, &Row> = produced.iter().map(|r| (key(r), r)).collect();
    let want = expected();
    let want_keys: std::collections::BTreeSet<String> = want.iter().map(key).collect();
    // The live account dropped 19 keywords of `data/` after the import. Keywords only in mads
    // are that drift. Any other extra row, or any missing one, is a format difference.
    let extra: Vec<&String> = by_key
        .keys()
        .filter(|k| !want_keys.contains(*k) && !k.starts_with("keyword|"))
        .collect();
    let missing: Vec<&String> = want_keys
        .iter()
        .filter(|k| !by_key.contains_key(*k))
        .collect();
    assert!(
        extra.is_empty() && missing.is_empty(),
        "extra {extra:#?}\nmissing {missing:#?}"
    );
    assert_eq!(
        by_key.len(),
        produced.len(),
        "every produced row has its own key"
    );
    for w in &want {
        let k = key(w);
        let got = by_key.get(&k).unwrap_or_else(|| panic!("missing row {k}"));
        for (col, value) in w.iter().filter(|(c, _)| !ignored(w, c)) {
            assert_eq!(got.get(col), Some(value), "{k}: column {col}");
        }
    }
}
