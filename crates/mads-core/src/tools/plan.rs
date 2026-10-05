use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    MissionTools, ToolOutput, ToolSpec,
    output::{cents_to_f64, parse_args, split},
    rules_summary,
    schema::schema_for,
};
use crate::{
    google::{
        Account, BidStrategy, BrandKit, Campaign, CampaignKind, Cents, Intent, Issue,
        PlannedAdGroup, Rules,
    },
    input::slugify,
    workspace::Workspace,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct QueryCatalogArgs {
    /// Only items of this category (exact, case-insensitive). Empty for all.
    #[serde(default)]
    category: String,
    /// Case-insensitive text searched in id, name and aliases. Empty for all.
    #[serde(default)]
    contains: String,
    #[serde(default)]
    offset: usize,
    /// Page size, at most 200.
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetBrandKitArgs {
    /// 8 to 12 headlines of at most 30 characters, shared by every ad.
    headlines: Vec<String>,
    /// 2 to 3 descriptions of at most 90 characters, shared by every ad.
    descriptions: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetAccountPlanArgs {
    /// 1 to 5 campaigns. Replaces the previous plan.
    campaigns: Vec<PlanCampaign>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanCampaign {
    name: String,
    /// Ad format: search (text ads), performance_max or demand_gen (image campaigns).
    #[serde(default)]
    kind: CampaignKind,
    intent: Intent,
    /// Daily budget in account currency units, at most 2 decimals. All campaigns must sum to the account budget.
    daily_budget: f64,
    bid_strategy: PlanBid,
    /// Why this budget share and this bidding.
    rationale: String,
    /// Ad groups of a search campaign, asset groups of an image campaign.
    ad_groups: Vec<PlanAdGroup>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanBid {
    /// search: manual_cpc. performance_max: maximize_conversions. demand_gen: maximize_clicks or maximize_conversions.
    #[serde(rename = "type")]
    kind: PlanBidType,
}

#[derive(Deserialize, JsonSchema, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PlanBidType {
    ManualCpc,
    MaximizeClicks,
    MaximizeConversions,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlanAdGroup {
    name: String,
    theme: String,
    /// Catalog ids this ad group covers.
    #[serde(default)]
    entity_ids: Vec<String>,
    /// Landing page. Empty to use the entity page (one entity) or the business URL.
    #[serde(default)]
    final_url: String,
}

pub fn specs() -> Vec<ToolSpec> {
    let spec = |name: &str, description: &str, input_schema: Value| ToolSpec {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    vec![
        spec(
            "get_business",
            "Business profile, account budget and the rules every tool enforces. Call this first.",
            schema_for::<Empty>(),
        ),
        spec(
            "query_catalog",
            "List catalog items (entities that can get their own ad group), with categories and pagination.",
            schema_for::<QueryCatalogArgs>(),
        ),
        spec(
            "set_brand_kit",
            "Set the headlines and descriptions shared by every ad. Replaces the previous kit.",
            schema_for::<SetBrandKitArgs>(),
        ),
        spec(
            "set_account_plan",
            "Define campaigns by format, intent, budget split, bidding and the planned ad or asset groups. Replaces the previous plan.",
            schema_for::<SetAccountPlanArgs>(),
        ),
        spec(
            "finish",
            "Finish the plan mission. Needs the brand kit and the account plan.",
            schema_for::<Empty>(),
        ),
    ]
}

pub async fn call(t: &MissionTools, name: &str, args: Value) -> ToolOutput {
    match name {
        "get_business" => get_business(t).await,
        "query_catalog" => match parse_args(args) {
            Ok(a) => query_catalog(t, a).await,
            Err(e) => e,
        },
        "set_brand_kit" => match parse_args(args) {
            Ok(a) => set_brand_kit(t, a).await,
            Err(e) => e,
        },
        "set_account_plan" => match parse_args(args) {
            Ok(a) => set_account_plan(t, a).await,
            Err(e) => e,
        },
        "finish" => finish(t).await,
        _ => ToolOutput::fail("UNKNOWN_TOOL", name),
    }
}

async fn get_business(t: &MissionTools) -> ToolOutput {
    let ws = t.ws.lock().await;
    let b = &ws.input.budget;
    let mut result = json!({
        "business": ws.input.business,
        "budget": {
            "daily": cents_to_f64(b.daily),
            "currency": b.currency,
            "max_cpc": b.max_cpc.map(cents_to_f64),
        },
        "catalog_size": ws.input.catalog.len(),
        "catalog_photos": ws.input.catalog.iter().filter(|c| c.image.is_some()).count(),
        "image_campaigns": image_availability(t, &ws),
        "app_campaigns": availability(kind_unavailable(t, &ws, CampaignKind::AppInstalls)),
        "app": ws.input.app.as_ref().map(|a| json!({"store": a.store, "id": a.id, "store_url": a.store_url()})),
        "focus": ws.input.focus,
        "rules": rules_summary(&t.settings),
    });
    if !ws.input.research.is_empty() {
        result["research"] = json!(ws.input.research);
    }
    if !ws.input.formats.is_empty() {
        result["required_formats"] = json!(ws.input.formats);
    }
    if let Some(live) = &ws.live {
        let (account, performance) = super::live::business_view(live);
        result["live_account"] = account;
        result["performance"] = performance;
    }
    ToolOutput::ok(result, &[], "get_business")
}

/// Why image campaigns can or cannot be planned in this run.
/// Why a campaign of this kind cannot be planned in this run. None: it can.
fn kind_unavailable(t: &MissionTools, ws: &Workspace, kind: CampaignKind) -> Option<&'static str> {
    if !kind.has_images() {
        return None;
    }
    if !t.settings.image_model {
        return Some("no image model in this run (set --image-provider and its API key)");
    }
    match kind {
        CampaignKind::AppInstalls if ws.input.app.is_none() => {
            Some("no app: set [app] store and id in business.toml")
        }
        CampaignKind::AppInstalls => None,
        _ if ws.input.logo.is_none() => Some("no logo: set [brand] logo in business.toml"),
        _ => None,
    }
}

/// Why Performance Max and Demand Gen cannot be planned. None: they can.
fn image_unavailable(t: &MissionTools, ws: &Workspace) -> Option<&'static str> {
    kind_unavailable(t, ws, CampaignKind::DemandGen)
}

fn availability(reason: Option<&str>) -> Value {
    match reason {
        None => json!({"available": true}),
        Some(reason) => json!({"available": false, "reason": reason}),
    }
}

fn image_availability(t: &MissionTools, ws: &Workspace) -> Value {
    availability(image_unavailable(t, ws))
}

fn any_image_kind_available(t: &MissionTools, ws: &Workspace) -> bool {
    image_unavailable(t, ws).is_none()
        || kind_unavailable(t, ws, CampaignKind::AppInstalls).is_none()
}

async fn query_catalog(t: &MissionTools, a: QueryCatalogArgs) -> ToolOutput {
    let ws = t.ws.lock().await;
    let (category, needle) = (
        a.category.trim().to_lowercase(),
        a.contains.trim().to_lowercase(),
    );
    let matches = |c: &&crate::input::CatalogItem| {
        let cat_ok = category.is_empty() || c.category.to_lowercase() == category;
        let text_ok = needle.is_empty()
            || c.id.contains(&needle)
            || c.name.to_lowercase().contains(&needle)
            || c.aliases
                .iter()
                .any(|al| al.to_lowercase().contains(&needle));
        cat_ok && text_ok
    };
    let filtered: Vec<_> = ws.input.catalog.iter().filter(matches).collect();
    let limit = a.limit.clamp(1, 200);
    let items: Vec<_> = filtered.iter().skip(a.offset).take(limit).collect();

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for c in ws.input.catalog.iter().filter(|c| !c.category.is_empty()) {
        *counts.entry(c.category.as_str()).or_default() += 1;
    }
    let categories: Vec<_> = counts
        .into_iter()
        .map(|(name, count)| json!({"name": name, "count": count}))
        .collect();
    let summary = format!("query_catalog: {} of {} items", items.len(), filtered.len());
    ToolOutput::ok(
        json!({"total": filtered.len(), "offset": a.offset, "items": items, "categories": categories}),
        &[],
        summary,
    )
}

async fn set_brand_kit(t: &MissionTools, a: SetBrandKitArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let kit = BrandKit {
        headlines: a.headlines,
        descriptions: a.descriptions,
    };
    let (errors, warnings) = split(Rules::new(&ws.input, t.settings.max_ad_groups).brand_kit(&kit));
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("set_brand_kit: {} errors", errors.len()),
        );
    }
    let summary = format!(
        "brand kit: {} headlines, {} descriptions",
        kit.headlines.len(),
        kit.descriptions.len()
    );
    ws.account.brand_kit = Some(kit);
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(
        json!({"headlines": ws.account.brand_kit.as_ref().map(|k| k.headlines.len()), "descriptions": ws.account.brand_kit.as_ref().map(|k| k.descriptions.len())}),
        &warnings,
        summary,
    )
}

async fn set_account_plan(t: &MissionTools, a: SetAccountPlanArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let rules = Rules::new(&ws.input, t.settings.max_ad_groups);
    let mut issues = Vec::new();
    let campaigns: Vec<Campaign> = a
        .campaigns
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let unavailable = kind_unavailable(t, ws, c.kind);
            build_campaign(ws, &rules, c, i, unavailable, &mut issues)
        })
        .collect();
    check_slugs(&campaigns, &mut issues);
    // `[campaigns] formats` is the advertiser's choice: E21 checks it, nothing else asks why.
    if ws.input.formats.is_empty() {
        if any_image_kind_available(t, ws) {
            check_image_choice(&campaigns, &mut issues);
        }
        if kind_unavailable(t, ws, CampaignKind::AppInstalls).is_none() {
            check_app_choice(&campaigns, &mut issues);
        }
    }
    let candidate = Account {
        brand_kit: ws.account.brand_kit.clone(),
        campaigns,
    };
    issues.extend(
        rules
            .account(&candidate, false)
            .into_iter()
            .filter(|i| i.path.starts_with("campaigns")),
    );
    let (errors, warnings) = split(issues);
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("set_account_plan: {} errors", errors.len()),
        );
    }
    let groups: usize = candidate
        .campaigns
        .iter()
        .map(|c| c.planned_ad_groups.len())
        .sum();
    let summary = format!(
        "plan: {} campaigns, {groups} ad groups",
        candidate.campaigns.len()
    );
    let overview: Vec<Value> = candidate
        .campaigns
        .iter()
        .map(|c| json!({"name": c.name, "slug": c.slug, "ad_groups": c.planned_ad_groups.len()}))
        .collect();
    ws.account.campaigns = candidate.campaigns;
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(json!({"campaigns": overview}), &warnings, summary)
}

