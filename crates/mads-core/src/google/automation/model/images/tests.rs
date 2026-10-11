use super::super::tests::{account, campaign, input};
use super::super::{ImageAssetRow, Package, PictureCopy, StructAction, package};
use super::image_campaign_gaps;
use crate::google::{
    Account, AspectRatio, AssetGroup, BidStrategy, Campaign, CampaignKind, ImageBrief, Intent,
};
use crate::input::{App, AppStore, Input};
use crate::money::Cents;
use crate::perf::{CampaignPerf, Live, LiveStatus, Performance};

fn brief(id: &str, ratio: AspectRatio, file: Option<&str>) -> ImageBrief {
    ImageBrief {
        id: id.into(),
        ratio,
        prompt: String::new(),
        reference: None,
        file: file.map(Into::into),
    }
}

fn asset_group(name: &str, images: Vec<ImageBrief>) -> AssetGroup {
    AssetGroup {
        name: name.into(),
        final_url: "https://vinellu.com/pmax".into(),
        business_name: "Vinellu App".into(),
        headlines: vec!["Wine Ratings".into(), "Scan the Label".into()],
        long_headlines: vec!["See the rating before you open the bottle".into()],
        descriptions: vec!["Real ratings.".into()],
        search_themes: vec!["wine app".into()],
        images,
    }
}

fn image_campaign(name: &str, slug: &str, kind: CampaignKind) -> Campaign {
    let mut c = campaign(name, slug, kind, Intent::Generic);
    c.daily_budget = Cents(4000);
    c.bid_strategy = BidStrategy::MaximizeConversions;
    c.asset_groups = vec![asset_group(
        "Wine lovers",
        vec![brief(
            "toast",
            AspectRatio::Landscape,
            Some("images/pmax-wines/wine-lovers-toast.jpg"),
        )],
    )];
    c
}

fn with_logo() -> Input {
    let mut i = input();
    i.logo = Some("/abs/brand/Logo.PNG".into());
    i
}

fn account_with(campaigns: Vec<Campaign>) -> Account {
    let mut a = account();
    a.campaigns = campaigns;
    a
}

fn build(input: &Input, campaigns: Vec<Campaign>) -> Package {
    package(input, &account_with(campaigns), None).expect("package")
}

fn pmax() -> Campaign {
    image_campaign("PMax - Wines", "pmax-wines", CampaignKind::PerformanceMax)
}

fn fields(p: &Package) -> Vec<(&str, &str, &str)> {
    p.image_assets
        .iter()
        .map(|a| (a.field.as_str(), a.text.as_str(), a.image_file.as_str()))
        .collect()
}

#[test]
fn performance_max_row_carries_the_campaign_values() {
    let p = build(&with_logo(), vec![pmax()]);
    assert_eq!(p.image_campaigns.len(), 1);
    let r = &p.image_campaigns[0];
    assert_eq!(r.key, "pmax-wines");
    assert_eq!(r.name, "PMax - Wines");
    assert_eq!(r.campaign_type, "Performance Max");
    assert_eq!(r.budget, money_of(4000));
    assert_eq!(r.bid_strategy, "maximize_conversions");
    assert_eq!(r.max_cpc, "");
    assert_eq!(r.language, "pt");
    assert_eq!(r.location, "Brazil");
    assert_eq!(r.final_url, "https://vinellu.com/pmax");
    assert_eq!(r.business_name, "Vinellu App");
    assert_eq!(r.app_id, "");
    assert_eq!(r.app_store, "");
    assert_eq!(r.action, StructAction::Add);
    assert!(p.campaigns.iter().all(|c| c.key != "pmax-wines"));
}

fn money_of(cents: u64) -> String {
    crate::google::editor::money(Cents(cents))
}

