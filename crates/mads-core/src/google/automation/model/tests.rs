use super::*;
use crate::google::{
    Account, AdGroup, BidStrategy, BrandKit, Campaign, CampaignKind, Intent, Keyword, MatchType,
    PlannedAdGroup, Rsa,
};
use crate::input::{Budget, Business, CatalogItem, ExportConfig, ExportStatus, GoogleAds, Input};
use crate::money::Cents;
use crate::perf::{CampaignPerf, Live, LiveStatus, Performance};

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

const LONG: &str = "Bodega Catena Zapata Malbec Argentino 2019";

pub(super) fn input() -> Input {
    Input {
        business: Business {
            name: "Vinellu: \"Wines\"".into(),
            url: "https://vinellu.com".into(),
            language: "pt-BR".into(),
            locations: vec!["Brazil".into(), "Portugal".into()],
            goal: "signups".into(),
            description: "Wine app.".into(),
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
            url_suffix: "utm_source=google".into(),
            eu_political_ads: false,
            decimal_comma: true,
        },
        research: String::new(),
        catalog: vec![
            item("alamos", "Alamos Malbec"),
            item("long", LONG),
            item("unlisted", "Unlisted"),
            item("old-wine", "Old Wine"),
        ],
        logo: None,
        formats: Vec::new(),
        design: String::new(),
        focus: None,
        app: None,
        google_ads: GoogleAds {
            customer_id: "000-000-0000".into(),
            tracking_template: "{lpurl}?utm_campaign={mads_campaign}".into(),
            devices: "Mobile;Desktop".into(),
            labels: vec!["mads".into(), "search".into()],
            app_id: "com.vinellu.app".into(),
        },
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
        final_url: format!(
            "https://vinellu.com/{}",
            name.to_lowercase().replace(' ', "-")
        ),
        keywords,
        negatives: vec![],
        rsa: Rsa::default(),
    }
}

fn planned(name: &str, ids: &[&str]) -> PlannedAdGroup {
    PlannedAdGroup {
        name: name.into(),
        theme: String::new(),
        entity_ids: ids.iter().map(|s| s.to_string()).collect(),
        final_url: String::new(),
    }
}

pub(super) fn campaign(name: &str, slug: &str, kind: CampaignKind, intent: Intent) -> Campaign {
    Campaign {
        name: name.into(),
        slug: slug.into(),
        kind,
        intent,
        daily_budget: Cents(5000),
        bid_strategy: BidStrategy::ManualCpc,
        rationale: String::new(),
        planned_ad_groups: vec![],
        ad_groups: vec![],
        asset_groups: vec![],
        negatives: vec![],
        assets: None,
    }
}

fn automated() -> Campaign {
    let mut c = campaign(
        "Catalog - Wines",
        "catalog-wines",
        CampaignKind::Search,
        Intent::Catalog,
    );
    c.planned_ad_groups = vec![
        planned("Alamos Malbec", &["alamos", "missing", "alamos"]),
        planned("Long", &["long"]),
    ];
    let mut alamos = group(
        "Alamos Malbec",
        vec![
            kw("alamos", MatchType::Exact),
            kw("Alamos  MALBEC", MatchType::Phrase),
        ],
    );
    alamos.rsa = Rsa {
        headlines: vec!["Wine Ratings".into()],
        descriptions: vec!["Rated by drinkers.".into()],
        path1: Some("wines".into()),
        path2: None,
    };
    c.ad_groups = vec![alamos, group("Long", vec![])];
    c.negatives = vec![kw("vinegar", MatchType::Phrase)];
    c
}

fn brand() -> Campaign {
    let mut c = campaign("Brand", "brand", CampaignKind::Search, Intent::Brand);
    let mut g = group(
        "Brand terms",
        vec![
            kw("vinellu", MatchType::Exact),
            kw("vinellu app", MatchType::Phrase),
        ],
    );
    g.negatives = vec![kw("job", MatchType::Exact)];
    g.rsa = Rsa {
        headlines: vec!["Vinellu".into(), "Wine Reviews".into()],
        descriptions: vec!["The wine app.".into()],
        path1: Some("app".into()),
        path2: Some("now".into()),
    };
    c.ad_groups = vec![g];
    c.negatives = vec![kw("free", MatchType::Phrase)];
    c
}