fn bid_of(kind: PlanBidType) -> BidStrategy {
    match kind {
        PlanBidType::ManualCpc => BidStrategy::ManualCpc,
        PlanBidType::MaximizeClicks => BidStrategy::MaximizeClicks { max_cpc: None },
        PlanBidType::MaximizeConversions => BidStrategy::MaximizeConversions,
    }
}

fn check_kind(
    c: &PlanCampaign,
    path: &str,
    images_unavailable: Option<&str>,
    issues: &mut Vec<Issue>,
) {
    if c.kind == CampaignKind::Search && c.bid_strategy.kind != PlanBidType::ManualCpc {
        let msg = "search campaigns take only manual_cpc until the Google Ads bulk templates are verified";
        issues.push(Issue::error(
            "UNSUPPORTED",
            format!("{path}.bid_strategy.type"),
            msg,
        ));
    }
    if let (true, Some(reason)) = (c.kind.has_images(), images_unavailable) {
        let msg = format!("{} campaigns are not available: {reason}", c.kind.label());
        issues.push(Issue::error("UNSUPPORTED", format!("{path}.kind"), msg));
    }
}

fn build_campaign(
    ws: &Workspace,
    rules: &Rules,
    c: PlanCampaign,
    i: usize,
    images_unavailable: Option<&str>,
    issues: &mut Vec<Issue>,
) -> Campaign {
    let path = format!("campaigns[{i}]");
    let daily_budget = Cents::from_f64(c.daily_budget).unwrap_or_else(|| {
        let msg = format!(
            "{} must be greater than 0 with at most 2 decimals",
            c.daily_budget
        );
        issues.push(Issue::error("E06", format!("{path}.daily_budget"), msg));
        Cents(0)
    });
    check_kind(&c, &path, images_unavailable, issues);
    let slug = slugify(&c.name);
    if slug.is_empty() {
        issues.push(Issue::error(
            "E12",
            format!("{path}.name"),
            "name needs letters or digits",
        ));
    }
    if c.ad_groups.is_empty() {
        issues.push(Issue::error(
            "E12",
            format!("{path}.ad_groups"),
            "campaign has no planned ad groups",
        ));
    }
    let planned = plan_ad_groups(ws, rules, c.kind, &c.ad_groups, &path, issues);
    Campaign {
        kind: c.kind,
        asset_groups: Vec::new(),
        name: c.name,
        slug,
        intent: c.intent,
        daily_budget,
        bid_strategy: bid_of(c.bid_strategy.kind),
        rationale: c.rationale,
        planned_ad_groups: planned,
        ad_groups: Vec::new(),
        negatives: Vec::new(),
        assets: None,
    }
}

