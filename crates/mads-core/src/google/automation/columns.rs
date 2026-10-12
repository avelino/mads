//! Column positions of every tab of `0. Feed`. Formula builders read letters from here, so no
//! formula hardcodes a column.

use super::sheet::col_letter;

/// One column of the `Feed` tab, in spec order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FeedColumn {
    Action,
    CampaignKey,
    Automated,
    Structure,
    Item,
    Category,
    Active,
    Campaign,
    AdGroup,
    Keyword,
    MatchType,
    MaxCpc,
    FinalUrl,
    Path1,
    Path2,
    /// Headline 1 to 15.
    Headline(u8),
    /// Description 1 to 4.
    Description(u8),
    Label,
    Status,
    CurrentStatus,
    NextStep,
    /// Structural action for the ad group (S3/S5/S6). Distinct from `Action`, which is the keyword.
    AdGroupAction,
}

pub(crate) const HEADLINES: u8 = 15;
pub(crate) const DESCRIPTIONS: u8 = 4;

impl FeedColumn {
    pub fn all() -> Vec<FeedColumn> {
        use FeedColumn::*;
        let mut out = vec![
            Action,
            CampaignKey,
            Automated,
            Structure,
            Item,
            Category,
            Active,
            Campaign,
            AdGroup,
            Keyword,
            MatchType,
            MaxCpc,
            FinalUrl,
            Path1,
            Path2,
        ];
        out.extend((1..=HEADLINES).map(Headline));
        out.extend((1..=DESCRIPTIONS).map(Description));
        out.extend([Label, Status, CurrentStatus, NextStep, AdGroupAction]);
        out
    }

    /// Zero-based column index. A headline or description number out of range is clamped to
    /// the nearest valid one, so the index never lands on another column's block.
    pub fn index(self) -> usize {
        use FeedColumn::*;
        let h = HEADLINES as usize;
        let d = DESCRIPTIONS as usize;
        match self {
            Action => 0,
            CampaignKey => 1,
            Automated => 2,
            Structure => 3,
            Item => 4,
            Category => 5,
            Active => 6,
            Campaign => 7,
            AdGroup => 8,
            Keyword => 9,
            MatchType => 10,
            MaxCpc => 11,
            FinalUrl => 12,
            Path1 => 13,
            Path2 => 14,
            Headline(n) => 14 + (n as usize).clamp(1, h),
            Description(n) => 14 + h + (n as usize).clamp(1, d),
            Label => 15 + h + d,
            Status => 16 + h + d,
            CurrentStatus => 17 + h + d,
            NextStep => 18 + h + d,
            AdGroupAction => 19 + h + d,
        }
    }

    pub fn letter(self) -> String {
        col_letter(self.index())
    }

    pub fn header(self) -> String {
        use FeedColumn::*;
        match self {
            Action => "action".into(),
            CampaignKey => "campaign_key".into(),
            Automated => "automated".into(),
            Structure => "structure".into(),
            Item => "item".into(),
            Category => "category".into(),
            Active => "active".into(),
            Campaign => "Campaign".into(),
            AdGroup => "Ad group".into(),
            Keyword => "Keyword".into(),
            MatchType => "Match Type".into(),
            MaxCpc => "Max CPC".into(),
            FinalUrl => "Final URL".into(),
            Path1 => "Path 1".into(),
            Path2 => "Path 2".into(),
            Headline(n) => format!("Headline {}", n.clamp(1, HEADLINES)),
            Description(n) => format!("Description {}", n.clamp(1, DESCRIPTIONS)),
            Label => "Label".into(),
            Status => "Status".into(),
            CurrentStatus => "current_status".into(),
            NextStep => "next_step".into(),
            AdGroupAction => "ad_group_action".into(),
        }
    }
}

/// `Settings` tab: one row per setting, label in column A, value in column B, note in C.
/// The constants are row offsets from the first data row.
pub(crate) mod settings {
    pub const HEADER: [&str; 3] = ["Setting", "Value", "Note"];
    pub const CUSTOMER_ID: usize = 0;
    pub const DOMAIN: usize = 1;
    pub const TRACKING_TEMPLATE: usize = 2;
    pub const DEFAULT_MAX_CPC: usize = 3;
    pub const MATCH_TYPE: usize = 4;
    pub const APP_ID: usize = 5;
    pub const APP_STORE: usize = 6;
    pub const LABELS: [&str; 7] = [
        "Customer ID",
        "Domain",
        "Tracking template",
        "Default Max CPC",
        "Keyword match type",
        "App ID",
        "App store",
    ];
    /// Column index of the value.
    pub const VALUE: usize = 1;
}

