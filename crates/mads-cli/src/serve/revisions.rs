use std::{fs, path::Path};

use mads_core::{
    google::{Account, Issue, Rules},
    input::Input,
    run::RunDir,
    workspace::Workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    pub path: String,
    pub value: Value,
}

/// What one edit changed, recorded in `edits.ndjson` so the UI can diff and undo.
#[derive(Debug, Clone, Serialize)]
pub struct RecordedEdit {
    pub path: String,
    pub before: Value,
    pub after: Value,
}

#[derive(Debug)]
pub enum RevisionError {
    /// Another job is running or queued for the project; handlers answer 409.
    Conflict,
    Message(String),
    Invalid(Vec<Issue>),
}

impl RevisionError {
    pub fn message(self) -> String {
        match self {
            RevisionError::Conflict => "este projeto já tem uma execução em andamento".into(),
            RevisionError::Message(message) => message,
            RevisionError::Invalid(issues) => issues
                .first()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .unwrap_or_else(|| "conta inválida".into()),
        }
    }
}

/// Applies edits to a copy of the account and validates it with the real rules.
/// Returns the edited account plus a before/after record of every edit.
pub fn apply(
    account: &Account,
    input: &Input,
    edits: &[Edit],
    max_ad_groups: usize,
) -> Result<(Account, Vec<RecordedEdit>), RevisionError> {
    if edits.is_empty() {
        return Err(RevisionError::Message("nenhuma edição enviada".into()));
    }
    let mut value = serde_json::to_value(account)
        .map_err(|e| RevisionError::Message(format!("conta inválida: {e}")))?;
    let mut records = Vec::with_capacity(edits.len());
    for edit in edits {
        let segments = parse_path(&edit.path)?;
        ensure_editable(&edit.path, &segments)?;
        let normalized = normalize_value(&edit.path, &edit.value)?;
        let before = get_path(&value, &segments).cloned().unwrap_or(Value::Null);
        set_path(&mut value, &edit.path, normalized.clone())?;
        records.push(RecordedEdit {
            path: edit.path.clone(),
            before,
            after: normalized,
        });
    }
    let edited: Account = serde_json::from_value(value)
        .map_err(|e| RevisionError::Message(format!("edição inválida: {e}")))?;
    let issues = Rules::new(input, max_ad_groups).account(&edited, false);
    let errors: Vec<Issue> = issues.into_iter().filter(Issue::is_error).collect();
    if errors.is_empty() {
        Ok((edited, records))
    } else {
        Err(RevisionError::Invalid(errors))
    }
}

/// V1 edits values only: text, budgets, CPCs, URLs, keywords, negatives and
/// assets. Adding or removing campaigns, groups or fields is a new run. The
/// path is flattened with `#` for indexes and matched against that shape.
fn ensure_editable(path: &str, segments: &[Segment]) -> Result<(), RevisionError> {
    let signature = segments
        .iter()
        .map(|segment| match segment {
            Segment::Key(key) => key.as_str().to_string(),
            Segment::Index(_) => "#".to_string(),
        })
        .collect::<Vec<_>>()
        .join(".");
    let allowed = matches!(
        signature.as_str(),
        "campaigns.#.rationale"
            | "campaigns.#.daily_budget"
            | "campaigns.#.negatives.#.text"
            | "campaigns.#.ad_groups.#.default_cpc"
            | "campaigns.#.ad_groups.#.final_url"
            | "campaigns.#.ad_groups.#.keywords.#.text"
            | "campaigns.#.ad_groups.#.negatives.#.text"
            | "campaigns.#.ad_groups.#.rsa.path1"
            | "campaigns.#.ad_groups.#.rsa.path2"
            | "campaigns.#.ad_groups.#.rsa.headlines.#"
            | "campaigns.#.ad_groups.#.rsa.descriptions.#"
            | "campaigns.#.asset_groups.#.business_name"
            | "campaigns.#.asset_groups.#.final_url"
            | "campaigns.#.asset_groups.#.headlines.#"
            | "campaigns.#.asset_groups.#.long_headlines.#"
            | "campaigns.#.asset_groups.#.descriptions.#"
            | "campaigns.#.asset_groups.#.search_themes.#"
            | "campaigns.#.assets.sitelinks.#.text"
            | "campaigns.#.assets.sitelinks.#.url"
            | "campaigns.#.assets.sitelinks.#.description1"
            | "campaigns.#.assets.sitelinks.#.description2"
            | "campaigns.#.assets.callouts.#"
            | "campaigns.#.assets.snippets.#.values.#"
    );
    if allowed {
        Ok(())
    } else {
        Err(RevisionError::Message(format!(
            "{path} não pode ser editado; mudanças de estrutura precisam de outra execução"
        )))
    }
}

