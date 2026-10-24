//! Account diff between a run and the run it came from.
//!
//! A human revision (`origin: "human"`) and an optimize run (`origin: "optimize"`)
//! both point at a parent through `run.json`'s `parent_run`. This module compares
//! the two `workspace.json` accounts and renders the result at `/runs/{id}/diff`.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use askama::Template;
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use mads_core::{
    google::{Account, AdGroup, Campaign, Intent, Keyword, MatchType},
    workspace::Workspace,
};

use crate::serve::{
    AppState, store,
    views::{BootstrapPage, Chrome, ErrorPage, Html},
};

// Field labels. These follow the Portuguese account UI and are not translation keys.
const CAMPAIGN: &str = "Campanha";
const GROUP: &str = "Grupo";
const BUDGET: &str = "Orçamento diário";
const BID: &str = "Estratégia de lance";
const RATIONALE: &str = "Justificativa";
const INTENT: &str = "Intenção";
const KIND: &str = "Tipo";
const CPC: &str = "CPC padrão";
const FINAL_URL: &str = "URL final";
const CPC_RATIONALE: &str = "Justificativa do CPC";
const KEYWORD_ADDED: &str = "Palavra-chave adicionada";
const KEYWORD_REMOVED: &str = "Palavra-chave removida";
const NEGATIVE_ADDED: &str = "Negativa adicionada";
const NEGATIVE_REMOVED: &str = "Negativa removida";
const HEADLINE_ADDED: &str = "Título adicionado";
const HEADLINE_REMOVED: &str = "Título removido";
const DESCRIPTION_ADDED: &str = "Descrição adicionada";
const DESCRIPTION_REMOVED: &str = "Descrição removida";
const PATH1: &str = "Caminho 1";
const PATH2: &str = "Caminho 2";

/// One line of the comparison table. An empty side renders as a dash in the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRow {
    pub scope: String,
    pub field: String,
    pub before: String,
    pub after: String,
}

/// Pure comparison of two accounts. A revision only changes texts, budgets and
/// bids, so the walk covers the fields a human or `mads optimize` can touch.
pub fn changes(before: &Account, after: &Account) -> Vec<ChangeRow> {
    let mut rows = Vec::new();
    let before_campaigns = campaigns_by_name(&before.campaigns);
    let after_campaigns = campaigns_by_name(&after.campaigns);
    let names: BTreeSet<&str> = before_campaigns
        .keys()
        .chain(after_campaigns.keys())
        .copied()
        .collect();
    for name in names {
        let scope = campaign_scope(name);
        match (before_campaigns.get(name), after_campaigns.get(name)) {
            (None, Some(_)) => field(&mut rows, &scope, CAMPAIGN, "", "adicionada"),
            (Some(_), None) => field(&mut rows, &scope, CAMPAIGN, "removida", ""),
            (Some(old), Some(new)) => compare_campaign(&mut rows, &scope, old, new),
            (None, None) => {}
        }
    }
    rows
}

fn campaigns_by_name(campaigns: &[Campaign]) -> BTreeMap<&str, &Campaign> {
    campaigns.iter().map(|c| (c.name.as_str(), c)).collect()
}

fn groups_by_name(groups: &[AdGroup]) -> BTreeMap<&str, &AdGroup> {
    groups.iter().map(|g| (g.name.as_str(), g)).collect()
}

fn campaign_scope(name: &str) -> String {
    format!("Campanha: {name}")
}

fn group_scope(campaign: &str, group: &str) -> String {
    format!("Grupo: {group} · Campanha: {campaign}")
}

fn field(
    rows: &mut Vec<ChangeRow>,
    scope: &str,
    name: &str,
    before: impl Into<String>,
    after: impl Into<String>,
) {
    let before = before.into();
    let after = after.into();
    if before != after {
        rows.push(ChangeRow {
            scope: scope.to_string(),
            field: name.to_string(),
            before,
            after,
        });
    }
}

fn compare_campaign(rows: &mut Vec<ChangeRow>, scope: &str, before: &Campaign, after: &Campaign) {
    field(
        rows,
        scope,
        BUDGET,
        before.daily_budget.format_budget(true),
        after.daily_budget.format_budget(true),
    );
    field(
        rows,
        scope,
        BID,
        before.bid_strategy.label(),
        after.bid_strategy.label(),
    );
    field(
        rows,
        scope,
        RATIONALE,
        before.rationale.as_str(),
        after.rationale.as_str(),
    );
    field(
        rows,
        scope,
        INTENT,
        intent_label(before.intent),
        intent_label(after.intent),
    );
    field(rows, scope, KIND, before.kind.label(), after.kind.label());
    compare_keywords(
        rows,
        scope,
        &before.negatives,
        &after.negatives,
        NEGATIVE_ADDED,
        NEGATIVE_REMOVED,
    );
    compare_groups(rows, &before.name, &before.ad_groups, &after.ad_groups);
}

