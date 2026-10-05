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
        AdGroup, Assets, Campaign, Cents, Intent, Issue, Keyword, KeywordSpec, MatchType, Rsa,
        Rules, Sitelink, Snippet, SnippetHeader, expand_keywords, merge_rsa, normalize,
        variant_themes,
    },
    workspace::Workspace,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpsertAdGroupArgs {
    /// Name of a planned ad group of this campaign.
    name: String,
    /// Default max CPC in currency units, at most 2 decimals.
    default_cpc: f64,
    #[serde(default)]
    cpc_rationale: String,
    keywords: KeywordsArg,
    /// Ad group level negatives.
    #[serde(default)]
    negatives: Vec<KwArg>,
    rsa: RsaArg,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct KeywordsArg {
    /// 1 to 6 ways people write the name. mads builds the keyword list from variants x modifiers.
    variants: Vec<String>,
    /// 0 to 10 intent words appended to each variant.
    #[serde(default)]
    modifiers: Vec<String>,
    /// Also add every variant as exact match.
    #[serde(default = "yes")]
    exact_heads: bool,
    /// 0 to 20 explicit keywords added as they are.
    #[serde(default)]
    extra: Vec<KwArg>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct KwArg {
    text: String,
    #[serde(rename = "match")]
    match_type: MatchType,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RsaArg {
    /// 3 to 7 headlines of at most 30 characters specific to this ad group.
    headlines: Vec<String>,
    /// 1 to 2 descriptions of at most 90 characters specific to this ad group.
    descriptions: Vec<String>,
    /// Display path, at most 15 characters. Empty for none.
    #[serde(default)]
    path1: String,
    #[serde(default)]
    path2: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetCampaignNegativesArgs {
    /// Replaces the campaign level negative list.
    negatives: Vec<KwArg>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetAssetsArgs {
    /// 2 to 8 sitelinks. URLs must be business pages or catalog URLs.
    sitelinks: Vec<SitelinkArg>,
    /// 2 to 10 callouts of at most 25 characters.
    callouts: Vec<String>,
    /// 0 to 2 structured snippets.
    #[serde(default)]
    snippets: Vec<SnippetArg>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SitelinkArg {
    /// At most 25 characters.
    text: String,
    /// At most 35 characters. Give both descriptions or none.
    #[serde(default)]
    description1: String,
    #[serde(default)]
    description2: String,
    url: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SnippetArg {
    header: SnippetHeader,
    /// 3 to 10 values of at most 25 characters.
    values: Vec<String>,
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
            "Everything about this campaign: plan, brand kit, business, catalog rows, what is already built and the rules. Call this first.",
            schema_for::<Empty>(),
        ),
        spec(
            "upsert_ad_group",
            "Create or replace one planned ad group: keyword spec (expanded by mads), negatives, CPC and the ad group specific ad texts.",
            schema_for::<UpsertAdGroupArgs>(),
        ),
        spec(
            "set_campaign_negatives",
            "Replace the campaign level negative keywords shared by all its ad groups.",
            schema_for::<SetCampaignNegativesArgs>(),
        ),
        spec(
            "set_assets",
            "Set sitelinks, callouts and structured snippets for this campaign.",
            schema_for::<SetAssetsArgs>(),
        ),
        spec(
            "validate",
            "Validate the campaign as it is now and list errors and warnings. Changes nothing.",
            schema_for::<Empty>(),
        ),
        spec(
            "finish",
            "Finish the campaign mission. Needs every planned ad group, assets and zero errors.",
            schema_for::<Empty>(),
        ),
    ]
}

pub async fn call(t: &MissionTools, slug: &str, name: &str, args: Value) -> ToolOutput {
    match name {
        "get_brief" => get_brief(t, slug).await,
        "upsert_ad_group" => match parse_args(args) {
            Ok(a) => upsert_ad_group(t, slug, a).await,
            Err(e) => e,
        },
        "set_campaign_negatives" => match parse_args(args) {
            Ok(a) => set_campaign_negatives(t, slug, a).await,
            Err(e) => e,
        },
        "set_assets" => match parse_args(args) {
            Ok(a) => set_assets(t, slug, a).await,
            Err(e) => e,
        },
        "validate" => validate(t, slug).await,
        "finish" => finish(t, slug).await,
        _ => ToolOutput::fail("UNKNOWN_TOOL", name),
    }
}

pub(super) fn campaign_index(ws: &Workspace, slug: &str) -> Option<usize> {
    ws.account.campaigns.iter().position(|c| c.slug == slug)
}

fn keywords(args: Vec<KwArg>) -> Vec<Keyword> {
    args.into_iter()
        .map(|k| Keyword {
            text: k.text,
            match_type: k.match_type,
        })
        .collect()
}

pub(super) fn not_found(slug: &str) -> ToolOutput {
    ToolOutput::fail("NOT_FOUND", format!("campaign '{slug}' no longer exists"))
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
    let entities: Vec<_> = ws
        .input
        .catalog
        .iter()
        .filter(|e| wanted.contains(&&e.id))
        .collect();
    let built: Vec<Value> = c
        .ad_groups
        .iter()
        .map(|g| json!({"name": g.name, "keywords": g.keywords.len(), "negatives": g.negatives.len(), "default_cpc": cents_to_f64(g.default_cpc)}))
        .collect();
    let mut result = json!({
        "campaign": {
            "name": c.name, "slug": c.slug, "intent": c.intent,
            "daily_budget": cents_to_f64(c.daily_budget),
            "bid_strategy": c.bid_strategy, "rationale": c.rationale,
            "planned_ad_groups": c.planned_ad_groups,
        },
        "brand_kit": ws.account.brand_kit,
        "business": ws.input.business,
        "focus": ws.input.focus,
        "budget": {"currency": ws.input.budget.currency, "max_cpc": ws.input.budget.max_cpc.map(cents_to_f64)},
        "entities": entities,
        "built_ad_groups": built,
        "campaign_negatives": c.negatives,
        "assets_set": c.assets.is_some(),
        "rules": rules_summary(&t.settings),
    });
    if let Some(live) = &ws.live {
        let (ran, performance) = super::live::brief_view(live, &c.name);
        result["live"] = ran;
        result["performance"] = performance;
    }
    ToolOutput::ok(result, &[], format!("get_brief {}", c.name))
}

fn count_issue(out: &mut Vec<Issue>, path: &str, n: usize, min: usize, max: usize) {
    if n < min || n > max {
        out.push(Issue::error(
            "E13",
            path,
            format!("{n} found, expected {min} to {max}"),
        ));
    }
}

fn build_ad_group(
    a: UpsertAdGroupArgs,
    planned_name: &str,
    final_url: &str,
    intent: Intent,
    issues: &mut Vec<Issue>,
) -> AdGroup {
    count_issue(issues, "keywords.variants", a.keywords.variants.len(), 1, 6);
    count_issue(
        issues,
        "keywords.modifiers",
        a.keywords.modifiers.len(),
        0,
        10,
    );
    count_issue(issues, "keywords.extra", a.keywords.extra.len(), 0, 20);
    variant_themes(issues, "keywords.variants", &a.keywords.variants);
    let default_cpc = Cents::from_f64(a.default_cpc).unwrap_or_else(|| {
        issues.push(Issue::error(
            "E11",
            "default_cpc",
            "CPC must be greater than 0 with at most 2 decimals",
        ));
        Cents(0)
    });
    let spec = KeywordSpec {
        variants: a.keywords.variants,
        modifiers: a.keywords.modifiers,
        exact_heads: a.keywords.exact_heads,
        one_word_phrase: intent == Intent::Brand,
        extra: keywords(a.keywords.extra),
    };
    let path = |p: String| Some(p.trim().to_string()).filter(|p| !p.is_empty());
    AdGroup {
        name: planned_name.to_string(),
        default_cpc,
        cpc_rationale: a.cpc_rationale,
        final_url: final_url.to_string(),
        keywords: expand_keywords(&spec),
        negatives: keywords(a.negatives),
        rsa: Rsa {
            headlines: a.rsa.headlines,
            descriptions: a.rsa.descriptions,
            path1: path(a.rsa.path1),
            path2: path(a.rsa.path2),
        },
    }
}

async fn upsert_ad_group(t: &MissionTools, slug: &str, a: UpsertAdGroupArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let Some(ci) = campaign_index(ws, slug) else {
        return not_found(slug);
    };
    let campaign = &ws.account.campaigns[ci];
    let Some(planned) = campaign
        .planned_ad_groups
        .iter()
        .find(|p| normalize(&p.name) == normalize(&a.name))
    else {
        let msg = format!("'{}' is not a planned ad group of this campaign", a.name);
        return ToolOutput::fail_issues(
            &[Issue::error("E12", "name", msg)],
            &[],
            "upsert_ad_group: not planned",
        );
    };
    let mut issues = Vec::new();
    let (name, url) = (planned.name.clone(), planned.final_url.clone());
    let ad_group = build_ad_group(a, &name, &url, campaign.intent, &mut issues);

    let mut candidate = campaign.clone();
    candidate
        .ad_groups
        .retain(|g| normalize(&g.name) != normalize(&name));
    candidate.ad_groups.push(ad_group);
    let order = |g: &AdGroup| {
        candidate
            .planned_ad_groups
            .iter()
            .position(|p| p.name == g.name)
    };
    let mut sorted = candidate.ad_groups.clone();
    sorted.sort_by_key(order);
    candidate.ad_groups = sorted;
    let gi = candidate
        .ad_groups
        .iter()
        .position(|g| g.name == name)
        .unwrap_or(0);

    let rules = Rules::new(&ws.input, t.settings.max_ad_groups);
    let prefix = format!("campaign.ad_groups[{gi}]");
    let found = rules.campaign(&candidate, ws.account.brand_kit.as_ref(), false, "campaign");
    issues.extend(found.into_iter().filter(|i| i.path.starts_with(&prefix)));
    let (errors, warnings) = split(issues);
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("upsert_ad_group {name}: {} errors", errors.len()),
        );
    }

    let g = &candidate.ad_groups[gi];
    let merged = ws.account.brand_kit.as_ref().map(|k| merge_rsa(&g.rsa, k));
    let (h, d) = merged.map_or((g.rsa.headlines.len(), g.rsa.descriptions.len()), |m| {
        (m.headlines.len(), m.descriptions.len())
    });
    let summary = format!(
        "{name}: {} kw, {} neg, rsa {h}/{d}",
        g.keywords.len(),
        g.negatives.len()
    );
    let result = json!({"ad_group": name, "keywords": g.keywords.len(), "negatives": g.negatives.len(), "merged_headlines": h, "merged_descriptions": d});
    ws.account.campaigns[ci] = candidate;
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(result, &warnings, summary)
}

async fn set_campaign_negatives(
    t: &MissionTools,
    slug: &str,
    a: SetCampaignNegativesArgs,
) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let Some(ci) = campaign_index(ws, slug) else {
        return not_found(slug);
    };
    let mut candidate = ws.account.campaigns[ci].clone();
    candidate.negatives = keywords(a.negatives);
    let rules = Rules::new(&ws.input, t.settings.max_ad_groups);
    let found = rules.campaign(&candidate, ws.account.brand_kit.as_ref(), false, "campaign");
    let relevant = found
        .into_iter()
        .filter(|i| i.path.starts_with("campaign.negatives") || i.code == "E09")
        .collect();
    let (errors, warnings) = split(relevant);
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("set_campaign_negatives: {} errors", errors.len()),
        );
    }
    let n = candidate.negatives.len();
    ws.account.campaigns[ci] = candidate;
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(
        json!({"negatives": n}),
        &warnings,
        format!("{n} campaign negatives"),
    )
}

