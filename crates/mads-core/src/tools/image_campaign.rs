//! Tools of a Performance Max or Demand Gen campaign mission.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    MissionTools, ToolOutput, ToolSpec,
    campaign::{Empty, campaign_index, not_found},
    image_rules_summary,
    output::{cents_to_f64, parse_args, split},
    rules_summary,
    schema::schema_for,
};
use crate::{
    google::{
        AspectRatio, AssetGroup, Campaign, ImageBrief, Issue, PlannedAdGroup, Rules, normalize,
    },
    workspace::Workspace,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpsertAssetGroupArgs {
    /// Name of a planned asset group of this campaign.
    name: String,
    /// At most 25 characters, usually the business name.
    business_name: String,
    headlines: Vec<String>,
    /// Performance Max only: 1 to 5 of at most 90 characters.
    #[serde(default)]
    long_headlines: Vec<String>,
    descriptions: Vec<String>,
    /// Performance Max only: searches that signal the audience, 0 to 25.
    #[serde(default)]
    search_themes: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetImageBriefsArgs {
    /// Name of an asset group already created with upsert_asset_group.
    asset_group: String,
    /// Replaces every image brief of the asset group.
    images: Vec<BriefArg>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BriefArg {
    /// Slug, unique in the asset group, such as wine-on-table. It becomes the file name.
    id: String,
    ratio: AspectRatio,
    /// What the picture shows, in English: subject, setting, light, composition. No text, logo or words.
    prompt: String,
    /// Catalog id whose real photo must appear in the picture. Empty for none.
    #[serde(default)]
    reference: String,
}

pub fn specs() -> Vec<ToolSpec> {
    let spec = |name: &str, description: &str, input_schema: Value| ToolSpec {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    vec![
        spec(
            "get_brief",
            "Everything about this campaign: plan, business, catalog rows with photos, what is already built and the rules. Call this first.",
            schema_for::<Empty>(),
        ),
        spec(
            "upsert_asset_group",
            "Create or replace the texts of one planned asset group. Its image briefs are kept.",
            schema_for::<UpsertAssetGroupArgs>(),
        ),
        spec(
            "set_image_briefs",
            "Replace the image briefs of one asset group. mads generates the pictures after the mission.",
            schema_for::<SetImageBriefsArgs>(),
        ),
        spec(
            "validate",
            "Validate the campaign as it is now and list errors and warnings. Changes nothing.",
            schema_for::<Empty>(),
        ),
        spec(
            "finish",
            "Finish the campaign mission. Needs every planned asset group with texts and image briefs, and zero errors.",
            schema_for::<Empty>(),
        ),
    ]
}

pub async fn call(t: &MissionTools, slug: &str, name: &str, args: Value) -> ToolOutput {
    match name {
        "get_brief" => get_brief(t, slug).await,
        "upsert_asset_group" => match parse_args(args) {
            Ok(a) => upsert_asset_group(t, slug, a).await,
            Err(e) => e,
        },
        "set_image_briefs" => match parse_args(args) {
            Ok(a) => set_image_briefs(t, slug, a).await,
            Err(e) => e,
        },
        "validate" => validate(t, slug).await,
        "finish" => finish(t, slug).await,
        _ => ToolOutput::fail("UNKNOWN_TOOL", name),
    }
}

async fn get_brief(t: &MissionTools, slug: &str) -> ToolOutput {
    let ws = t.ws.lock().await;
    let Some(i) = campaign_index(&ws, slug) else {
        return not_found(slug);
    };
    let c = &ws.account.campaigns[i];
    let wanted: Vec<&String> = c
        .planned_ad_groups
        .iter()
        .flat_map(|g| &g.entity_ids)
        .collect();
    let entities: Vec<Value> = ws
        .input
        .catalog
        .iter()
        .filter(|e| wanted.contains(&&e.id))
        .map(|e| json!({"id": e.id, "name": e.name, "url": e.url, "category": e.category, "notes": e.notes, "has_photo": e.image.is_some()}))
        .collect();
    let built: Vec<Value> = c
        .asset_groups
        .iter()
        .map(|g| json!({"name": g.name, "headlines": g.headlines.len(), "images": g.images.len()}))
        .collect();
    let mut result = json!({
        "campaign": {
            "name": c.name, "slug": c.slug, "kind": c.kind, "intent": c.intent,
            "daily_budget": cents_to_f64(c.daily_budget),
            "bid_strategy": c.bid_strategy, "rationale": c.rationale,
            "planned_asset_groups": c.planned_ad_groups,
        },
        "brand_kit": ws.account.brand_kit,
        "business": ws.input.business,
        "focus": ws.input.focus,
        "entities": entities,
        "built_asset_groups": built,
        "rules": rules_summary(&t.settings),
        "image_rules": image_rules_summary(),
    });
    if !ws.input.design.is_empty() {
        result["design"] = json!(ws.input.design);
    }
    ToolOutput::ok(result, &[], format!("get_brief {}", c.name))
}

/// Errors and warnings of one asset group as it would be in the campaign.
fn group_issues(ws: &Workspace, t: &MissionTools, c: &Campaign, gi: usize) -> Vec<Issue> {
    let rules = Rules::new(&ws.input, t.settings.max_ad_groups);
    let prefix = format!("campaign.asset_groups[{gi}]");
    rules
        .campaign(c, None, false, "campaign")
        .into_iter()
        .filter(|i| i.path.starts_with(&prefix))
        .collect()
}

/// Saves the candidate campaign when its issues have no error.
fn commit(
    t: &MissionTools,
    ws: &mut Workspace,
    ci: usize,
    candidate: Campaign,
    issues: Vec<Issue>,
    what: String,
) -> ToolOutput {
    let (errors, warnings) = split(issues);
    if !errors.is_empty() {
        let summary = format!("{what}: {} errors", errors.len());
        return ToolOutput::fail_issues(&errors, &warnings, summary);
    }
    ws.account.campaigns[ci] = candidate;
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(json!({"saved": what}), &warnings, what)
}

async fn upsert_asset_group(t: &MissionTools, slug: &str, a: UpsertAssetGroupArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let Some(ci) = campaign_index(ws, slug) else {
        return not_found(slug);
    };
    let mut candidate = ws.account.campaigns[ci].clone();
    let Some(planned) = candidate
        .planned_ad_groups
        .iter()
        .find(|p| normalize(&p.name) == normalize(&a.name))
        .cloned()
    else {
        let msg = format!("'{}' is not a planned asset group of this campaign", a.name);
        return ToolOutput::fail_issues(
            &[Issue::error("E12", "name", msg)],
            &[],
            "upsert_asset_group: not planned",
        );
    };
    let gi = place_group(&mut candidate, &planned, a);
    let issues = group_issues(ws, t, &candidate, gi);
    commit(
        t,
        ws,
        ci,
        candidate,
        issues,
        format!("asset group {}", planned.name),
    )
}

/// Puts the new texts of a planned group in the campaign, keeping its briefs and the plan order.
/// Returns the group's index.
fn place_group(c: &mut Campaign, planned: &PlannedAdGroup, a: UpsertAssetGroupArgs) -> usize {
    let images = c
        .asset_groups
        .iter()
        .find(|g| g.name == planned.name)
        .map(|g| g.images.clone())
        .unwrap_or_default();
    c.asset_groups.retain(|g| g.name != planned.name);
    c.asset_groups.push(AssetGroup {
        name: planned.name.clone(),
        final_url: planned.final_url.clone(),
        business_name: a.business_name,
        headlines: a.headlines,
        long_headlines: a.long_headlines,
        descriptions: a.descriptions,
        search_themes: a.search_themes,
        images,
    });
    let plan = &c.planned_ad_groups;
    c.asset_groups
        .sort_by_key(|g| plan.iter().position(|p| p.name == g.name));
    c.asset_groups
        .iter()
        .position(|g| g.name == planned.name)
        .unwrap_or(0)
}

fn briefs(args: Vec<BriefArg>) -> Vec<ImageBrief> {
    args.into_iter()
        .map(|b| ImageBrief {
            id: b.id.trim().to_string(),
            ratio: b.ratio,
            prompt: b.prompt,
            reference: Some(b.reference.trim().to_string()).filter(|r| !r.is_empty()),
            file: None,
        })
        .collect()
}

async fn set_image_briefs(t: &MissionTools, slug: &str, a: SetImageBriefsArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let Some(ci) = campaign_index(ws, slug) else {
        return not_found(slug);
    };
    let mut candidate = ws.account.campaigns[ci].clone();
    let Some(gi) = candidate
        .asset_groups
        .iter()
        .position(|g| normalize(&g.name) == normalize(&a.asset_group))
    else {
        let msg = format!(
            "asset group '{}' does not exist yet: call upsert_asset_group first",
            a.asset_group
        );
        return ToolOutput::fail_issues(
            &[Issue::error("E12", "asset_group", msg)],
            &[],
            "set_image_briefs: no asset group",
        );
    };
    let group = &mut candidate.asset_groups[gi];
    group.images = briefs(a.images);
    let what = format!("{}: {} image briefs", group.name, group.images.len());
    let images_prefix = format!("campaign.asset_groups[{gi}].images");
    let issues = group_issues(ws, t, &candidate, gi)
        .into_iter()
        .filter(|i| i.path.starts_with(&images_prefix))
        .collect();
    commit(t, ws, ci, candidate, issues, what)
}

/// Issues at finish time. Image files do not exist yet: the image step makes them after the mission.
fn campaign_issues(ws: &Workspace, c: &Campaign, max_ad_groups: usize) -> Vec<Issue> {
    Rules::new(&ws.input, max_ad_groups)
        .campaign(c, None, true, "campaign")
        .into_iter()
        .filter(|i| i.code != "E20")
        .collect()
}

async fn validate(t: &MissionTools, slug: &str) -> ToolOutput {
    let ws = t.ws.lock().await;
    let Some(ci) = campaign_index(&ws, slug) else {
        return not_found(slug);
    };
    let (errors, warnings) = split(campaign_issues(
        &ws,
        &ws.account.campaigns[ci],
        t.settings.max_ad_groups,
    ));
    let summary = format!(
        "validate: {} errors, {} warnings",
        errors.len(),
        warnings.len()
    );
    ToolOutput::ok(
        json!({"errors": errors, "warnings": warnings}),
        &[],
        summary,
    )
}

async fn finish(t: &MissionTools, slug: &str) -> ToolOutput {
    let ws = t.ws.lock().await;
    let Some(ci) = campaign_index(&ws, slug) else {
        return not_found(slug);
    };
    let c = &ws.account.campaigns[ci];
    let mut issues = campaign_issues(&ws, c, t.settings.max_ad_groups);
    for g in c.asset_groups.iter().filter(|g| g.images.is_empty()) {
        let msg = format!("asset group '{}' has no image briefs", g.name);
        issues.push(Issue::error("E16", "campaign.asset_groups", msg));
    }
    let (errors, warnings) = split(issues);
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("finish: {} errors", errors.len()),
        );
    }
    t.mark_finished();
    ToolOutput::ok(
        json!({"asset_groups": c.asset_groups.len()}),
        &warnings,
        "campaign finished",
    )
}