fn compare_groups(
    rows: &mut Vec<ChangeRow>,
    campaign: &str,
    before: &[AdGroup],
    after: &[AdGroup],
) {
    let before_groups = groups_by_name(before);
    let after_groups = groups_by_name(after);
    let names: BTreeSet<&str> = before_groups
        .keys()
        .chain(after_groups.keys())
        .copied()
        .collect();
    for name in names {
        let scope = group_scope(campaign, name);
        match (before_groups.get(name), after_groups.get(name)) {
            (None, Some(_)) => field(rows, &scope, GROUP, "", "adicionado"),
            (Some(_), None) => field(rows, &scope, GROUP, "removido", ""),
            (Some(old), Some(new)) => compare_group(rows, &scope, old, new),
            (None, None) => {}
        }
    }
}

fn compare_group(rows: &mut Vec<ChangeRow>, scope: &str, before: &AdGroup, after: &AdGroup) {
    field(
        rows,
        scope,
        CPC,
        before.default_cpc.format_cpc(true),
        after.default_cpc.format_cpc(true),
    );
    field(
        rows,
        scope,
        FINAL_URL,
        before.final_url.as_str(),
        after.final_url.as_str(),
    );
    field(
        rows,
        scope,
        CPC_RATIONALE,
        before.cpc_rationale.as_str(),
        after.cpc_rationale.as_str(),
    );
    compare_keywords(
        rows,
        scope,
        &before.keywords,
        &after.keywords,
        KEYWORD_ADDED,
        KEYWORD_REMOVED,
    );
    compare_keywords(
        rows,
        scope,
        &before.negatives,
        &after.negatives,
        NEGATIVE_ADDED,
        NEGATIVE_REMOVED,
    );
    compare_texts(
        rows,
        scope,
        &before.rsa.headlines,
        &after.rsa.headlines,
        HEADLINE_ADDED,
        HEADLINE_REMOVED,
    );
    compare_texts(
        rows,
        scope,
        &before.rsa.descriptions,
        &after.rsa.descriptions,
        DESCRIPTION_ADDED,
        DESCRIPTION_REMOVED,
    );
    field(
        rows,
        scope,
        PATH1,
        before.rsa.path1.as_deref().unwrap_or_default(),
        after.rsa.path1.as_deref().unwrap_or_default(),
    );
    field(
        rows,
        scope,
        PATH2,
        before.rsa.path2.as_deref().unwrap_or_default(),
        after.rsa.path2.as_deref().unwrap_or_default(),
    );
}

fn compare_keywords(
    rows: &mut Vec<ChangeRow>,
    scope: &str,
    before: &[Keyword],
    after: &[Keyword],
    added_field: &str,
    removed_field: &str,
) {
    let before_items = keyword_items(before);
    let after_items = keyword_items(after);
    for item in &after_items {
        if !before_items.contains(item) {
            field(rows, scope, added_field, "", keyword_label(item));
        }
    }
    for item in &before_items {
        if !after_items.contains(item) {
            field(rows, scope, removed_field, keyword_label(item), "");
        }
    }
}

fn compare_texts(
    rows: &mut Vec<ChangeRow>,
    scope: &str,
    before: &[String],
    after: &[String],
    added_field: &str,
    removed_field: &str,
) {
    let before_items = sorted_texts(before);
    let after_items = sorted_texts(after);
    for text in &after_items {
        if !before_items.contains(text) {
            field(rows, scope, added_field, "", text.as_str());
        }
    }
    for text in &before_items {
        if !after_items.contains(text) {
            field(rows, scope, removed_field, text.as_str(), "");
        }
    }
}

fn sorted_texts(texts: &[String]) -> Vec<String> {
    let mut items = texts.to_vec();
    items.sort();
    items.dedup();
    items
}

fn keyword_items(keywords: &[Keyword]) -> Vec<(String, MatchType)> {
    let mut items: Vec<(String, MatchType)> = keywords
        .iter()
        .map(|keyword| (keyword.text.clone(), keyword.match_type))
        .collect();
    items.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| match_rank(a.1).cmp(&match_rank(b.1)))
    });
    items.dedup();
    items
}

