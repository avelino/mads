//! Shared fixtures for unit tests.
use crate::input::{Budget, Business, CatalogItem, ExportConfig, ExportStatus, Input, Page};
use crate::money::Cents;

pub fn input() -> Input {
    let item = |id: &str, name: &str, url: &str, category: &str, alias: &str| CatalogItem {
        image: None,
        id: id.into(),
        name: name.into(),
        url: url.into(),
        category: category.into(),
        aliases: vec![alias.into()],
        third_party: true,
        notes: String::new(),
    };
    Input {
        logo: None,
        formats: Vec::new(),
        design: String::new(),
        focus: None,
        app: None,
        business: Business {
            name: "Vinellu".into(),
            url: "https://vinellu.com".into(),
            language: "pt-BR".into(),
            locations: vec!["Brazil".into()],
            goal: "cadastros no app".into(),
            description: "App social de vinhos com reviews, safras e harmonização.".into(),
            conversion_tracking: false,
            restricted: vec![],
            brand_terms: vec!["vinellu".into()],
            competitors: vec!["Vivino".into()],
            avoid: vec!["melhor do mundo".into()],
            pages: ["app", "sobre", "blog", "ajuda"]
                .iter()
                .map(|p| Page {
                    name: (*p).into(),
                    url: format!("https://vinellu.com/{p}"),
                })
                .collect(),
        },
        budget: Budget {
            daily: Cents(5000),
            currency: "BRL".into(),
            max_cpc: Some(Cents(300)),
        },
        export: ExportConfig {
            status: ExportStatus::Paused,
            url_suffix: "utm_campaign={mads_campaign}".into(),
            eu_political_ads: false,
            decimal_comma: true,
        },
        research: String::new(),
        catalog: vec![
            item(
                "alamos-malbec",
                "Alamos Malbec",
                "https://vinellu.com/w/alamos",
                "malbec",
                "alamos",
            ),
            item(
                "luigi-bosca",
                "Luigi Bosca",
                "https://vinellu.com/w/luigi",
                "malbec",
                "luigi bosca malbec",
            ),
            item(
                "cartuxa",
                "Cartuxa",
                "https://vinellu.com/w/cartuxa",
                "alentejo",
                "cartuxa reserva",
            ),
        ],
    }
}