#[test]
fn campaign_types_and_bid_strategies_use_the_script_vocabulary() {
    let mut dg = image_campaign("DG", "dg", CampaignKind::DemandGen);
    dg.bid_strategy = BidStrategy::MaximizeClicks {
        max_cpc: Some(Cents(120)),
    };
    let mut app = image_campaign("App", "app", CampaignKind::AppInstalls);
    app.bid_strategy = BidStrategy::ManualCpc;
    let mut uncapped = image_campaign("DG2", "dg2", CampaignKind::DemandGen);
    uncapped.bid_strategy = BidStrategy::MaximizeClicks { max_cpc: None };
    let p = build(&with_logo(), vec![dg, app, uncapped]);
    let got: Vec<(&str, &str, &str)> = p
        .image_campaigns
        .iter()
        .map(|r| {
            (
                r.campaign_type.as_str(),
                r.bid_strategy.as_str(),
                r.max_cpc.as_str(),
            )
        })
        .collect();
    let cap = money_of(120);
    assert_eq!(
        got,
        [
            ("Demand Gen", "maximize_clicks", cap.as_str()),
            ("App", "manual_cpc", ""),
            ("Demand Gen", "maximize_clicks", ""),
        ]
    );
}

#[test]
fn without_asset_group_values_the_business_url_and_name_stand_in() {
    let mut c = pmax();
    c.asset_groups[0].final_url = String::new();
    c.asset_groups[0].business_name = String::new();
    let mut i = with_logo();
    i.business.locations.clear();
    let p = build(&i, vec![c]);
    let r = &p.image_campaigns[0];
    assert_eq!(r.final_url, "https://vinellu.com");
    assert_eq!(r.business_name, i.business.name);
    assert_eq!(r.location, "");
}

#[test]
fn performance_max_assets_list_texts_themes_business_name_logo_and_pictures() {
    let p = build(&with_logo(), vec![pmax()]);
    assert!(
        p.image_assets
            .iter()
            .all(|a| a.campaign_key == "pmax-wines" && a.group == "Wine lovers")
    );
    assert_eq!(
        fields(&p),
        [
            ("HEADLINE", "Wine Ratings", ""),
            ("HEADLINE", "Scan the Label", ""),
            (
                "LONG_HEADLINE",
                "See the rating before you open the bottle",
                ""
            ),
            ("DESCRIPTION", "Real ratings.", ""),
            ("SEARCH_THEME", "wine app", ""),
            ("BUSINESS_NAME", "Vinellu App", ""),
            ("LOGO", "", "logo.png"),
            ("MARKETING_IMAGE", "", "pmax-wines/wine-lovers-toast.jpg"),
        ]
    );
}

#[test]
fn demand_gen_has_no_search_themes_and_app_has_no_logo_or_business_name() {
    let dg = image_campaign("DG", "dg", CampaignKind::DemandGen);
    let mut i = with_logo();
    i.app = Some(App {
        store: AppStore::GooglePlay,
        id: "com.vinellu.app".into(),
    });
    let app = image_campaign("App", "app", CampaignKind::AppInstalls);
    let p = build(&i, vec![dg, app]);
    let of = |key: &str| -> Vec<&str> {
        p.image_assets
            .iter()
            .filter(|a| a.campaign_key == key)
            .map(|a| a.field.as_str())
            .collect()
    };
    let dg_fields = of("dg");
    assert!(!dg_fields.contains(&"SEARCH_THEME"));
    assert!(dg_fields.contains(&"BUSINESS_NAME"));
    assert!(dg_fields.contains(&"LOGO"));
    let app_fields = of("app");
    assert_eq!(
        app_fields,
        [
            "HEADLINE",
            "HEADLINE",
            "LONG_HEADLINE",
            "DESCRIPTION",
            "MARKETING_IMAGE"
        ]
    );
}