pub(crate) mod catalog {
    pub const HEADER: [&str; 8] = [
        "item_id",
        "item",
        "category",
        "campaign_key",
        "active",
        "keyword",
        "final_url",
        "headline_1",
    ];
    pub const ITEM_ID: usize = 0;
    pub const ITEM: usize = 1;
    pub const CATEGORY: usize = 2;
    pub const CAMPAIGN_KEY: usize = 3;
    pub const ACTIVE: usize = 4;
    pub const KEYWORD: usize = 5;
    pub const FINAL_URL: usize = 6;
    pub const HEADLINE_1: usize = 7;
}

pub(crate) mod campaigns {
    pub const HEADER: [&str; 14] = [
        "campaign_key",
        "automated",
        "Campaign",
        "Campaign type",
        "Networks",
        "Budget",
        "Budget type",
        "Bid Strategy Type",
        "Language",
        "Location",
        "Devices",
        "Tracking template",
        "Label",
        "action",
    ];
    pub const KEY: usize = 0;
    pub const AUTOMATED: usize = 1;
    pub const CAMPAIGN: usize = 2;
    pub const CAMPAIGN_TYPE: usize = 3;
    pub const NETWORKS: usize = 4;
    pub const BUDGET: usize = 5;
    pub const BUDGET_TYPE: usize = 6;
    pub const BID_STRATEGY: usize = 7;
    pub const LANGUAGE: usize = 8;
    pub const LOCATION: usize = 9;
    pub const DEVICES: usize = 10;
    pub const TRACKING_TEMPLATE: usize = 11;
    pub const LABEL: usize = 12;
    pub const ACTION: usize = 13;
}

/// `Fixed Keywords` tab.
pub(crate) mod fixed {
    pub const HEADER: [&str; 8] = [
        "action",
        "campaign_key",
        "Ad group",
        "Keyword",
        "Match Type",
        "Max CPC",
        "Final URL",
        "status",
    ];
    pub const ACTION: usize = 0;
    pub const CAMPAIGN_KEY: usize = 1;
    pub const AD_GROUP: usize = 2;
    pub const KEYWORD: usize = 3;
    pub const MATCH_TYPE: usize = 4;
    pub const MAX_CPC: usize = 5;
    pub const FINAL_URL: usize = 6;
    pub const STATUS: usize = 7;
}

/// `Ad Templates` tab: Headline n at column n, Description n at 15 + n, paths last.
pub(crate) mod templates {
    pub const HEADER: [&str; 22] = [
        "campaign_key",
        "Headline 1",
        "Headline 2",
        "Headline 3",
        "Headline 4",
        "Headline 5",
        "Headline 6",
        "Headline 7",
        "Headline 8",
        "Headline 9",
        "Headline 10",
        "Headline 11",
        "Headline 12",
        "Headline 13",
        "Headline 14",
        "Headline 15",
        "Description 1",
        "Description 2",
        "Description 3",
        "Description 4",
        "Path 1",
        "Path 2",
    ];
    pub const KEY: usize = 0;
    pub const HEADLINE_1: usize = 1;
    pub const PATH_1: usize = 20;
    pub const PATH_2: usize = 21;

    /// Column of Headline `n` (1 to 15).
    pub const fn headline(n: usize) -> usize {
        n
    }

    /// Column of Description `n` (1 to 4).
    pub const fn description(n: usize) -> usize {
        15 + n
    }
}

pub(crate) mod negatives {
    pub const HEADER: [&str; 6] = [
        "Level",
        "Campaign",
        "Ad group",
        "Negative keyword",
        "Match Type",
        "Reason",
    ];
    pub const LEVEL: usize = 0;
    pub const CAMPAIGN: usize = 1;
    pub const AD_GROUP: usize = 2;
    pub const KEYWORD: usize = 3;
    pub const MATCH_TYPE: usize = 4;
    pub const REASON: usize = 5;
}

/// Written by the script. `key` is `Campaign|Ad group`.
pub(crate) mod account_status {
    pub const HEADER: [&str; 4] = ["Campaign", "Ad group", "Status", "key"];
    pub const CAMPAIGN: usize = 0;
    pub const AD_GROUP: usize = 1;
    pub const STATUS: usize = 2;
    pub const KEY: usize = 3;
}

/// Written by the script.
pub(crate) mod auto_negatives {
    pub const HEADER: [&str; 5] = [
        "Campaign",
        "Ad group",
        "Negative keyword",
        "Match Type",
        "Reason",
    ];
    pub const CAMPAIGN: usize = 0;
    pub const AD_GROUP: usize = 1;
    pub const KEYWORD: usize = 2;
    pub const MATCH_TYPE: usize = 3;
    pub const REASON: usize = 4;
}