fn build_assets(a: SetAssetsArgs) -> Assets {
    let opt = |s: String| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    Assets {
        sitelinks: a
            .sitelinks
            .into_iter()
            .map(|s| Sitelink {
                text: s.text,
                description1: opt(s.description1),
                description2: opt(s.description2),
                url: s.url,
            })
            .collect(),
        callouts: a.callouts,
        snippets: a
            .snippets
            .into_iter()
            .map(|s| Snippet {
                header: s.header,
                values: s.values,
            })
            .collect(),
    }
}

async fn set_assets(t: &MissionTools, slug: &str, a: SetAssetsArgs) -> ToolOutput {
    let mut guard = t.ws.lock().await;
    let ws: &mut Workspace = &mut guard;
    let Some(ci) = campaign_index(ws, slug) else {
        return not_found(slug);
    };
    let mut candidate = ws.account.campaigns[ci].clone();
    candidate.assets = Some(build_assets(a));
    let rules = Rules::new(&ws.input, t.settings.max_ad_groups);
    let found = rules.campaign(&candidate, ws.account.brand_kit.as_ref(), false, "campaign");
    let (errors, warnings) = split(
        found
            .into_iter()
            .filter(|i| i.path.starts_with("campaign.assets"))
            .collect(),
    );
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("set_assets: {} errors", errors.len()),
        );
    }
    let summary = candidate.assets.as_ref().map_or(String::new(), |a| {
        format!(
            "assets: {} sitelinks, {} callouts, {} snippets",
            a.sitelinks.len(),
            a.callouts.len(),
            a.snippets.len()
        )
    });
    ws.account.campaigns[ci] = candidate;
    if let Err(e) = t.persist(ws) {
        return e;
    }
    ToolOutput::ok(json!({"saved": true}), &warnings, summary)
}

fn campaign_issues(ws: &Workspace, c: &Campaign, max_ad_groups: usize) -> Vec<Issue> {
    let rules = Rules::new(&ws.input, max_ad_groups);
    let mut issues = rules.campaign(c, ws.account.brand_kit.as_ref(), true, "campaign");
    if c.assets.is_none() {
        issues.push(Issue::error("E12", "campaign.assets", "assets are not set"));
    }
    issues
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
    let (errors, warnings) = split(campaign_issues(
        &ws,
        &ws.account.campaigns[ci],
        t.settings.max_ad_groups,
    ));
    if !errors.is_empty() {
        return ToolOutput::fail_issues(
            &errors,
            &warnings,
            format!("finish: {} errors", errors.len()),
        );
    }
    t.mark_finished();
    ToolOutput::ok(
        json!({"ad_groups": ws.account.campaigns[ci].ad_groups.len()}),
        &warnings,
        "campaign finished",
    )
}