fn plan_ad_groups(
    ws: &Workspace,
    rules: &Rules,
    kind: CampaignKind,
    groups: &[PlanAdGroup],
    path: &str,
    issues: &mut Vec<Issue>,
) -> Vec<PlannedAdGroup> {
    let mut seen = std::collections::BTreeSet::new();
    groups
        .iter()
        .enumerate()
        .map(|(j, g)| {
            let at = format!("{path}.ad_groups[{j}]");
            if !seen.insert(crate::google::normalize(&g.name)) {
                issues.push(Issue::error(
                    "E12",
                    format!("{at}.name"),
                    format!("duplicate planned ad group '{}'", g.name),
                ));
            }
            for id in g
                .entity_ids
                .iter()
                .filter(|id| !ws.input.catalog.iter().any(|c| &c.id == *id))
            {
                issues.push(Issue::error(
                    "E12",
                    format!("{at}.entity_ids"),
                    format!("unknown catalog id '{id}'"),
                ));
            }
            PlannedAdGroup {
                name: g.name.clone(),
                theme: g.theme.clone(),
                entity_ids: g.entity_ids.clone(),
                final_url: match (kind, &ws.input.app) {
                    // App ads link to the store page: the site and the focus do not apply.
                    (CampaignKind::AppInstalls, Some(app)) => app.store_url(),
                    _ => resolve_url(ws, rules, g, &at, issues),
                },
            }
        })
        .collect()
}

