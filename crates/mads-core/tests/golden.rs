use mads_core::google::{Account, export_csvs};
use mads_core::input::{Budget, Business, ExportConfig, ExportStatus, Input, Page};
use mads_core::money::Cents;

fn reference_input() -> Input {
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
            pages: Vec::<Page>::new(),
        },
        budget: Budget { daily: Cents(5000), currency: "BRL".into(), max_cpc: None },
        export: ExportConfig {
            status: ExportStatus::Paused,
            url_suffix: "utm_source=google&utm_medium=cpc&utm_campaign=rotulos&utm_content={adgroupid}&utm_term={keyword}".into(),
            eu_political_ads: false,
            decimal_comma: true,
        },
        research: String::new(),
        logo: None,
        formats: Vec::new(),
        design: String::new(),
        focus: None,
        app: None,
        catalog: vec![],
    }
}

fn reference_account() -> Account {
    let json = include_str!("fixtures/vinellu.account.json");
    serde_json::from_str(json).expect("fixture parses")
}

#[test]
fn exports_files_1_to_5_byte_identical_to_the_reference_csvs() {
    let files = export_csvs(&reference_input(), &reference_account()).expect("export");
    let names: Vec<_> = files.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        [
            "1-campaign.csv",
            "2-ad-groups.csv",
            "3-keywords.csv",
            "4-negative-keywords.csv",
            "5-responsive-search-ads.csv"
        ]
    );
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    for f in files {
        let expected = std::fs::read(data.join(f.name)).expect("reference file");
        assert_eq!(
            String::from_utf8_lossy(&f.bytes),
            String::from_utf8_lossy(&expected),
            "{} differs from data/",
            f.name
        );
        assert_eq!(f.bytes, expected, "{} differs byte for byte", f.name);
    }
}
