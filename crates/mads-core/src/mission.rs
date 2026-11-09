use crate::{agent::MissionSpec, google::Campaign, input::Input};

pub const PLAN_ID: &str = "plan";

const PLAN_CORE: &str = include_str!("../prompts/plan/core.md");
const PLAN_FOCUS: &str = include_str!("../prompts/plan/focus.md");
const PLAN_RESTRICTED: &str = include_str!("../prompts/plan/restricted.md");
const PLAN_APPS: &str = include_str!("../prompts/plan/apps.md");
const PLAN_LIVE: &str = include_str!("../prompts/plan/live.md");
const PLAN_TARGETS: &str = include_str!("../prompts/plan/targets.md");
const PLAN_FINISH: &str = include_str!("../prompts/plan/finish.md");
const CAMPAIGN_PROMPT: &str = include_str!("../prompts/campaign.md");
const IMAGE_CAMPAIGN_PROMPT: &str = include_str!("../prompts/image-campaign.md");

pub fn campaign_id(slug: &str) -> String {
    format!("campaign:{slug}")
}

/// The plan prompt for this run. A section that talks about something the run does not have
/// (a focus, restricted categories, an app, a live account, targets) is left out: every token of
/// it would be read at each step of the mission and bring nothing.
pub fn plan_prompt(input: &Input, live: bool) -> String {
    let b = &input.budget;
    let targets = live && (b.target_cpa.is_some() || b.target_cpi.is_some());
    let sections = [
        (true, PLAN_CORE),
        (input.focus.is_some(), PLAN_FOCUS),
        (!input.business.restricted.is_empty(), PLAN_RESTRICTED),
        (!input.apps.is_empty(), PLAN_APPS),
        (live, PLAN_LIVE),
        (targets, PLAN_TARGETS),
        (true, PLAN_FINISH),
    ];
    sections
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, text)| text.trim_end())
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n"
}

/// `live` is true when the run starts from an account that already runs (`mads optimize`).
pub fn plan_mission(input: &Input, live: bool) -> MissionSpec {
    MissionSpec {
        id: PLAN_ID.into(),
        system: plan_prompt(input, live),
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
            app_store: None,
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

    /// An input with everything a section of the plan prompt is about.
    fn full_input() -> crate::input::Input {
        let mut input = testutil::input();
        input.focus = Some(crate::input::Focus {
            name: "Rota".into(),
            urls: vec!["https://example.com/rota".into()],
            terms: vec![],
        });
        input.business.restricted = vec![crate::input::RestrictedCategory::Alcohol];
        input.apps = vec![crate::input::App {
            store: crate::input::AppStore::GooglePlay,
            id: "com.example.app".into(),
        }];
        input.budget.target_cpa = Some(Cents(4000));
        input
    }

    #[test]
    fn plan_mission_has_id_language_and_tool_names() {
        let m = plan_mission(&testutil::input(), false);
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
            "E26",
            "LIVE_DETAIL",
            "W09",
            "do not sell",
            "`live`",
            "first_page_bid",
            "thin_rule",
            "live_rules",
            "E28",
            "W10",
            "hidden_terms_cost",
            "get_ad_group_performance",
        ] {
            assert!(
                m.system.contains(rule),
                "campaign prompt misses rule: {rule}"
            );
        }
        let p = plan_mission(&full_input(), true);
        assert!(p.system.contains("sum exactly") && p.system.contains("10 to 20 percent"));
        for rule in [
            "`research`",
            "expected return",
            "competition",
            "contain a competitor name",
            "Never mix",
            "catch-all",
            "nobody types",
            "live_account",
            "Keep the name",
            "ignored_campaigns",
            "E25",
            "Drop <group>:",
            "live_rules",
            "E27",
            "idle_budget",
            "budget_cap",
            "vs_target",
            "target_cpa",
            "target_cpi",
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
            "`live`",
            "Best or Good",
            "get_ad_group_performance",
            "LIVE_DETAIL",
        ] {
            assert!(m.system.contains(needle), "image prompt misses {needle}");
        }
        let p = plan_mission(&full_input(), true);
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
            "`apps`",
            "`app_store`",
            "one campaign per store",
            "E30",
            "E31",
            "cannot change the app",
            "cost per install",
            "test share",
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
    fn the_plan_prompt_leaves_out_what_the_run_does_not_have() {
        let lean = plan_prompt(&testutil::input(), false);
        for gone in [
            "`focus`",
            "business.restricted",
            "app_installs` shows ads",
            "E30",
            "E31",
            "live_account",
            "vs_target",
        ] {
            assert!(!lean.contains(gone), "a plain run does not need: {gone}");
        }
        for kept in [
            "set_account_plan",
            "Brand kit",
            "sum exactly",
            "NO_IMAGE_REASON",
        ] {
            assert!(lean.contains(kept), "every run needs: {kept}");
        }
        let full = plan_prompt(&full_input(), true);
        assert!(full.len() > lean.len() + 5000, "the sections add up");
        let no_targets = plan_prompt(&testutil::input(), true);
        assert!(no_targets.contains("live_account") && !no_targets.contains("vs_target"));
        let first_run = plan_prompt(&full_input(), false);
        assert!(
            !first_run.contains("vs_target"),
            "targets judge a live account only"
        );
    }

    #[test]
    fn mission_ids_are_stable() {
        assert_eq!(PLAN_ID, "plan");
        assert_eq!(campaign_id("x"), "campaign:x");
    }
}