fn resolve_url(
    ws: &Workspace,
    rules: &Rules,
    g: &PlanAdGroup,
    at: &str,
    issues: &mut Vec<Issue>,
) -> String {
    if !g.final_url.trim().is_empty() {
        rules.focus_url(issues, &format!("{at}.final_url"), &g.final_url);
        if !rules.url_allowed(&g.final_url) {
            issues.push(Issue::error(
                "E07",
                format!("{at}.final_url"),
                format!("URL not allowed: {}", g.final_url),
            ));
        }
        return g.final_url.clone();
    }
    let single = match g.entity_ids.as_slice() {
        [id] => ws
            .input
            .catalog
            .iter()
            .find(|c| &c.id == id)
            .map(|c| c.url.clone()),
        _ => None,
    };
    let url = single
        .or_else(|| {
            ws.input
                .focus
                .as_ref()
                .and_then(|f| f.urls.first().cloned())
        })
        .unwrap_or_else(|| ws.input.business.url.clone());
    rules.focus_url(issues, &format!("{at}.final_url"), &url);
    url
}

/// The marker a rationale carries when the plan skips image campaigns that were available.
const NO_IMAGE: &str = "No image campaign:";

/// With images available, the plan has an image campaign or says why not, so the choice is visible.
fn check_image_choice(campaigns: &[Campaign], issues: &mut Vec<Issue>) {
    let has_image = campaigns.iter().any(|c| c.kind.has_images());
    let explained = campaigns.iter().any(|c| c.rationale.contains(NO_IMAGE));
    if !has_image && !explained && !campaigns.is_empty() {
        let msg = format!(
            "image campaigns are available: plan one, or start a rationale with '{NO_IMAGE} <reason>'"
        );
        issues.push(Issue::error("NO_IMAGE_REASON", "campaigns", msg));
    }
}

/// The marker a rationale carries when the plan skips the app it could advertise.
const NO_APP: &str = "No app campaign:";

/// With the app available, the plan has an App campaign or says why not. An image campaign that
/// asks people to install the app does worse than an App campaign, which optimizes for installs.
fn check_app_choice(campaigns: &[Campaign], issues: &mut Vec<Issue>) {
    let has_app = campaigns
        .iter()
        .any(|c| c.kind == CampaignKind::AppInstalls);
    let explained = campaigns.iter().any(|c| c.rationale.contains(NO_APP));
    if !has_app && !explained && !campaigns.is_empty() {
        let msg = format!(
            "the app is available: plan an app_installs campaign, or start a rationale with '{NO_APP} <reason>'"
        );
        issues.push(Issue::error("NO_APP_REASON", "campaigns", msg));
    }
}

fn check_slugs(campaigns: &[Campaign], issues: &mut Vec<Issue>) {
    let mut seen = std::collections::BTreeSet::new();
    for (i, c) in campaigns.iter().enumerate() {
        if !c.slug.is_empty() && !seen.insert(c.slug.clone()) {
            issues.push(Issue::error(
                "E12",
                format!("campaigns[{i}].name"),
                format!("name '{}' collides with another campaign", c.name),
            ));
        }
    }
}

async fn finish(t: &MissionTools) -> ToolOutput {
    let ws = t.ws.lock().await;
    let mut errors = Vec::new();
    if ws.account.brand_kit.is_none() {
        errors.push(Issue::error("E12", "brand_kit", "brand kit is not set"));
    }
    if ws.account.campaigns.is_empty() {
        errors.push(Issue::error("E12", "campaigns", "account plan is not set"));
    }
    if !errors.is_empty() {
        return ToolOutput::fail_issues(&errors, &[], "finish: plan incomplete");
    }
    t.mark_finished();
    ToolOutput::ok(
        json!({"campaigns": ws.account.campaigns.len()}),
        &[],
        "plan finished",
    )
}
