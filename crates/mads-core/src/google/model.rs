use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use crate::money::Cents;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub brand_kit: Option<BrandKit>,
    pub campaigns: Vec<Campaign>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrandKit {
    pub headlines: Vec<String>,
    pub descriptions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Brand,
    Catalog,
    Generic,
    Competitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BidStrategy {
    ManualCpc,
    MaximizeClicks { max_cpc: Option<Cents> },
    MaximizeConversions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub name: String,
    pub slug: String,
    pub intent: Intent,
    pub daily_budget: Cents,
    pub bid_strategy: BidStrategy,
    pub rationale: String,
    pub planned_ad_groups: Vec<PlannedAdGroup>,
    pub ad_groups: Vec<AdGroup>,
    pub negatives: Vec<Keyword>,
    pub assets: Option<Assets>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedAdGroup {
    pub name: String,
    pub theme: String,
    pub entity_ids: Vec<String>,
    pub final_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdGroup {
    pub name: String,
    pub default_cpc: Cents,
    pub cpc_rationale: String,
    pub final_url: String,
    pub keywords: Vec<Keyword>,
    pub negatives: Vec<Keyword>,
    pub rsa: Rsa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Phrase,
    Exact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyword {
    pub text: String,
    pub match_type: MatchType,
}

/// Ad group specific part of a responsive search ad. The brand kit completes it at export.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rsa {
    pub headlines: Vec<String>,
    pub descriptions: Vec<String>,
    pub path1: Option<String>,
    pub path2: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Assets {
    pub sitelinks: Vec<Sitelink>,
    pub callouts: Vec<String>,
    pub snippets: Vec<Snippet>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sitelink {
    pub text: String,
    pub description1: Option<String>,
    pub description2: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SnippetHeader {
    Amenities,
    Brands,
    Courses,
    DegreePrograms,
    Destinations,
    FeaturedHotels,
    InsuranceCoverage,
    Models,
    Neighborhoods,
    ServiceCatalog,
    Shows,
    Styles,
    Types,
}

impl SnippetHeader {
    pub const ALL: [SnippetHeader; 13] = [
        SnippetHeader::Amenities,
        SnippetHeader::Brands,
        SnippetHeader::Courses,
        SnippetHeader::DegreePrograms,
        SnippetHeader::Destinations,
        SnippetHeader::FeaturedHotels,
        SnippetHeader::InsuranceCoverage,
        SnippetHeader::Models,
        SnippetHeader::Neighborhoods,
        SnippetHeader::ServiceCatalog,
        SnippetHeader::Shows,
        SnippetHeader::Styles,
        SnippetHeader::Types,
    ];

    /// Header name as Google Ads writes it (English account UI).
    pub fn label(self) -> &'static str {
        match self {
            SnippetHeader::Amenities => "Amenities",
            SnippetHeader::Brands => "Brands",
            SnippetHeader::Courses => "Courses",
            SnippetHeader::DegreePrograms => "Degree programs",
            SnippetHeader::Destinations => "Destinations",
            SnippetHeader::FeaturedHotels => "Featured hotels",
            SnippetHeader::InsuranceCoverage => "Insurance coverage",
            SnippetHeader::Models => "Models",
            SnippetHeader::Neighborhoods => "Neighborhoods",
            SnippetHeader::ServiceCatalog => "Service catalog",
            SnippetHeader::Shows => "Shows",
            SnippetHeader::Styles => "Styles",
            SnippetHeader::Types => "Types",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snippet {
    pub header: SnippetHeader,
    pub values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&Intent::Catalog).unwrap(),
            "\"catalog\""
        );
        assert_eq!(
            serde_json::to_string(&MatchType::Phrase).unwrap(),
            "\"phrase\""
        );
        assert_eq!(
            serde_json::to_string(&SnippetHeader::DegreePrograms).unwrap(),
            "\"degree_programs\""
        );
    }

    #[test]
    fn bid_strategy_roundtrips_with_tag() {
        let b = BidStrategy::MaximizeClicks {
            max_cpc: Some(Cents(150)),
        };
        let json = serde_json::to_value(b).unwrap();
        assert_eq!(json["type"], "maximize_clicks");
        assert_eq!(serde_json::from_value::<BidStrategy>(json).unwrap(), b);
    }

    #[test]
    fn snippet_header_labels_match_google() {
        assert_eq!(SnippetHeader::DegreePrograms.label(), "Degree programs");
        assert_eq!(SnippetHeader::Brands.label(), "Brands");
    }
}
