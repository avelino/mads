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

impl BidStrategy {
    /// Bid strategy type as Google Ads writes it.
    pub fn label(self) -> &'static str {
        match self {
            BidStrategy::ManualCpc => "Manual CPC",
            BidStrategy::MaximizeClicks { .. } => "Maximize clicks",
            BidStrategy::MaximizeConversions => "Maximize conversions",
        }
    }
}

/// Google Ads campaign format. Search shows text ads; the others show images.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CampaignKind {
    #[default]
    Search,
    PerformanceMax,
    DemandGen,
}

impl CampaignKind {
    pub fn has_images(self) -> bool {
        self != CampaignKind::Search
    }

    /// Campaign type as Google Ads Editor writes it.
    pub fn label(self) -> &'static str {
        match self {
            CampaignKind::Search => "Search",
            CampaignKind::PerformanceMax => "Performance Max",
            CampaignKind::DemandGen => "Demand Gen",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub kind: CampaignKind,
    pub intent: Intent,
    pub daily_budget: Cents,
    pub bid_strategy: BidStrategy,
    pub rationale: String,
    /// Planned ad groups of a Search campaign, planned asset groups of an image campaign.
    pub planned_ad_groups: Vec<PlannedAdGroup>,
    pub ad_groups: Vec<AdGroup>,
    #[serde(default)]
    pub asset_groups: Vec<AssetGroup>,
    pub negatives: Vec<Keyword>,
    pub assets: Option<Assets>,
}

/// Texts and images of one Performance Max asset group or Demand Gen ad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetGroup {
    pub name: String,
    pub final_url: String,
    pub business_name: String,
    pub headlines: Vec<String>,
    pub long_headlines: Vec<String>,
    pub descriptions: Vec<String>,
    pub search_themes: Vec<String>,
    pub images: Vec<ImageBrief>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
// Variants carry no doc comments: schemars would turn them into a `oneOf` some providers reject.
// Landscape is 1.91:1, square 1:1, portrait 4:5, vertical 9:16.
pub enum AspectRatio {
    Landscape,
    Square,
    Portrait,
    Vertical,
}

impl AspectRatio {
    pub const ALL: [AspectRatio; 4] = [
        AspectRatio::Landscape,
        AspectRatio::Square,
        AspectRatio::Portrait,
        AspectRatio::Vertical,
    ];

    /// Width and height of the file mads writes: Google's recommended size.
    pub fn size(self) -> (u32, u32) {
        match self {
            AspectRatio::Landscape => (1200, 628),
            AspectRatio::Square => (1200, 1200),
            AspectRatio::Portrait => (960, 1200),
            AspectRatio::Vertical => (1080, 1920),
        }
    }

    /// Smallest width and height Google accepts.
    pub fn min_size(self) -> (u32, u32) {
        match self {
            AspectRatio::Landscape => (600, 314),
            AspectRatio::Square => (300, 300),
            AspectRatio::Portrait => (480, 600),
            AspectRatio::Vertical => (600, 1067),
        }
    }

    pub fn value(self) -> f64 {
        let (w, h) = self.size();
        f64::from(w) / f64::from(h)
    }

    /// Column prefix in the Google Ads Editor CSV.
    pub fn label(self) -> &'static str {
        match self {
            AspectRatio::Landscape => "Landscape image",
            AspectRatio::Square => "Square image",
            AspectRatio::Portrait => "Portrait image",
            AspectRatio::Vertical => "Vertical image",
        }
    }
}

/// What one picture should show. The image step turns it into a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBrief {
    pub id: String,
    pub ratio: AspectRatio,
    pub prompt: String,
    /// Catalog id whose real photo the model must use.
    pub reference: Option<String>,
    /// Path relative to `google-ads/editor/`, set once the image exists.
    pub file: Option<String>,
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
