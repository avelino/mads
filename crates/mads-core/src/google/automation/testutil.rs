//! A small package shared by the workbook and script tests.

use super::model::{
    CampaignRow, CatalogRow, FixedRow, ImageAssetRow, ImageCampaignRow, NegativeRow, Package,
    PictureCopy, Settings, StructAction, TemplateRow,
};

fn padded(values: &[&str], len: usize) -> Vec<String> {
    let mut out: Vec<String> = values.iter().map(|v| (*v).to_string()).collect();
    out.resize(len, String::new());
    out
}

fn campaign(key: &str, name: &str, automated: bool) -> CampaignRow {
    CampaignRow {
        key: key.into(),
        automated,
        name: name.into(),
        campaign_type: "Search".into(),
        networks: "Google search".into(),
        budget: "50.00".into(),
        budget_type: "Daily".into(),
        bid_strategy: "Manual CPC".into(),
        language: "pt".into(),
        location: "Brazil".into(),
        devices: "Mobile;Desktop;Tablet".into(),
        tracking_template: "{lpurl}?utm_source=google".into(),
        label: "mads".into(),
        action: StructAction::Add,
    }
}

fn fixed(keyword: &str) -> FixedRow {
    FixedRow {
        action: StructAction::Add,
        group_action: StructAction::Add,
        campaign_key: "brand".into(),
        campaign: "Brand".into(),
        ad_group: "Brand terms".into(),
        keyword: keyword.into(),
        match_type: "Exact".into(),
        max_cpc: "1.50".into(),
        final_url: "https://vinellu.com".into(),
        path1: "app".into(),
        path2: String::new(),
        headlines: padded(&["Vinellu", "Wine Reviews", "Scan the Label"], 15),
        descriptions: padded(&["Real ratings from people who drank it."], 4),
        label: "mads".into(),
    }
}

/// One automated campaign with two catalog items (one name carries a quote and a comma) and one
/// fixed campaign with one ad group of two keywords. No Customer ID, no app.
pub(crate) fn sample_package() -> Package {
    Package {
        folder: "Vinellu Ads Automation".into(),
        business: "Vinellu".into(),
        settings: Settings {
            customer_id: String::new(),
            domain: "https://vinellu.com".into(),
            tracking_template: "{lpurl}?utm_source=google".into(),
            default_max_cpc: "1.00".into(),
            match_type: "Phrase".into(),
            app_id: String::new(),
            app_store: String::new(),
        },
        campaigns: vec![
            campaign("catalog-wines", "Catalog - Wines", true),
            campaign("brand", "Brand", false),
        ],
        catalog: vec![
            CatalogRow {
                item_id: "alamos-malbec".into(),
                item: "Alamos Malbec".into(),
                category: "red".into(),
                campaign_key: "catalog-wines".into(),
                active: true,
                keyword: "alamos malbec".into(),
                final_url: "https://vinellu.com/w/alamos".into(),
                headline_1: "Alamos Malbec".into(),
            },
            CatalogRow {
                item_id: "don-melchor-2019".into(),
                item: "Don \"Melchor\", 2019".into(),
                category: "red".into(),
                campaign_key: "catalog-wines".into(),
                active: true,
                keyword: "don melchor".into(),
                final_url: "https://vinellu.com/w/don-melchor".into(),
                headline_1: "Don \"Melchor\", 2019".into(),
            },
        ],
        fixed: vec![fixed("vinellu"), fixed("vinellu app")],
        templates: vec![
            TemplateRow {
                campaign_key: "catalog-wines".into(),
                headlines: padded(
                    &["Wine Ratings", "Real Ratings", "Compare Before Buying"],
                    15,
                ),
                descriptions: padded(&["See the rating before you open the bottle."], 4),
                path1: "wines".into(),
                path2: String::new(),
            },
            TemplateRow {
                campaign_key: "brand".into(),
                headlines: padded(&["Vinellu", "Wine Reviews"], 15),
                descriptions: padded(&["The wine app."], 4),
                path1: String::new(),
                path2: String::new(),
            },
        ],
        negatives: vec![NegativeRow {
            level: "Campaign".into(),
            campaign: "Brand".into(),
            ad_group: String::new(),
            keyword: "job".into(),
            match_type: "Phrase".into(),
            reason: "Not a customer search".into(),
        }],
        capacity: 150,
        automated_campaigns: vec!["Catalog - Wines".into()],
        has_app: false,
        live: false,
        image_campaigns: vec![ImageCampaignRow {
            key: "pmax-wines".into(),
            name: "PMax - Wines".into(),
            campaign_type: "Performance Max".into(),
            budget: "40.00".into(),
            bid_strategy: "maximize_conversions".into(),
            max_cpc: String::new(),
            language: "pt".into(),
            location: "Brazil".into(),
            final_url: "https://vinellu.com".into(),
            business_name: "Vinellu".into(),
            app_id: String::new(),
            app_store: String::new(),
            action: StructAction::Add,
        }],
        image_assets: sample_image_assets(),
        pictures: vec![
            PictureCopy {
                from: "images/pmax-wines/toast-landscape.jpg".into(),
                to: "pmax-wines/toast-landscape.jpg".into(),
            },
            PictureCopy {
                from: "images/logo.png".into(),
                to: "logo.png".into(),
            },
        ],
    }
}

fn asset(field: &str, text: &str, image_file: &str) -> ImageAssetRow {
    ImageAssetRow {
        campaign_key: "pmax-wines".into(),
        group: "Wine lovers".into(),
        field: field.into(),
        text: text.into(),
        image_file: image_file.into(),
    }
}

fn sample_image_assets() -> Vec<ImageAssetRow> {
    vec![
        asset("HEADLINE", "Wine Ratings", ""),
        asset("HEADLINE", "Scan the Label", ""),
        asset("HEADLINE", "Real Reviews", ""),
        asset(
            "LONG_HEADLINE",
            "See the rating before you open the bottle",
            "",
        ),
        asset("DESCRIPTION", "Real ratings from people who drank it.", ""),
        asset("DESCRIPTION", "Scan any label and see the score.", ""),
        asset("BUSINESS_NAME", "Vinellu", ""),
        asset("MARKETING_IMAGE", "", "pmax-wines/toast-landscape.jpg"),
        asset("LOGO", "", "logo.png"),
    ]
}
