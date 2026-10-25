use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use mads_core::{
    google::CampaignKind,
    input::{Input, load_input, parse_input_toml},
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DraftView {
    pub valid: bool,
    pub error: String,
    pub name: String,
    pub url: String,
    pub language: String,
    pub location: String,
    pub goal: String,
    pub description: String,
    pub conversion_tracking: bool,
    pub brand_terms: String,
    pub competitors: String,
    pub avoid: String,
    pub restricted_alcohol: bool,
    pub restricted_gambling: bool,
    pub restricted_healthcare: bool,
    pub restricted_financial: bool,
    pub restricted_political: bool,
    pub restricted_sexual: bool,
    pub daily: String,
    pub currency: String,
    pub max_cpc: String,
    pub export_status: String,
    pub url_suffix: String,
    pub eu_political_ads: bool,
    pub decimal_comma: bool,
    pub format_search: bool,
    pub format_performance_max: bool,
    pub format_demand_gen: bool,
    pub catalog_file: String,
    pub catalog_text: String,
    pub catalog_rows: Vec<CatalogRow>,
    pub focus_name: String,
    pub focus_urls: String,
    pub focus_terms: String,
    pub app_store: String,
    pub app_id: String,
    pub google_ads_customer_id: String,
    pub google_ads_tracking_template: String,
    pub google_ads_devices: String,
    pub google_ads_labels: String,
    pub google_ads_app_id: String,
    pub research_preview: String,
    pub design_preview: String,
    pub research_text: String,
    pub design_text: String,
    pub has_logo: bool,
    pub logo_name: String,
    pub toml_preview: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CatalogRow {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub aliases: String,
    pub third_party: String,
    pub notes: String,
    pub image: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DraftPayload {
    pub name: String,
    pub url: String,
    pub language: String,
    pub location: String,
    pub goal: String,
    pub description: String,
    #[serde(default)]
    pub conversion_tracking: bool,
    #[serde(default)]
    pub brand_terms: String,
    #[serde(default)]
    pub competitors: String,
    #[serde(default)]
    pub avoid: String,
    #[serde(default)]
    pub restricted: Vec<String>,
    pub daily: f64,
    pub currency: String,
    #[serde(default)]
    pub max_cpc: Option<f64>,
    pub export_status: String,
    pub url_suffix: String,
    #[serde(default)]
    pub eu_political_ads: bool,
    #[serde(default)]
    pub decimal_comma: bool,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub catalog_text: String,
    #[serde(default)]
    pub focus_name: String,
    #[serde(default)]
    pub focus_urls: String,
    #[serde(default)]
    pub focus_terms: String,
    #[serde(default)]
    pub app_store: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub google_ads_customer_id: String,
    #[serde(default)]
    pub google_ads_tracking_template: String,
    #[serde(default)]
    pub google_ads_devices: String,
    #[serde(default)]
    pub google_ads_labels: String,
    #[serde(default)]
    pub google_ads_app_id: String,
}

pub fn load(project_dir: &Path) -> DraftView {
    let business = project_dir.join("business.toml");
    if !business.is_file() {
        return DraftView {
            error: "business.toml is missing".into(),
            toml_preview: String::new(),
            ..DraftView::default()
        };
    }
    let text = fs::read_to_string(&business).unwrap_or_default();
    let file = parse_input_toml(&text);
    // A hand-edited TOML may point the catalog anywhere: keep the read inside
    // the project, like save does.
    let catalog_file = file
        .as_ref()
        .ok()
        .and_then(|f| f.catalog_file.clone())
        .map(|f| safe_relative(&f))
        .transpose()
        .unwrap_or(None)
        .unwrap_or_else(|| "catalog.csv".into());
    let catalog_text = fs::read_to_string(project_dir.join(&catalog_file)).unwrap_or_default();
    let mut draft = DraftView {
        catalog_file,
        catalog_text,
        toml_preview: text.clone(),
        ..DraftView::default()
    };
    let input = match load_input(&business) {
        Ok(input) => input,
        Err(e) => {
            draft.error = e.to_string();
            return draft;
        }
    };
    draft.valid = true;
    fill(&mut draft, &input, &text);
    draft
}

fn fill(draft: &mut DraftView, input: &Input, toml: &str) {
    let business = &input.business;
    draft.name = business.name.clone();
    draft.url = business.url.clone();
    draft.language = business.language.clone();
    draft.location = business.locations.first().cloned().unwrap_or_default();
    draft.goal = business.goal.clone();
    draft.description = business.description.clone();
    draft.conversion_tracking = business.conversion_tracking;
    draft.brand_terms = business.brand_terms.join(", ");
    draft.competitors = business.competitors.join(", ");
    draft.avoid = business.avoid.join(", ");
    for restricted in &business.restricted {
        match restricted {
            mads_core::input::RestrictedCategory::Alcohol => draft.restricted_alcohol = true,
            mads_core::input::RestrictedCategory::Gambling => draft.restricted_gambling = true,
            mads_core::input::RestrictedCategory::Healthcare => draft.restricted_healthcare = true,
            mads_core::input::RestrictedCategory::FinancialServices => {
                draft.restricted_financial = true;
            }
            mads_core::input::RestrictedCategory::Political => draft.restricted_political = true,
            mads_core::input::RestrictedCategory::SexualContent => draft.restricted_sexual = true,
        }
    }
    draft.daily = format!("{:.2}", input.budget.daily.0 as f64 / 100.0);
    draft.currency = input.budget.currency.clone();
    draft.max_cpc = input
        .budget
        .max_cpc
        .map(|cpc| format!("{:.2}", cpc.0 as f64 / 100.0))
        .unwrap_or_default();
    draft.export_status = match input.export.status {
        mads_core::input::ExportStatus::Paused => "Paused".into(),
        mads_core::input::ExportStatus::Enabled => "Enabled".into(),
    };
    draft.url_suffix = input.export.url_suffix.clone();
    draft.eu_political_ads = input.export.eu_political_ads;
    draft.decimal_comma = input.export.decimal_comma;
    for format in &input.formats {
        match format {
            CampaignKind::Search => draft.format_search = true,
            CampaignKind::PerformanceMax => draft.format_performance_max = true,
            CampaignKind::DemandGen => draft.format_demand_gen = true,
            CampaignKind::AppInstalls => {}
        }
    }
    draft.research_preview = input.research.chars().take(4000).collect();
    draft.design_preview = input.design.chars().take(4000).collect();
    draft.research_text = input.research.clone();
    draft.design_text = input.design.clone();
    draft.has_logo = input.logo.is_some();
    draft.logo_name = input
        .logo
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if let Some(focus) = &input.focus {
        draft.focus_name = focus.name.clone();
        draft.focus_urls = focus.urls.join("\n");
        draft.focus_terms = focus
            .terms
            .iter()
            .map(|group| group.join(", "))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if let Some(app) = &input.app {
        draft.app_store = match app.store {
            mads_core::input::AppStore::GooglePlay => "google_play".into(),
            mads_core::input::AppStore::AppStore => "app_store".into(),
        };
        draft.app_id = app.id.clone();
    }
    draft.google_ads_customer_id = input.google_ads.customer_id.clone();
    draft.google_ads_tracking_template = input.google_ads.tracking_template.clone();
    draft.google_ads_devices = input.google_ads.devices.clone();
    draft.google_ads_labels = input.google_ads.labels.join(", ");
    draft.google_ads_app_id = input.google_ads.app_id.clone();
    draft.catalog_rows = input
        .catalog
        .iter()
        .map(|item| CatalogRow {
            id: item.id.clone(),
            name: item.name.clone(),
            url: item.url.clone(),
            category: item.category.clone(),
            aliases: item.aliases.join(", "),
            third_party: if item.third_party {
                "sim".into()
            } else {
                "não".into()
            },
            notes: item.notes.clone(),
            image: item.image.clone().unwrap_or_default(),
        })
        .collect();
    draft.toml_preview = toml.to_string();
}

pub fn save(project_dir: &Path, payload: &DraftPayload) -> Result<(), String> {
    let business = project_dir.join("business.toml");
    let existing = fs::read_to_string(&business)
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .unwrap_or_default();
    let mut root = existing;
    let business_table = table(&mut root, "business")?;
    insert_string(business_table, "name", &payload.name);
    insert_string(business_table, "url", &payload.url);
    insert_string(business_table, "language", &payload.language);
    insert_string(business_table, "goal", &payload.goal);
    insert_string(business_table, "description", &payload.description);
    business_table.insert(
        "locations".into(),
        toml::Value::Array(vec![toml::Value::String(
            payload.location.trim().to_string(),
        )]),
    );
    insert_bool(
        business_table,
        "conversion_tracking",
        payload.conversion_tracking,
    );
    insert_list(business_table, "brand_terms", &payload.brand_terms);
    insert_list(business_table, "competitors", &payload.competitors);
    insert_list(business_table, "avoid", &payload.avoid);
    if payload.restricted.is_empty() {
        business_table.remove("restricted");
    } else {
        business_table.insert(
            "restricted".into(),
            toml::Value::Array(
                payload
                    .restricted
                    .iter()
                    .map(|value| toml::Value::String(value.clone()))
                    .collect(),
            ),
        );
    }

    let budget = table(&mut root, "budget")?;
    budget.insert("daily".into(), toml::Value::Float(payload.daily));
    insert_string(budget, "currency", &payload.currency.to_uppercase());
    match payload.max_cpc {
        Some(value) => {
            budget.insert("max_cpc".into(), toml::Value::Float(value));
        }
        None => {
            budget.remove("max_cpc");
        }
    }

    let export = table(&mut root, "export")?;
    insert_string(export, "status", &payload.export_status);
    insert_string(export, "url_suffix", &payload.url_suffix);
    insert_bool(export, "eu_political_ads", payload.eu_political_ads);
    insert_bool(export, "decimal_comma", payload.decimal_comma);

    if payload.formats.is_empty() {
        root.remove("campaigns");
    } else {
        let campaigns = table(&mut root, "campaigns")?;
        campaigns.insert(
            "formats".into(),
            toml::Value::Array(
                payload
                    .formats
                    .iter()
                    .map(|value| toml::Value::String(value.clone()))
                    .collect(),
            ),
        );
    }

    let focus_urls: Vec<String> = payload
        .focus_urls
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    let focus_terms: Vec<Vec<String>> = payload
        .focus_terms
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|line| {
            line.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .collect();
    if payload.focus_name.trim().is_empty() {
        root.remove("focus");
    } else {
        let focus = table(&mut root, "focus")?;
        insert_string(focus, "name", &payload.focus_name);
        focus.insert(
            "urls".into(),
            toml::Value::Array(focus_urls.into_iter().map(toml::Value::String).collect()),
        );
        focus.insert(
            "terms".into(),
            toml::Value::Array(
                focus_terms
                    .into_iter()
                    .map(|group| {
                        toml::Value::Array(group.into_iter().map(toml::Value::String).collect())
                    })
                    .collect(),
            ),
        );
    }

    if payload.app_id.trim().is_empty() {
        root.remove("app");
    } else {
        let app = table(&mut root, "app")?;
        let store = if payload.app_store.trim() == "app_store" {
            "app_store"
        } else {
            "google_play"
        };
        insert_string(app, "store", store);
        insert_string(app, "id", &payload.app_id);
    }

    let google_ads = table(&mut root, "google_ads")?;
    insert_string(google_ads, "customer_id", &payload.google_ads_customer_id);
    insert_string(
        google_ads,
        "tracking_template",
        &payload.google_ads_tracking_template,
    );
    insert_string(google_ads, "devices", &payload.google_ads_devices);
    insert_list(google_ads, "labels", &payload.google_ads_labels);
    insert_string(google_ads, "app_id", &payload.google_ads_app_id);

    let catalog_file = if payload.catalog_text.trim().is_empty() {
        root.remove("catalog");
        "catalog.csv".to_string()
    } else {
        let catalog = table(&mut root, "catalog")?;
        let existing = catalog
            .get("file")
            .and_then(toml::Value::as_str)
            .unwrap_or("catalog.csv")
            .to_string();
        let file = safe_relative(&existing)?;
        insert_string(catalog, "file", &file);
        file
    };

    let toml_text = toml::to_string_pretty(&root).map_err(|e| e.to_string())?;
    let staging = StagingGuard::new(project_dir)?;
    copy_project(project_dir, staging.path())?;
    fs::write(staging.path().join("business.toml"), &toml_text).map_err(|e| e.to_string())?;
    if !payload.catalog_text.trim().is_empty() {
        let target = staging.path().join(&catalog_file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(target, payload.catalog_text.as_bytes()).map_err(|e| e.to_string())?;
    }
    match load_input(staging.path().join("business.toml").as_path()) {
        Ok(_) => {}
        Err(e) => return Err(e.to_string()),
    }
    // The catalog moves first and the TOML is the commit point: a crash in
    // between leaves the old TOML with the new catalog, never the reverse.
    if !payload.catalog_text.trim().is_empty() {
        let target = project_dir.join(&catalog_file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        write_atomic(&target, payload.catalog_text.as_bytes())?;
    }
    write_atomic(&business, toml_text.as_bytes())
}

pub fn save_research(project_dir: &Path, text: &str) -> Result<(), String> {
    save_text(project_dir, "research", "research.md", text)
}

pub fn save_design(project_dir: &Path, text: &str) -> Result<(), String> {
    save_text(project_dir, "design", "DESIGN.md", text)
}

fn save_text(
    project_dir: &Path,
    table_name: &str,
    default_file: &str,
    text: &str,
) -> Result<(), String> {
    let business = project_dir.join("business.toml");
    let existing = fs::read_to_string(&business)
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .unwrap_or_default();
    let mut root = existing;
    let table = table(&mut root, table_name)?;
    let file = table
        .get("file")
        .and_then(toml::Value::as_str)
        .map(safe_relative)
        .transpose()?
        .unwrap_or_else(|| default_file.to_string());
    insert_string(table, "file", &file);
    let toml_text = toml::to_string_pretty(&root).map_err(|e| e.to_string())?;
    let staging = StagingGuard::new(project_dir)?;
    copy_project(project_dir, staging.path())?;
    let staged_toml = staging.path().join("business.toml");
    fs::write(&staged_toml, &toml_text).map_err(|e| e.to_string())?;
    let staged_file = staging.path().join(&file);
    if let Some(parent) = staged_file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&staged_file, text.as_bytes()).map_err(|e| e.to_string())?;
    if let Err(e) = load_input(&staged_toml) {
        return Err(e.to_string());
    }
    // Asset first, TOML last: the TOML rename is the commit point.
    let target = project_dir.join(&file);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    write_atomic(&target, text.as_bytes())?;
    write_atomic(&business, toml_text.as_bytes())
}

pub fn save_logo(project_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    let png = mads_core::images::prepare_logo(bytes).map_err(|e| e.to_string())?;
    let business = project_dir.join("business.toml");
    let existing = fs::read_to_string(&business)
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .unwrap_or_default();
    let mut root = existing;
    let brand = table(&mut root, "brand")?;
    insert_string(brand, "logo", "brand/logo.png");
    let toml_text = toml::to_string_pretty(&root).map_err(|e| e.to_string())?;
    let staging = StagingGuard::new(project_dir)?;
    copy_project(project_dir, staging.path())?;
    let staged_toml = staging.path().join("business.toml");
    fs::write(&staged_toml, &toml_text).map_err(|e| e.to_string())?;
    let staged_logo = staging.path().join("brand/logo.png");
    if let Some(parent) = staged_logo.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&staged_logo, &png).map_err(|e| e.to_string())?;
    if let Err(e) = load_input(&staged_toml) {
        return Err(e.to_string());
    }
    write_atomic(&business, toml_text.as_bytes())?;
    let target = project_dir.join("brand/logo.png");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    write_atomic(&target, &png)?;
    Ok(())
}

fn table<'a>(root: &'a mut toml::Table, key: &str) -> Result<&'a mut toml::Table, String> {
    if !root.get(key).is_some_and(toml::Value::is_table) {
        root.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    root.get_mut(key)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| format!("[{key}] não é uma tabela"))
}

fn insert_string(table: &mut toml::Table, key: &str, value: &str) {
    if value.trim().is_empty() {
        table.remove(key);
    } else {
        table.insert(key.into(), toml::Value::String(value.trim().to_string()));
    }
}

fn insert_bool(table: &mut toml::Table, key: &str, value: bool) {
    table.insert(key.into(), toml::Value::Boolean(value));
}

fn insert_list(table: &mut toml::Table, key: &str, value: &str) {
    let values: Vec<toml::Value> = value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| toml::Value::String(part.to_string()))
        .collect();
    if values.is_empty() {
        table.remove(key);
    } else {
        table.insert(key.into(), toml::Value::Array(values));
    }
}

/// Removes the staging directory unless the save completed. A failed write in
/// the middle of `save` must not leak a full project copy.
struct StagingGuard(PathBuf);

impl StagingGuard {
    fn new(project_dir: &Path) -> Result<Self, String> {
        let dir = project_dir
            .join(".mads")
            .join("staging")
            .join(Uuid::new_v4().simple().to_string());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn safe_relative(value: &str) -> Result<String, String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("o caminho do catálogo precisa ser relativo e simples".into());
    }
    Ok(value.to_string())
}

fn copy_project(from: &Path, to: &Path) -> Result<(), String> {
    let entries = fs::read_dir(from).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "out" || name_str == ".mads" || name_str == ".git" {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            copy_project(&source, &target)?;
        } else if meta.is_file() {
            fs::copy(&source, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    // A unique temp name: two concurrent saves must not share one temp file.
    let tmp = path.with_extension(format!("tmp.{}", Uuid::new_v4().simple()));
    let result = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUSINESS: &str = r#"
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
[catalog]
file = "catalog.csv"
"#;

    #[test]
    fn load_fills_the_form_from_the_real_parser() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("business.toml"), BUSINESS).unwrap();
        fs::write(
            dir.path().join("catalog.csv"),
            "name,url\nMalbec,https://acme.com/malbec\n",
        )
        .unwrap();
        let view = load(dir.path());
        assert!(view.valid, "{}", view.error);
        assert_eq!(view.name, "Acme");
        assert_eq!(view.location, "Brazil");
        assert_eq!(view.daily, "50.00");
        assert!(view.catalog_text.contains("Malbec"));
    }

    #[test]
    fn save_round_trips_a_business_field() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("business.toml"), BUSINESS).unwrap();
        fs::write(dir.path().join("catalog.csv"), "name,url\n").unwrap();
        let mut payload = DraftPayload {
            name: "Acme".into(),
            url: "https://acme.com".into(),
            language: "pt-BR".into(),
            location: "Brazil".into(),
            goal: "vender mais".into(),
            description: "Uma descrição com mais de vinte caracteres para o teste.".into(),
            conversion_tracking: false,
            brand_terms: String::new(),
            competitors: String::new(),
            avoid: String::new(),
            restricted: Vec::new(),
            daily: 50.0,
            currency: "BRL".into(),
            max_cpc: None,
            export_status: "Paused".into(),
            url_suffix: String::new(),
            eu_political_ads: false,
            decimal_comma: true,
            formats: Vec::new(),
            catalog_text: "name,url\n".into(),
            ..DraftPayload::default()
        };
        save(dir.path(), &payload).unwrap();
        let text = fs::read_to_string(dir.path().join("business.toml")).unwrap();
        assert!(text.contains("vender mais"));
        payload.goal = "outro objetivo".into();
        save(dir.path(), &payload).unwrap();
        let text = fs::read_to_string(dir.path().join("business.toml")).unwrap();
        assert!(text.contains("outro objetivo"));
    }

    #[test]
    fn an_empty_name_is_rejected_by_the_real_parser() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("business.toml"), BUSINESS).unwrap();
        let payload = DraftPayload {
            name: String::new(),
            url: "https://acme.com".into(),
            language: "pt-BR".into(),
            location: "Brazil".into(),
            goal: "vender".into(),
            description: "Uma descrição com mais de vinte caracteres para o teste.".into(),
            conversion_tracking: false,
            brand_terms: String::new(),
            competitors: String::new(),
            avoid: String::new(),
            restricted: Vec::new(),
            daily: 50.0,
            currency: "BRL".into(),
            max_cpc: None,
            export_status: "Paused".into(),
            url_suffix: String::new(),
            eu_political_ads: false,
            decimal_comma: true,
            formats: Vec::new(),
            catalog_text: String::new(),
            ..DraftPayload::default()
        };
        assert!(save(dir.path(), &payload).is_err());
    }

    #[test]
    fn advanced_sections_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("business.toml"), BUSINESS).unwrap();
        fs::write(dir.path().join("catalog.csv"), "name,url\n").unwrap();
        let payload = DraftPayload {
            name: "Acme".into(),
            url: "https://acme.com".into(),
            language: "pt-BR".into(),
            location: "Brazil".into(),
            goal: "vender".into(),
            description: "Uma descrição com mais de vinte caracteres para o teste.".into(),
            conversion_tracking: false,
            brand_terms: String::new(),
            competitors: String::new(),
            avoid: String::new(),
            restricted: Vec::new(),
            daily: 50.0,
            currency: "BRL".into(),
            max_cpc: None,
            export_status: "Paused".into(),
            url_suffix: String::new(),
            eu_political_ads: false,
            decimal_comma: true,
            formats: Vec::new(),
            catalog_text: "name,url\n".into(),
            focus_name: "Vinhos".into(),
            focus_urls: "https://acme.com/vinhos".into(),
            focus_terms: "vinho, tinto\nmalbec, argentino".into(),
            app_store: "google_play".into(),
            app_id: "com.acme.app".into(),
            google_ads_customer_id: String::new(),
            google_ads_tracking_template: String::new(),
            google_ads_devices: "Mobile".into(),
            google_ads_labels: "estrutural".into(),
            google_ads_app_id: String::new(),
        };
        save(dir.path(), &payload).unwrap();
        let view = load(dir.path());
        assert!(view.valid, "{}", view.error);
        assert_eq!(view.focus_name, "Vinhos");
        assert_eq!(view.app_id, "com.acme.app");
        assert_eq!(view.google_ads_devices, "Mobile");
        assert!(view.focus_terms.contains("malbec"));
    }

    #[test]
    fn a_catalog_path_cannot_escape_the_project() {
        assert!(safe_relative("../x.csv").is_err());
        assert!(safe_relative("/tmp/x.csv").is_err());
        assert!(safe_relative("data/catalog.csv").is_ok());
    }

    #[test]
    fn empty_lists_are_removed_from_the_toml() {
        let mut table = toml::Table::new();
        insert_list(&mut table, "brand_terms", " ");
        assert!(!table.contains_key("brand_terms"));
        insert_list(&mut table, "brand_terms", "acme, acme app");
        assert_eq!(
            table
                .get("brand_terms")
                .and_then(toml::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }
}