/// `Image Campaigns` tab of `0. Feed`: one row per Performance Max, Demand Gen or App campaign.
pub(crate) mod image_campaigns {
    pub const HEADER: [&str; 13] = [
        "campaign_key",
        "Campaign",
        "Campaign type",
        "Budget",
        "Bid strategy",
        "Max CPC",
        "Language",
        "Location",
        "Final URL",
        "Business name",
        "App ID",
        "App store",
        "action",
    ];
    pub const KEY: usize = 0;
    pub const CAMPAIGN: usize = 1;
    pub const CAMPAIGN_TYPE: usize = 2;
    pub const BUDGET: usize = 3;
    pub const BID_STRATEGY: usize = 4;
    pub const MAX_CPC: usize = 5;
    pub const LANGUAGE: usize = 6;
    pub const LOCATION: usize = 7;
    pub const FINAL_URL: usize = 8;
    pub const BUSINESS_NAME: usize = 9;
    pub const APP_ID: usize = 10;
    pub const APP_STORE: usize = 11;
    pub const ACTION: usize = 12;
}

/// `Image Assets` tab of `0. Feed`: one row per text or picture of an image campaign group.
pub(crate) mod image_assets {
    pub const HEADER: [&str; 5] = ["campaign_key", "Group", "Field", "Text", "Image file"];
    pub const KEY: usize = 0;
    pub const GROUP: usize = 1;
    pub const FIELD: usize = 2;
    pub const TEXT: usize = 3;
    pub const IMAGE_FILE: usize = 4;
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_ORDER: [&str; 39] = [
        "action",
        "campaign_key",
        "automated",
        "structure",
        "item",
        "category",
        "active",
        "Campaign",
        "Ad group",
        "Keyword",
        "Match Type",
        "Max CPC",
        "Final URL",
        "Path 1",
        "Path 2",
        "Headline 1",
        "Headline 2",
        "Headline 3",
        "Headline 4",
        "Headline 5",
        "Headline 6",
        "Headline 7",
        "Headline 8",
        "Headline 9",
        "Headline 10",
        "Headline 11",
        "Headline 12",
        "Headline 13",
        "Headline 14",
        "Headline 15",
        "Description 1",
        "Description 2",
        "Description 3",
        "Description 4",
        "Label",
        "Status",
        "current_status",
        "next_step",
        "ad_group_action",
    ];

    #[test]
    fn feed_columns_follow_the_spec_order() {
        let all = FeedColumn::all();
        let headers: Vec<String> = all.iter().map(|c| c.header()).collect();
        assert_eq!(headers, SPEC_ORDER);
        for (i, c) in all.iter().enumerate() {
            assert_eq!(c.index(), i, "{c:?}");
        }
        assert_eq!(FeedColumn::AdGroupAction.index(), all.len() - 1);
    }

    #[test]
    fn feed_column_letters() {
        assert_eq!(FeedColumn::Action.letter(), "A");
        assert_eq!(FeedColumn::Automated.letter(), "C");
        assert_eq!(FeedColumn::Campaign.letter(), "H");
        assert_eq!(FeedColumn::AdGroup.letter(), "I");
        assert_eq!(FeedColumn::Headline(1).letter(), "P");
        assert_eq!(FeedColumn::Description(4).letter(), "AH");
        assert_eq!(FeedColumn::CurrentStatus.letter(), "AK");
        assert_eq!(FeedColumn::NextStep.letter(), "AL");
        assert_eq!(FeedColumn::AdGroupAction.letter(), "AM");
    }

    #[test]
    fn out_of_range_numbers_stay_in_their_block() {
        assert_eq!(
            FeedColumn::Headline(99).index(),
            FeedColumn::Headline(15).index()
        );
        assert_eq!(
            FeedColumn::Description(0).index(),
            FeedColumn::Description(1).index()
        );
    }

    #[test]
    fn tab_constants_point_at_their_header() {
        assert_eq!(settings::LABELS[settings::CUSTOMER_ID], "Customer ID");
        assert_eq!(settings::LABELS[settings::APP_STORE], "App store");
        assert_eq!(catalog::HEADER[catalog::HEADLINE_1], "headline_1");
        assert_eq!(catalog::HEADER[catalog::ACTIVE], "active");
        assert_eq!(campaigns::HEADER[campaigns::ACTION], "action");
        assert_eq!(campaigns::HEADER[campaigns::LABEL], "Label");
        assert_eq!(fixed::HEADER[fixed::STATUS], "status");
        assert_eq!(templates::HEADER[templates::headline(15)], "Headline 15");
        assert_eq!(
            templates::HEADER[templates::description(1)],
            "Description 1"
        );
        assert_eq!(templates::HEADER[templates::PATH_2], "Path 2");
        assert_eq!(negatives::HEADER[negatives::REASON], "Reason");
        assert_eq!(account_status::HEADER[account_status::KEY], "key");
        assert_eq!(auto_negatives::HEADER[auto_negatives::REASON], "Reason");
    }
}