pub(super) fn account() -> Account {
    Account {
        brand_kit: Some(BrandKit {
            headlines: vec!["Kit headline".into()],
            descriptions: vec!["Kit description".into()],
        }),
        campaigns: vec![
            automated(),
            campaign(
                "PMax",
                "pmax",
                CampaignKind::PerformanceMax,
                Intent::Catalog,
            ),
            brand(),
        ],
    }
}

fn build() -> Package {
    package(&input(), &account(), None).expect("package")
}

fn strings(values: &[&str], len: usize) -> Vec<String> {
    let mut out: Vec<String> = values.iter().map(|v| v.to_string()).collect();
    out.resize(len, String::new());
    out
}

#[test]
fn only_search_catalog_campaigns_are_automated_and_other_kinds_are_skipped() {
    let p = build();
    assert_eq!(p.automated_campaigns, ["Catalog - Wines"]);
    let keys: Vec<(&str, bool)> = p
        .campaigns
        .iter()
        .map(|c| (c.key.as_str(), c.automated))
        .collect();
    assert_eq!(keys, [("catalog-wines", true), ("brand", false)]);
    assert!(p.templates.iter().all(|t| t.campaign_key != "pmax"));
}

#[test]
fn package_header_fields_come_from_the_business() {
    let p = build();
    assert_eq!(p.folder, "Vinellu Wines Ads Automation");
    assert_eq!(p.business, "Vinellu: \"Wines\"");
    assert!(p.has_app);
    assert!(!p.live);
}

#[test]
fn campaign_rows_carry_the_editor_values() {
    let p = build();
    assert_eq!(
        p.campaigns[1],
        CampaignRow {
            key: "brand".into(),
            automated: false,
            name: "Brand".into(),
            campaign_type: "Search".into(),
            networks: "Google search".into(),
            budget: "50.00".into(),
            budget_type: "Daily".into(),
            bid_strategy: "Manual CPC".into(),
            language: "pt".into(),
            location: "Brazil".into(),
            devices: "Mobile;Desktop".into(),
            tracking_template: "{lpurl}?utm_campaign=brand".into(),
            label: "mads;search".into(),
            action: StructAction::Add,
        }
    );
}

#[test]
fn catalog_rows_follow_entity_ids_found_in_the_catalog_without_duplicates() {
    let p = build();
    let ids: Vec<&str> = p.catalog.iter().map(|r| r.item_id.as_str()).collect();
    assert_eq!(ids, ["alamos", "long"]);
    let r = &p.catalog[0];
    assert_eq!(r.item, "Alamos Malbec");
    assert_eq!(r.category, "red");
    assert_eq!(r.campaign_key, "catalog-wines");
    assert!(r.active);
    assert_eq!(r.final_url, "https://vinellu.com/w/alamos");
}

#[test]
fn catalog_keyword_is_the_matching_keyword_of_the_group_else_the_lowercased_name() {
    let p = build();
    assert_eq!(p.catalog[0].keyword, "Alamos  MALBEC");
    assert_eq!(p.catalog[1].keyword, LONG.to_lowercase());
}

#[test]
fn catalog_headline_1_is_the_name_up_to_30_characters() {
    let p = build();
    assert_eq!(p.catalog[0].headline_1, "Alamos Malbec");
    assert_eq!(p.catalog[1].headline_1, "", "{LONG} is over 30 characters");
}

#[test]
fn templates_merge_the_first_ad_group_and_pad() {
    let p = build();
    assert_eq!(
        p.templates[0],
        TemplateRow {
            campaign_key: "catalog-wines".into(),
            headlines: strings(&["Wine Ratings", "Kit headline"], 15),
            descriptions: strings(&["Rated by drinkers.", "Kit description"], 4),
            path1: "wines".into(),
            path2: String::new(),
        }
    );
    assert_eq!(p.templates[1].campaign_key, "brand");
    assert_eq!(p.templates.len(), 2);
}

#[test]
fn fixed_rows_are_one_per_keyword_of_fixed_campaigns() {
    let p = build();
    assert_eq!(p.fixed.len(), 2);
    assert_eq!(
        p.fixed[0],
        FixedRow {
            action: StructAction::Add,
            group_action: StructAction::Add,
            campaign_key: "brand".into(),
            campaign: "Brand".into(),
            ad_group: "Brand terms".into(),
            keyword: "vinellu".into(),
            match_type: "Exact".into(),
            max_cpc: "1.50".into(),
            final_url: "https://vinellu.com/brand-terms".into(),
            path1: "app".into(),
            path2: "now".into(),
            headlines: strings(&["Vinellu", "Wine Reviews", "Kit headline"], 15),
            descriptions: strings(&["The wine app.", "Kit description"], 4),
            label: "mads;search".into(),
        }
    );
    assert_eq!(p.fixed[1].keyword, "vinellu app");
    assert_eq!(p.fixed[1].match_type, "Phrase");
}