#[test]
fn each_ratio_maps_to_its_asset_field_and_briefs_without_a_file_are_skipped() {
    let mut c = pmax();
    c.asset_groups[0].images = vec![
        brief("a", AspectRatio::Landscape, Some("images/pmax-wines/a.jpg")),
        brief("b", AspectRatio::Square, Some("images/pmax-wines/b.jpg")),
        brief("c", AspectRatio::Portrait, Some("images/pmax-wines/c.jpg")),
        brief("d", AspectRatio::Vertical, Some("images/pmax-wines/d.jpg")),
        brief("e", AspectRatio::Square, None),
    ];
    let p = build(&with_logo(), vec![c]);
    let pictures: Vec<(&str, &str)> = p
        .image_assets
        .iter()
        .filter(|a| a.field.contains("IMAGE"))
        .map(|a| (a.field.as_str(), a.image_file.as_str()))
        .collect();
    assert_eq!(
        pictures,
        [
            ("MARKETING_IMAGE", "pmax-wines/a.jpg"),
            ("SQUARE_MARKETING_IMAGE", "pmax-wines/b.jpg"),
            ("PORTRAIT_MARKETING_IMAGE", "pmax-wines/c.jpg"),
            ("TALL_PORTRAIT_MARKETING_IMAGE", "pmax-wines/d.jpg"),
        ]
    );
}

#[test]
fn pictures_copy_each_file_once_plus_the_logo() {
    let mut c = pmax();
    let shared = brief("x", AspectRatio::Square, Some("images/pmax-wines/x.jpg"));
    c.asset_groups[0].images.push(shared.clone());
    let mut second = asset_group("Gifts", vec![shared]);
    second.search_themes.clear();
    c.asset_groups.push(second);
    let p = build(&with_logo(), vec![c]);
    let copy = |from: &str, to: &str| PictureCopy {
        from: from.into(),
        to: to.into(),
    };
    assert_eq!(
        p.pictures,
        [
            copy(
                "images/pmax-wines/wine-lovers-toast.jpg",
                "pmax-wines/wine-lovers-toast.jpg"
            ),
            copy("images/pmax-wines/x.jpg", "pmax-wines/x.jpg"),
            copy("images/logo.png", "logo.png"),
        ]
    );
    let logos: Vec<&ImageAssetRow> = p
        .image_assets
        .iter()
        .filter(|a| a.field == "LOGO")
        .collect();
    assert_eq!(logos.len(), 2);
    assert!(logos.iter().all(|a| a.image_file == "logo.png"));
}

#[test]
fn app_only_packages_copy_no_logo() {
    let mut i = with_logo();
    i.google_ads.app_id = "123456".into();
    let p = build(
        &i,
        vec![image_campaign("App", "app", CampaignKind::AppInstalls)],
    );
    assert!(p.pictures.iter().all(|c| c.to != "logo.png"));
}

#[test]
fn app_id_comes_from_the_app_section_then_google_ads() {
    let app = || image_campaign("App", "app", CampaignKind::AppInstalls);
    let mut i = with_logo();
    i.app = Some(App {
        store: AppStore::AppStore,
        id: "987".into(),
    });
    let p = build(&i, vec![app()]);
    assert_eq!(p.image_campaigns[0].app_id, "987");
    assert_eq!(p.image_campaigns[0].app_store, "Apple App Store");

    i.app = Some(App {
        store: AppStore::GooglePlay,
        id: "com.vinellu.app".into(),
    });
    let p = build(&i, vec![app()]);
    assert_eq!(p.image_campaigns[0].app_store, "Google Play");

    i.app = None;
    i.google_ads.app_id = " 123456 ".into();
    let p = build(&i, vec![app()]);
    assert_eq!(p.image_campaigns[0].app_id, "123456");
    assert_eq!(p.image_campaigns[0].app_store, "Apple App Store");
}

#[test]
fn a_campaign_without_picture_files_is_a_gap_and_left_out() {
    let mut c = pmax();
    c.asset_groups[0].images = vec![brief("a", AspectRatio::Square, None)];
    let a = account_with(vec![
        campaign("S", "s", CampaignKind::Search, Intent::Generic),
        c,
    ]);
    let i = with_logo();
    assert_eq!(
        image_campaign_gaps(&i, &a),
        [(1, "no picture files".to_string())]
    );
    let p = package(&i, &a, None).expect("package");
    assert!(p.image_campaigns.is_empty());
    assert!(p.image_assets.is_empty());
    assert!(p.pictures.is_empty());
}

