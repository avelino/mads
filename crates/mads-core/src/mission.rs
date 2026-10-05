use crate::{agent::MissionSpec, google::Campaign, input::Input};

pub const PLAN_ID: &str = "plan";

const PLAN_PROMPT: &str = include_str!("../prompts/plan.md");
const CAMPAIGN_PROMPT: &str = include_str!("../prompts/campaign.md");
const IMAGE_CAMPAIGN_PROMPT: &str = include_str!("../prompts/image-campaign.md");

pub fn campaign_id(slug: &str) -> String {
    format!("campaign:{slug}")
}

pub fn plan_mission(input: &Input) -> MissionSpec {
    MissionSpec {
        id: PLAN_ID.into(),
        system: PLAN_PROMPT.into(),
        user: format!(
            "Plan the Google Ads account for {}. Write every ad text in {}. Call get_business first.",
            input.business.name, input.business.language
        ),
        web_search: false,
    }
}

pub fn campaign_mission(input: &Input, campaign: &Campaign) -> MissionSpec {
    MissionSpec {
        id: campaign_id(&campaign.slug),
        system: if campaign.kind.has_images() {
            IMAGE_CAMPAIGN_PROMPT.into()
        } else {
            CAMPAIGN_PROMPT.into()
        },
        user: format!(
            "Build the campaign '{}' of {}. Write every ad text in {}. Call get_brief first.",
            campaign.name, input.business.name, input.business.language
        ),
        web_search: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{BidStrategy, Campaign, Cents, Intent},
        testutil,
    };

    fn campaign() -> Campaign {
        Campaign {
            kind: Default::default(),
            asset_groups: Vec::new(),
            name: "Vinellu - Catalogo".into(),
            slug: "vinellu-catalogo".into(),
            intent: Intent::Catalog,
            daily_budget: Cents(3000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![],
            negatives: vec![],
            assets: None,
        }
    }

    #[test]
    fn plan_mission_has_id_language_and_tool_names() {
        let m = plan_mission(&testutil::input());
        assert_eq!(m.id, "plan");
        assert!(m.user.contains("get_business"));
        assert!(m.user.contains("pt-BR"));
        for tool in [
            "get_business",
            "query_catalog",
            "set_brand_kit",
            "set_account_plan",
            "finish",
        ] {
            assert!(m.system.contains(tool), "plan prompt misses {tool}");
        }
    }

    #[test]
    fn campaign_mission_is_scoped_by_slug_and_names_the_campaign() {
        let m = campaign_mission(&testutil::input(), &campaign());
        assert_eq!(m.id, "campaign:vinellu-catalogo");
        assert!(
            m.user.contains("Vinellu - Catalogo")
                && m.user.contains("get_brief")
                && m.user.contains("pt-BR")
        );
        for tool in [
            "get_brief",
            "upsert_ad_group",
            "set_campaign_negatives",
            "set_assets",
            "validate",
            "finish",
        ] {
            assert!(m.system.contains(tool), "campaign prompt misses {tool}");
        }
    }

    #[test]
    fn prompts_carry_the_playbook_rules() {
        let m = campaign_mission(&testutil::input(), &campaign());
        for rule in [
            "30 characters",
            "No `!`",
            "Never invent facts",
            "phrase and exact",
            "cpc_rationale",
            "4 sitelinks",
            "`focus`",
            "business.restricted",
            "E23",
            "first-page bid",
            "W08",
            "W09",
            "do not sell",
        ] {
            assert!(
                m.system.contains(rule),
                "campaign prompt misses rule: {rule}"
            );
        }
        let p = plan_mission(&testutil::input());
        assert!(p.system.contains("sum exactly") && p.system.contains("10 to 20 percent"));
        for rule in [
            "`research`",
            "expected return",
            "competition",
            "contain a competitor name",
            "Never mix",
            "catch-all",
            "nobody types",
        ] {
            assert!(p.system.contains(rule), "plan prompt misses rule: {rule}");
        }
    }

    #[test]
    fn image_campaigns_get_the_image_prompt_with_their_tools_and_rules() {
        let mut c = campaign();
        c.kind = crate::google::CampaignKind::PerformanceMax;
        let m = campaign_mission(&testutil::input(), &c);
        for needle in [
            "get_brief",
            "upsert_asset_group",
            "set_image_briefs",
            "validate",
            "finish",
            "No text, no logo",
            "`reference`",
            "has_photo",
            "adults only",
            "`design`",
            "brand color",
            "Never invent facts",
            "in English",
        ] {
            assert!(m.system.contains(needle), "image prompt misses {needle}");
        }
        let p = plan_mission(&testutil::input());
        for rule in [
            "`kind`",
            "performance_max",
            "demand_gen",
            "image_campaigns.available",
            "conversion_tracking",
            "does not need conversion tracking",
            "app_installs",
            "app_campaigns.available",
            "20 percent of the daily budget",
            "No image campaign:",
            "No app campaign:",
            "business.restricted",
            "NO_APP_REASON",
            "NO_IMAGE_REASON",
            "required_formats",
            "E21",
            "`focus`",
            "E22",
            "E23",
        ] {
            assert!(p.system.contains(rule), "plan prompt misses rule: {rule}");
        }
    }

    #[test]
    fn mission_ids_are_stable() {
        assert_eq!(PLAN_ID, "plan");
        assert_eq!(campaign_id("x"), "campaign:x");
    }
}
