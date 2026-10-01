use crate::{agent::MissionSpec, google::Campaign, input::Input};

pub const PLAN_ID: &str = "plan";

const PLAN_PROMPT: &str = include_str!("../prompts/plan.md");
const CAMPAIGN_PROMPT: &str = include_str!("../prompts/campaign.md");

pub fn campaign_id(slug: &str) -> String {
    format!("campaign:{slug}")
}

pub fn plan_mission(input: &Input) -> MissionSpec {
    MissionSpec {
        id: PLAN_ID.into(),
        system: PLAN_PROMPT.into(),
        user: format!(
            "Plan the Google Ads Search account for {}. Write every ad text in {}. Call get_business first.",
            input.business.name, input.business.language
        ),
    }
}

pub fn campaign_mission(input: &Input, campaign: &Campaign) -> MissionSpec {
    MissionSpec {
        id: campaign_id(&campaign.slug),
        system: CAMPAIGN_PROMPT.into(),
        user: format!(
            "Build the campaign '{}' of {}. Write every ad text in {}. Call get_brief first.",
            campaign.name, input.business.name, input.business.language
        ),
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
        ] {
            assert!(
                m.system.contains(rule),
                "campaign prompt misses rule: {rule}"
            );
        }
        let p = plan_mission(&testutil::input());
        assert!(p.system.contains("sum exactly") && p.system.contains("10 to 20 percent"));
    }

    #[test]
    fn mission_ids_are_stable() {
        assert_eq!(PLAN_ID, "plan");
        assert_eq!(campaign_id("x"), "campaign:x");
    }
}