fn get_path<'a>(root: &'a Value, segments: &[Segment]) -> Option<&'a Value> {
    let mut current = root;
    for segment in segments {
        current = match segment {
            Segment::Key(key) => current.get(key)?,
            Segment::Index(index) => current.get(*index)?,
        };
    }
    Some(current)
}

/// Creates a sibling run `<base>-rN` and returns its id and revision number.
pub fn create(
    project_dir: &Path,
    base_run_id: &str,
    workspace: &Workspace,
    records: &[RecordedEdit],
) -> Result<(String, u32), String> {
    let base = project_dir.join("out").join(base_run_id);
    if !base.is_dir() {
        return Err("execução base não encontrada".into());
    }
    let previous: Value = fs::read_to_string(base.join("run.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let mut revision = 1u32;
    let mut id = format!("{base_run_id}-r{revision}");
    while project_dir.join("out").join(&id).exists() {
        revision += 1;
        if revision > 99 {
            return Err("limite de revisões atingido".into());
        }
        id = format!("{base_run_id}-r{revision}");
    }
    let target = project_dir.join("out").join(&id);
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    for folder in ["input", "transcripts", "google-ads/editor/images"] {
        // Images cost money: hard link them when the filesystem allows, so a
        // revision does not duplicate every picture.
        link_dir(&base.join(folder), &target.join(folder))?;
    }
    let run = RunDir::open(&target).map_err(|e| e.to_string())?;
    workspace
        .save(&run.workspace_path())
        .map_err(|e| e.to_string())?;
    let layout = previous
        .get("layout")
        .and_then(Value::as_str)
        .unwrap_or("bulk");
    let provider = previous
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let model = previous.get("model").cloned().unwrap_or(Value::Null);
    let meta = json!({
        "version": 1,
        "run_id": id.clone(),
        "provider": provider,
        "model": model,
        "layout": layout,
        "status": "incomplete",
        "exit_code": 1,
        "totals": {
            "input_tokens": 0,
            "output_tokens": 0,
            "cost_usd": null,
            "missions": workspace.missions.len(),
            "failed_missions": 0
        },
        "origin": "human",
        "parent_run": base_run_id,
        "revision": revision,
        "edits": records.len()
    });
    crate::serve::project_admin::write_atomic(
        &run.run_json_path(),
        &serde_json::to_vec_pretty(&meta).map_err(|e| e.to_string())?,
    )?;
    let ts = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let audit: Vec<String> = records
        .iter()
        .map(|record| {
            serde_json::to_string(&json!({
                "ts": ts,
                "path": record.path,
                "before": record.before,
                "after": record.after,
            }))
            .unwrap_or_default()
        })
        .collect();
    let audit_dir = target.join(".mads");
    fs::create_dir_all(&audit_dir).map_err(|e| e.to_string())?;
    fs::write(audit_dir.join("edits.ndjson"), audit.join("\n")).map_err(|e| e.to_string())?;
    Ok((id, revision))
}

fn normalize_value(path: &str, value: &Value) -> Result<Value, RevisionError> {
    if path.ends_with("daily_budget") || path.ends_with("default_cpc") {
        let number = value.as_f64().ok_or_else(|| {
            RevisionError::Message(format!("{path}: o valor precisa ser um número"))
        })?;
        let cents = mads_core::money::Cents::from_f64(number).ok_or_else(|| {
            RevisionError::Message(format!("{path}: maior que zero, com até 2 decimais"))
        })?;
        return Ok(Value::from(cents.0));
    }
    Ok(value.clone())
}

#[derive(Debug)]
enum Segment {
    Key(String),
    Index(usize),
}

fn parse_path(path: &str) -> Result<Vec<Segment>, RevisionError> {
    if path.is_empty() || path.contains("..") {
        return Err(RevisionError::Message("caminho inválido".into()));
    }
    let mut segments = Vec::new();
    for part in path.split('.') {
        let mut name = part;
        let mut indices = Vec::new();
        if let Some(open) = part.find('[') {
            name = &part[..open];
            let mut rest = &part[open..];
            while let Some(close) = rest.find(']') {
                let index = rest[1..close]
                    .parse::<usize>()
                    .map_err(|_| RevisionError::Message(format!("índice inválido em {path}")))?;
                indices.push(Segment::Index(index));
                rest = &rest[close + 1..];
                if rest.is_empty() || !rest.starts_with('[') {
                    break;
                }
            }
        }
        if name.is_empty() {
            return Err(RevisionError::Message(format!("caminho inválido: {path}")));
        }
        segments.push(Segment::Key(name.to_string()));
        segments.append(&mut indices);
    }
    if segments.is_empty() {
        return Err(RevisionError::Message("caminho inválido".into()));
    }
    Ok(segments)
}

fn set_path(root: &mut Value, path: &str, value: Value) -> Result<(), RevisionError> {
    let segments = parse_path(path)?;
    let (last, parents) = segments
        .split_last()
        .ok_or_else(|| RevisionError::Message("caminho inválido".into()))?;
    let mut current = root;
    for segment in parents {
        current = match segment {
            Segment::Key(key) => current
                .get_mut(key)
                .ok_or_else(|| RevisionError::Message(format!("campo desconhecido em {path}")))?,
            Segment::Index(index) => current
                .get_mut(*index)
                .ok_or_else(|| RevisionError::Message(format!("índice fora de faixa em {path}")))?,
        };
    }
    match last {
        Segment::Key(key) => {
            let object = current
                .as_object_mut()
                .ok_or_else(|| RevisionError::Message(format!("{path} não é um objeto")))?;
            if !object.contains_key(key) {
                return Err(RevisionError::Message(format!(
                    "campo desconhecido: {path}"
                )));
            }
            object.insert(key.clone(), value);
        }
        Segment::Index(index) => {
            let array = current
                .as_array_mut()
                .ok_or_else(|| RevisionError::Message(format!("{path} não é uma lista")))?;
            let slot = array
                .get_mut(*index)
                .ok_or_else(|| RevisionError::Message(format!("índice fora de faixa: {path}")))?;
            *slot = value;
        }
    }
    Ok(())
}

/// Copies a tree, hard linking files when the filesystem allows. A hard link
/// keeps the revision honest (same bytes, no duplicate storage) and falls back
/// to a copy across devices.
fn link_dir(from: &Path, to: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(from) else {
        return Ok(());
    };
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            link_dir(&path, &target)?;
        } else if meta.is_file() && fs::hard_link(&path, &target).is_err() {
            fs::copy(&path, &target)
                .map_err(|e| format!("não foi possível copiar {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use mads_core::{
        google::{Campaign, Cents, Intent},
        input::load_input,
    };

    fn account() -> Account {
        Account {
            brand_kit: None,
            campaigns: vec![Campaign {
                name: "Acme".into(),
                slug: "acme".into(),
                kind: Default::default(),
                intent: Intent::Brand,
                daily_budget: Cents(5000),
                bid_strategy: mads_core::google::BidStrategy::ManualCpc,
                rationale: String::new(),
                planned_ad_groups: Vec::new(),
                ad_groups: Vec::new(),
                asset_groups: Vec::new(),
                negatives: Vec::new(),
                assets: None,
            }],
        }
    }

    #[test]
    fn a_text_edit_reaches_the_nested_field() {
        let dir = tempfile::tempdir().unwrap();
        let business = dir.path().join("business.toml");
        fs::write(
            &business,
            r#"
[business]
name = "Acme"
url = "https://acme.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "vender"
description = "Uma descrição com mais de vinte caracteres para o teste."
[budget]
daily = 50
currency = "BRL"
"#,
        )
        .unwrap();
        let input = load_input(&business).unwrap();
        let account = account();
        let (edited, records) = apply(
            &account,
            &input,
            &[Edit {
                path: "campaigns[0].rationale".into(),
                value: json!("porque sim"),
            }],
            50,
        )
        .unwrap();
        assert_eq!(edited.campaigns[0].rationale, "porque sim");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].before, json!(""));
        assert_eq!(records[0].after, json!("porque sim"));
    }

    #[test]
    fn a_structural_edit_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let business = dir.path().join("business.toml");
        fs::write(
            &business,
            r#"
[business]
name = "Acme"
url = "https://acme.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "vender"
description = "Uma descrição com mais de vinte caracteres para o teste."
[budget]
daily = 50
currency = "BRL"
"#,
        )
        .unwrap();
        let input = load_input(&business).unwrap();
        for path in [
            "campaigns[0].slug",
            "campaigns[0].name",
            "campaigns[0].intent",
            "campaigns[0].ad_groups[0].name",
            "campaigns[0].unknown",
        ] {
            let error = apply(
                &account(),
                &input,
                &[Edit {
                    path: path.into(),
                    value: json!("x"),
                }],
                50,
            )
            .unwrap_err();
            assert!(error.message().contains("não pode ser editado"), "{path}");
        }
    }

    #[test]
    fn a_budget_converts_from_currency_units_to_cents() {
        let dir = tempfile::tempdir().unwrap();
        let business = dir.path().join("business.toml");
        fs::write(
            &business,
            r#"
[business]
name = "Acme"
url = "https://acme.com"
language = "pt-BR"
locations = ["Brazil"]
goal = "vender"
description = "Uma descrição com mais de vinte caracteres para o teste."
[budget]
daily = 50
currency = "BRL"
"#,
        )
        .unwrap();
        let input = load_input(&business).unwrap();
        let (edited, records) = apply(
            &account(),
            &input,
            &[Edit {
                path: "campaigns[0].daily_budget".into(),
                value: json!(50.0),
            }],
            50,
        )
        .unwrap();
        assert_eq!(edited.campaigns[0].daily_budget, Cents(5000));
        assert_eq!(records[0].before, json!(5000));
        assert_eq!(records[0].after, json!(5000));
    }
}