#[test]
fn negatives_expand_fixed_groups_and_list_campaign_negatives() {
    let p = build();
    let got: Vec<(&str, &str, &str, &str, &str)> = p
        .negatives
        .iter()
        .map(|n| {
            assert_eq!(n.reason, "");
            (
                n.level.as_str(),
                n.campaign.as_str(),
                n.ad_group.as_str(),
                n.keyword.as_str(),
                n.match_type.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("Campaign", "Catalog - Wines", "", "vinegar", "Phrase"),
            ("Campaign", "Brand", "", "free", "Phrase"),
            ("Ad group", "Brand", "Brand terms", "free", "Phrase"),
            ("Ad group", "Brand", "Brand terms", "job", "Exact"),
        ]
    );
}

#[test]
fn settings_blank_the_placeholder_customer_id_and_use_the_campaignid_valuetrack() {
    let p = build();
    assert_eq!(
        p.settings,
        Settings {
            customer_id: String::new(),
            domain: "https://vinellu.com".into(),
            tracking_template: "{lpurl}?utm_campaign={campaignid}".into(),
            default_max_cpc: "3.00".into(),
            match_type: "Phrase".into(),
            app_id: "com.vinellu.app".into(),
            app_store: "Google Play".into(),
        }
    );
}

#[test]
fn settings_without_app_or_max_cpc_are_empty() {
    let mut i = input();
    i.google_ads.app_id = " ".into();
    i.google_ads.customer_id = "123-456-7890".into();
    i.budget.max_cpc = None;
    let p = package(&i, &account(), None).expect("package");
    assert!(!p.has_app);
    assert_eq!(p.settings.app_id, "");
    assert_eq!(p.settings.app_store, "");
    assert_eq!(p.settings.customer_id, "123-456-7890");
}

#[test]
fn without_budget_max_cpc_the_default_bid_is_the_median_of_the_automated_groups() {
    let mut i = input();
    i.budget.max_cpc = None;
    let mut a = account();
    for (g, cents) in a.campaigns[0].ad_groups.iter_mut().zip([100, 300]) {
        g.default_cpc = Cents(cents);
    }
    let mut third = a.campaigns[0].ad_groups[0].clone();
    third.name = "Third".into();
    third.default_cpc = Cents(200);
    a.campaigns[0].ad_groups.push(third);
    let p = package(&i, &a, None).expect("package");
    assert_eq!(p.settings.default_max_cpc, "2.00");

    a.campaigns.retain(|c| c.intent != Intent::Catalog);
    let p = package(&i, &a, None).expect("package");
    assert_eq!(
        p.settings.default_max_cpc, "",
        "no automated group, no bid to infer"
    );

    i.budget.max_cpc = Some(Cents(150));
    let p = package(&i, &a, None).expect("package");
    assert_eq!(p.settings.default_max_cpc, "1.50", "budget.max_cpc wins");
}

/// `n` catalog items, all listed by the automated campaign.
fn with_items(n: usize) -> (Input, Account) {
    let mut i = input();
    i.catalog = (0..n)
        .map(|k| item(&format!("i{k}"), &format!("Wine {k}")))
        .collect();
    let mut a = account();
    let ids: Vec<String> = i.catalog.iter().map(|c| c.id.clone()).collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    a.campaigns[0].planned_ad_groups = vec![planned("All", &ids)];
    (i, a)
}

#[test]
fn capacity_is_at_least_150_and_one_and_a_half_times_the_catalog() {
    assert_eq!(build().capacity, 150);
    let (i, a) = with_items(101);
    assert_eq!(package(&i, &a, None).expect("package").capacity, 152);
    let (i, a) = with_items(666);
    assert_eq!(package(&i, &a, None).expect("package").capacity, 999);
}

#[test]
fn capacity_counts_the_catalog_rows_of_every_automated_campaign() {
    let (i, mut a) = with_items(80);
    let mut second = a.campaigns[0].clone();
    second.name = "Catalog - More".into();
    second.slug = "catalog-more".into();
    a.campaigns.push(second);
    let p = package(&i, &a, None).expect("package");
    assert_eq!(p.catalog.len(), 160);
    assert_eq!(p.capacity, 240);
}

#[test]
fn capacity_over_1000_rows_is_an_error() {
    let (i, a) = with_items(667);
    assert!(matches!(
        package(&i, &a, None),
        Err(ExportError::Capacity(_))
    ));
}

#[test]
fn negatives_over_2000_rows_is_an_error() {
    let mut a = account();
    a.campaigns[2].negatives = (0..1000)
        .map(|k| kw(&format!("neg {k}"), MatchType::Phrase))
        .collect();
    // 1001 campaign rows (with the automated one) plus 1001 in the ad group: 2002.
    assert!(matches!(
        package(&input(), &a, None),
        Err(ExportError::Capacity(_))
    ));
}

#[test]
fn search_ads_without_a_brand_kit_is_an_error() {
    let mut a = account();
    a.brand_kit = None;
    assert_eq!(
        package(&input(), &a, None),
        Err(ExportError::MissingBrandKit)
    );
}

// Live account. The baseline serves the two plan campaigns plus `Antiga`, which the new plan
// dropped. Brand's `Old group` and keyword `vinellu old` are dropped too, as is the automated
// campaign's `Old Wine` item.
fn live() -> Live {
    let mut cat = automated();
    cat.ad_groups
        .push(group("Old Wine", vec![kw("old wine", MatchType::Phrase)]));
    let mut b = brand();
    b.ad_groups[0].keywords = vec![
        kw("vinellu", MatchType::Exact),
        kw("vinellu old", MatchType::Phrase),
    ];
    b.ad_groups
        .push(group("Old group", vec![kw("old", MatchType::Exact)]));
    let antiga = campaign("Antiga", "antiga", CampaignKind::Search, Intent::Generic);
    let mut performance = Performance::default();
    for name in ["Catalog - Wines", "Brand", "Antiga"] {
        performance.campaigns.push(CampaignPerf {
            name: name.into(),
            live_status: Some(LiveStatus::Enabled),
            ..Default::default()
        });
    }
    Live {
        baseline: Account {
            brand_kit: None,
            campaigns: vec![cat, b, antiga],
        },
        performance,
    }
}

/// `account()` plus a new Search campaign the live account does not have.
fn new_plan() -> Account {
    let mut a = account();
    let mut nova = campaign("Nova", "nova", CampaignKind::Search, Intent::Generic);
    nova.ad_groups = vec![group("nova", vec![kw("nova", MatchType::Exact)])];
    a.campaigns.push(nova);
    a
}

fn build_live() -> Package {
    package(&input(), &new_plan(), Some(&live())).expect("package")
}

#[test]
fn live_campaigns_are_edited_added_or_paused() {
    let p = build_live();
    assert!(p.live);
    let got: Vec<(&str, &str, StructAction)> = p
        .campaigns
        .iter()
        .map(|c| (c.key.as_str(), c.name.as_str(), c.action))
        .collect();
    assert_eq!(
        got,
        [
            ("catalog-wines", "Catalog - Wines", StructAction::Edit),
            ("brand", "Brand", StructAction::Edit),
            ("nova", "Nova", StructAction::Add),
            ("antiga", "Antiga", StructAction::Pause),
        ]
    );
    let paused = &p.campaigns[3];
    assert_eq!(paused.campaign_type, "");
    assert_eq!(paused.budget, "");
    assert!(!paused.automated);
}

#[test]
fn live_fixed_keywords_are_edited_added_or_paused() {
    let p = build_live();
    let got: Vec<(&str, &str, &str, StructAction, StructAction)> = p
        .fixed
        .iter()
        .map(|f| {
            (
                f.campaign.as_str(),
                f.ad_group.as_str(),
                f.keyword.as_str(),
                f.action,
                f.group_action,
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            // Existing group: keyword Edit/Add, group stays Edit so S3/S5/S6 do not re-add it.
            (
                "Brand",
                "Brand terms",
                "vinellu",
                StructAction::Edit,
                StructAction::Edit
            ),
            (
                "Brand",
                "Brand terms",
                "vinellu app",
                StructAction::Add,
                StructAction::Edit
            ),
            ("Nova", "nova", "nova", StructAction::Add, StructAction::Add),
            (
                "Brand",
                "Brand terms",
                "vinellu old",
                StructAction::Pause,
                StructAction::Edit
            ),
            (
                "Brand",
                "Old group",
                "",
                StructAction::Pause,
                StructAction::Pause
            ),
        ]
    );
    let dropped_keyword = &p.fixed[3];
    assert_eq!(dropped_keyword.campaign_key, "brand");
    assert_eq!(dropped_keyword.match_type, "Phrase");
    assert_eq!(dropped_keyword.headlines, strings(&[], 15));
    assert_eq!(dropped_keyword.descriptions, strings(&[], 4));
    let dropped_group = &p.fixed[4];
    assert_eq!(dropped_group.match_type, "");
}

#[test]
fn live_existing_group_keeps_edit_when_its_first_keyword_is_new() {
    // Reorder so the first planned keyword is new: keyword_action is Add, group_action must stay
    // Edit or the structural import tries to add the existing group and its ad.
    let mut plan = new_plan();
    let brand_camp = plan
        .campaigns
        .iter_mut()
        .find(|c| c.name == "Brand")
        .expect("Brand");
    brand_camp.ad_groups[0].keywords = vec![
        kw("vinellu app", MatchType::Phrase),
        kw("vinellu", MatchType::Exact),
    ];
    let p = package(&input(), &plan, Some(&live())).expect("package");
    let brand: Vec<_> = p
        .fixed
        .iter()
        .filter(|f| {
            f.ad_group == "Brand terms" && !f.keyword.is_empty() && f.action != StructAction::Pause
        })
        .collect();
    assert_eq!(brand[0].keyword, "vinellu app");
    assert_eq!(brand[0].action, StructAction::Add);
    assert_eq!(brand[0].group_action, StructAction::Edit);
    assert_eq!(brand[1].keyword, "vinellu");
    assert_eq!(brand[1].action, StructAction::Edit);
    assert_eq!(brand[1].group_action, StructAction::Edit);
}

#[test]
fn live_dropped_catalog_item_stays_inactive() {
    let p = build_live();
    let ids: Vec<(&str, bool)> = p
        .catalog
        .iter()
        .map(|r| (r.item_id.as_str(), r.active))
        .collect();
    assert_eq!(ids, [("alamos", true), ("long", true), ("old-wine", false)]);
    let old = &p.catalog[2];
    assert_eq!(old.item, "Old Wine");
    assert_eq!(old.campaign_key, "catalog-wines");
    assert_eq!(old.keyword, "old wine");
    assert_eq!(old.final_url, "https://vinellu.com/w/old-wine");
}

#[test]
fn live_dropped_group_missing_from_the_catalog_keeps_its_name() {
    let mut l = live();
    l.baseline.campaigns[0]
        .ad_groups
        .push(group("Gone Item", vec![]));
    let p = package(&input(), &new_plan(), Some(&l)).expect("package");
    let gone = p.catalog.last().expect("row");
    assert_eq!(gone.item, "Gone Item");
    assert_eq!(gone.item_id, "");
    assert!(!gone.active);
}

#[test]
fn an_automated_campaign_group_with_no_catalog_item_stays_as_fixed_rows() {
    let mut c = automated();
    c.planned_ad_groups.push(planned("How to taste", &[]));
    c.planned_ad_groups
        .push(planned("Ghost", &["not-in-catalog"]));
    c.ad_groups.push(group(
        "How to taste",
        vec![kw("how to taste wine", MatchType::Phrase)],
    ));
    c.ad_groups
        .push(group("Ghost", vec![kw("ghost wine", MatchType::Exact)]));
    let mut acc = account();
    acc.campaigns[0] = c;
    let p = package(&input(), &acc, None).unwrap();
    let fixed: Vec<(&str, &str, &str)> = p
        .fixed
        .iter()
        .filter(|f| f.campaign_key == "catalog-wines")
        .map(|f| {
            (
                f.ad_group.as_str(),
                f.keyword.as_str(),
                f.match_type.as_str(),
            )
        })
        .collect();
    assert_eq!(
        fixed,
        vec![
            ("How to taste", "how to taste wine", "Phrase"),
            ("Ghost", "ghost wine", "Exact")
        ]
    );
    assert!(p.catalog.iter().all(|r| r.item != "How to taste"));
}