#[test]
fn performance_max_and_demand_gen_without_a_logo_are_gaps() {
    let a = account_with(vec![
        pmax(),
        image_campaign("DG", "dg", CampaignKind::DemandGen),
    ]);
    let i = input();
    let reason = "no logo (business.toml logo)".to_string();
    assert_eq!(
        image_campaign_gaps(&i, &a),
        [(0, reason.clone()), (1, reason)]
    );
    assert!(
        package(&i, &a, None)
            .expect("package")
            .image_campaigns
            .is_empty()
    );
}

#[test]
fn app_without_an_app_id_is_a_gap_and_reasons_add_up() {
    let mut app = image_campaign("App", "app", CampaignKind::AppInstalls);
    let mut i = input();
    i.google_ads.app_id = "  ".into();
    assert_eq!(
        image_campaign_gaps(&i, &account_with(vec![app.clone()])),
        [(0, "no app id (google_ads.app_id or [app])".to_string())]
    );
    app.asset_groups[0].images.clear();
    assert_eq!(
        image_campaign_gaps(&i, &account_with(vec![app])),
        [(
            0,
            "no picture files, no app id (google_ads.app_id or [app])".to_string()
        )]
    );
}

#[test]
fn search_campaigns_are_never_gaps() {
    assert!(
        image_campaign_gaps(
            &input(),
            &account_with(vec![campaign(
                "S",
                "s",
                CampaignKind::Search,
                Intent::Generic
            )])
        )
        .is_empty()
    );
}

// Live: the baseline serves `PMax - Wines` and the dropped `PMax - Old` and `Antiga Search`.
fn live() -> Live {
    let old = image_campaign("PMax - Old", "pmax-old", CampaignKind::PerformanceMax);
    let search = campaign(
        "Antiga Search",
        "antiga",
        CampaignKind::Search,
        Intent::Generic,
    );
    let mut performance = Performance::default();
    for name in ["PMax - Wines", "PMax - Old", "Antiga Search"] {
        performance.campaigns.push(CampaignPerf {
            name: name.into(),
            live_status: Some(LiveStatus::Enabled),
            ..Default::default()
        });
    }
    Live {
        baseline: account_with(vec![pmax(), old, search]),
        performance,
    }
}

#[test]
fn live_image_campaigns_are_edited_added_or_paused() {
    let dg = image_campaign("DG New", "dg-new", CampaignKind::DemandGen);
    let p = package(&with_logo(), &account_with(vec![pmax(), dg]), Some(&live())).expect("package");
    let got: Vec<(&str, &str, &str, StructAction)> = p
        .image_campaigns
        .iter()
        .map(|r| {
            (
                r.key.as_str(),
                r.name.as_str(),
                r.campaign_type.as_str(),
                r.action,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "pmax-wines",
                "PMax - Wines",
                "Performance Max",
                StructAction::Edit
            ),
            ("dg-new", "DG New", "Demand Gen", StructAction::Add),
            (
                "pmax-old",
                "PMax - Old",
                "Performance Max",
                StructAction::Pause
            ),
        ]
    );
    let paused = &p.image_campaigns[2];
    assert_eq!(paused.budget, "");
    assert_eq!(paused.bid_strategy, "");
    assert!(p.image_assets.iter().all(|a| a.campaign_key != "pmax-old"));
    assert!(p.pictures.iter().all(|c| !c.from.contains("pmax-old")));
    let search: Vec<(&str, StructAction)> = p
        .campaigns
        .iter()
        .map(|c| (c.name.as_str(), c.action))
        .collect();
    assert_eq!(search, [("Antiga Search", StructAction::Pause)]);
}