fn keyword_label((text, match_type): &(String, MatchType)) -> String {
    format!("{text} ({})", match_label(*match_type))
}

fn match_rank(match_type: MatchType) -> u8 {
    match match_type {
        MatchType::Phrase => 0,
        MatchType::Exact => 1,
    }
}

fn match_label(match_type: MatchType) -> &'static str {
    match match_type {
        MatchType::Phrase => "frase",
        MatchType::Exact => "exata",
    }
}

fn intent_label(intent: Intent) -> &'static str {
    match intent {
        Intent::Brand => "marca",
        Intent::Catalog => "catálogo",
        Intent::Generic => "genérica",
        Intent::Competitor => "concorrente",
    }
}

#[derive(Template)]
#[template(path = "diff.html")]
struct DiffPage {
    chrome: Chrome,
    title: String,
    run: store::RunDetail,
    not_revision: bool,
    missing_parent: bool,
    error_text: String,
    parent_id: String,
    changes: Vec<ChangeRow>,
}

/// GET `/runs/{id}/diff`. Merged by `serve::app`.
pub fn router() -> axum::Router<crate::serve::AppState> {
    Router::new().route("/runs/{id}/diff", get(page))
}

async fn page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(identity) = state.auth.authenticate(&headers) else {
        return bootstrap(&headers);
    };
    let Some((_, project_name, run)) = store::find_run(state.root(), &id) else {
        return not_found_page(&identity, "Execução não encontrada");
    };
    let chrome = Chrome::new(identity.csrf.clone(), "diff", &identity.lang)
        .with_project(&project_name)
        .with_run(&run.summary.id);
    let title = format!("Alterações {}", run.summary.id);
    let parent_id = run.summary.parent.clone();

    let mut not_revision = false;
    let mut missing_parent = false;
    let mut error_text = String::new();
    let mut rows = Vec::new();

    if !run.summary.has_parent {
        not_revision = true;
    } else if let Some(parent_dir) = sibling_dir(&run, &parent_id)
        && parent_dir.is_dir()
    {
        match Workspace::load(&parent_dir.join("workspace.json")) {
            Ok(parent) => match Workspace::load(&run.dir.join("workspace.json")) {
                Ok(current) => rows = changes(&parent.account, &current.account),
                Err(error) => {
                    error_text = format!("Não foi possível ler a conta desta execução: {error}")
                }
            },
            Err(error) => {
                error_text = format!("Não foi possível ler a conta de {parent_id}: {error}");
            }
        }
    } else {
        missing_parent = true;
    }

    Html(DiffPage {
        chrome,
        title,
        run,
        not_revision,
        missing_parent,
        error_text,
        parent_id,
        changes: rows,
    })
    .into_response()
}

/// Sibling run directory in the same project `out/`, refusing anything that is
/// not a single run id segment.
fn sibling_dir(run: &store::RunDetail, parent_id: &str) -> Option<PathBuf> {
    if parent_id.is_empty()
        || parent_id.starts_with('.')
        || !parent_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    Some(run.dir.parent()?.join(parent_id))
}

fn bootstrap(headers: &HeaderMap) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Html(BootstrapPage {
            title: "mads studio".into(),
            lang: crate::serve::i18n::lang_of(headers),
        }),
    )
        .into_response()
}

fn not_found_page(identity: &crate::serve::auth::Identity, message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(ErrorPage {
            chrome: Chrome::new(identity.csrf.clone(), "not-found", &identity.lang),
            title: "Não encontrado".into(),
            message: message.into(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use mads_core::google::{BidStrategy, CampaignKind, Cents, Rsa};

    use super::*;

    fn group() -> AdGroup {
        AdGroup {
            name: "marca".into(),
            default_cpc: Cents(150),
            cpc_rationale: "estimativa".into(),
            final_url: "https://acme.com/".into(),
            keywords: vec![Keyword {
                text: "acme".into(),
                match_type: MatchType::Exact,
            }],
            negatives: Vec::new(),
            rsa: Rsa {
                headlines: vec!["Acme".into()],
                descriptions: vec!["Compre na Acme".into()],
                path1: Some("acme".into()),
                path2: None,
            },
        }
    }

    fn campaign() -> Campaign {
        Campaign {
            name: "Acme - Marca".into(),
            slug: "acme-marca".into(),
            kind: CampaignKind::Search,
            intent: Intent::Brand,
            daily_budget: Cents(5000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: "protege a marca".into(),
            planned_ad_groups: Vec::new(),
            ad_groups: vec![group()],
            asset_groups: Vec::new(),
            negatives: vec![Keyword {
                text: "grátis".into(),
                match_type: MatchType::Phrase,
            }],
            assets: None,
        }
    }

    fn account(campaign: Campaign) -> Account {
        Account {
            brand_kit: None,
            campaigns: vec![campaign],
        }
    }

    #[test]
    fn identical_accounts_have_no_changes() {
        let account = account(campaign());
        assert!(changes(&account, &account).is_empty());
    }

    #[test]
    fn budget_bids_and_texts_are_compared() {
        let before = account(campaign());
        let mut after = account(campaign());
        after.campaigns[0].daily_budget = Cents(6000);
        after.campaigns[0].bid_strategy = BidStrategy::MaximizeClicks { max_cpc: None };
        after.campaigns[0].ad_groups[0].default_cpc = Cents(250);
        after.campaigns[0].ad_groups[0].keywords.push(Keyword {
            text: "acme oficial".into(),
            match_type: MatchType::Phrase,
        });
        after.campaigns[0].ad_groups[0]
            .rsa
            .headlines
            .push("Acme Oficial".into());
        after.campaigns[0].ad_groups[0].rsa.path2 = Some("loja".into());

        let rows = changes(&before, &after);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].field, BUDGET);
        assert_eq!(
            (rows[0].before.as_str(), rows[0].after.as_str()),
            ("50", "60")
        );
        assert_eq!(rows[1].field, BID);
        assert_eq!(
            (rows[1].before.as_str(), rows[1].after.as_str()),
            ("Manual CPC", "Maximize clicks")
        );
        assert_eq!(rows[2].field, CPC);
        assert_eq!(
            (rows[2].before.as_str(), rows[2].after.as_str()),
            ("1,50", "2,50")
        );
        assert_eq!(rows[3].field, KEYWORD_ADDED);
        assert_eq!(rows[3].after, "acme oficial (frase)");
        assert!(rows[3].before.is_empty());
        assert!(
            rows.iter()
                .any(|row| row.field == HEADLINE_ADDED && row.after == "Acme Oficial")
        );
        assert!(
            rows.iter()
                .any(|row| row.field == PATH2 && row.before.is_empty() && row.after == "loja")
        );
    }

    #[test]
    fn a_replaced_keyword_match_type_is_removed_and_added() {
        let before = account(campaign());
        let mut after = account(campaign());
        after.campaigns[0].ad_groups[0].keywords = vec![Keyword {
            text: "acme".into(),
            match_type: MatchType::Phrase,
        }];

        let rows = changes(&before, &after);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.field == KEYWORD_REMOVED
            && row.before == "acme (exata)"
            && row.after.is_empty()));
        assert!(rows.iter().any(|row| row.field == KEYWORD_ADDED
            && row.before.is_empty()
            && row.after == "acme (frase)"));
    }

    #[test]
    fn added_and_removed_campaigns_are_reported() {
        let before = account(campaign());
        let removed = changes(&before, &Account::default());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].field, CAMPAIGN);
        assert_eq!(removed[0].scope, "Campanha: Acme - Marca");
        assert_eq!(removed[0].before, "removida");
        assert!(removed[0].after.is_empty());

        let added = changes(&Account::default(), &before);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].field, CAMPAIGN);
        assert_eq!(added[0].after, "adicionada");
        assert!(added[0].before.is_empty());
    }

    #[test]
    fn added_and_removed_groups_are_reported() {
        let before = account(campaign());
        let mut after = account(campaign());
        after.campaigns[0].ad_groups.clear();

        let rows = changes(&before, &after);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, GROUP);
        assert_eq!(rows[0].scope, "Grupo: marca · Campanha: Acme - Marca");
        assert_eq!(rows[0].before, "removido");
        assert!(rows[0].after.is_empty());

        let rows = changes(&after, &before);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, GROUP);
        assert_eq!(rows[0].after, "adicionado");
    }

    #[test]
    fn the_output_order_is_deterministic() {
        let before = account(campaign());
        let mut after = account(campaign());
        after.campaigns[0].ad_groups[0].final_url = "https://acme.com/loja".into();
        after.campaigns[0].negatives.push(Keyword {
            text: "barato".into(),
            match_type: MatchType::Exact,
        });
        assert_eq!(changes(&before, &after), changes(&before, &after));
    }
}
