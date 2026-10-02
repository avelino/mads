use crate::{
    events::{Event, EventSink},
    google::{Account, CsvFile, ExportError, Issue, Rules, export_csvs, export_editor},
    input::Input,
    post::{UrlResult, check_urls, collect_urls, url_issues},
    web::Web,
};

/// Result of the last three post-processing steps: validate, check URLs, export.
#[derive(Debug, Clone)]
pub struct Finalized {
    /// 0 when the CSVs were produced, 3 when the account is invalid.
    pub exit_code: i32,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
    pub urls: Option<Vec<UrlResult>>,
    pub csv: Vec<CsvFile>,
}

fn step(events: &EventSink, name: &str, detail: &str) {
    events.emit(Event::Step {
        name: name.into(),
        detail: detail.into(),
    });
}

fn invalid(errors: Vec<Issue>, warnings: Vec<Issue>, urls: Option<Vec<UrlResult>>) -> Finalized {
    Finalized {
        exit_code: 3,
        errors,
        warnings,
        urls,
        csv: Vec::new(),
    }
}

/// Bulk upload files 1 to 5 for Search, then the Editor file for image campaigns.
fn export_all(input: &Input, account: &Account) -> Result<Vec<CsvFile>, ExportError> {
    let mut files = export_csvs(input, account)?;
    files.extend(export_editor(input, account));
    Ok(files)
}

pub async fn finalize(
    input: &Input,
    account: &Account,
    web: &dyn Web,
    skip_url_check: bool,
    max_ad_groups: usize,
    events: &EventSink,
) -> Finalized {
    step(events, "validate", "checking limits and policies");
    let (errors, warnings): (Vec<_>, Vec<_>) = Rules::new(input, max_ad_groups)
        .account(account, true)
        .into_iter()
        .partition(Issue::is_error);
    events.emit(Event::Validation {
        errors: errors.clone(),
        warnings: warnings.clone(),
    });
    if !errors.is_empty() {
        return invalid(errors, warnings, None);
    }

    let urls = if skip_url_check {
        step(events, "url-check", "skipped");
        None
    } else {
        let list = collect_urls(account);
        step(events, "url-check", &format!("{} URLs", list.len()));
        Some(check_urls(web, &list, events).await)
    };
    let url_errors = urls.as_deref().map(url_issues).unwrap_or_default();
    if !url_errors.is_empty() {
        events.emit(Event::Validation {
            errors: url_errors.clone(),
            warnings: Vec::new(),
        });
        return invalid(url_errors, warnings, urls);
    }

    step(events, "export", "writing CSV files");
    match export_all(input, account) {
        Ok(csv) => Finalized {
            exit_code: 0,
            errors: Vec::new(),
            warnings,
            urls,
            csv,
        },
        Err(e) => invalid(
            vec![Issue::error("EXPORT", "", e.to_string())],
            warnings,
            urls,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;

    use super::*;
    use crate::{
        events::{Event, EventSink},
        google::Account,
        input::{Budget, Business, ExportConfig, ExportStatus, Input, Page},
        money::Cents,
    };

    struct FakeWeb(HashMap<String, u16>);

    #[async_trait]
    impl Web for FakeWeb {
        async fn check_url(&self, url: &str) -> Result<u16, String> {
            Ok(self.0.get(url).copied().unwrap_or(200))
        }
    }

    fn reference_account() -> Account {
        serde_json::from_str(include_str!("../tests/fixtures/vinellu.account.json")).unwrap()
    }

    fn reference_input(account: &Account) -> Input {
        let pages = account
            .campaigns
            .iter()
            .flat_map(|c| &c.ad_groups)
            .map(|g| Page {
                name: g.name.clone(),
                url: g.final_url.clone(),
            })
            .collect();
        Input {
            logo: None,
            formats: Vec::new(),
            design: String::new(),
            focus: None,
            app: None,
            business: Business {
                name: "Vinellu".into(),
                url: "https://vinellu.com".into(),
                language: "pt-BR".into(),
                locations: vec!["Brazil".into()],
                goal: "cadastros".into(),
                description: "App social de vinhos com reviews e safras.".into(),
                conversion_tracking: false,
                restricted: vec![],
                brand_terms: vec!["vinellu".into()],
                competitors: vec![],
                avoid: vec![],
                pages,
            },
            budget: Budget {
                daily: Cents(5000),
                currency: "BRL".into(),
                max_cpc: None,
            },
            export: ExportConfig {
                status: ExportStatus::Paused,
                url_suffix: "utm_campaign=rotulos".into(),
                eu_political_ads: false,
                decimal_comma: true,
            },
            research: String::new(),
            catalog: vec![],
        }
    }

    #[tokio::test]
    async fn valid_account_exports_the_web_files_and_the_editor_file() {
        let account = reference_account();
        let input = reference_input(&account);
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            &FakeWeb(HashMap::new()),
            false,
            50,
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 0, "{:?}", out.errors);
        let names: Vec<&str> = out.csv.iter().map(|f| f.name).collect();
        assert_eq!(
            names,
            [
                "1-campaign.csv",
                "2-ad-groups.csv",
                "3-keywords.csv",
                "4-negative-keywords.csv",
                "5-responsive-search-ads.csv",
                "editor/account.csv"
            ],
            "the web files for Search, and the whole account for Editor"
        );
        assert_eq!(out.urls.as_ref().map(Vec::len), Some(12));
    }

    #[tokio::test]
    async fn skipping_the_url_check_leaves_urls_empty() {
        let account = reference_account();
        let input = reference_input(&account);
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            &FakeWeb(HashMap::new()),
            true,
            50,
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 0);
        assert!(out.urls.is_none());
    }

    #[tokio::test]
    async fn validation_errors_stop_before_url_check_and_export() {
        let mut account = reference_account();
        account.campaigns[0].daily_budget = Cents(1000);
        let input = reference_input(&account);
        let (events, mut rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            &FakeWeb(HashMap::new()),
            false,
            50,
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 3);
        assert!(out.errors.iter().any(|e| e.code == "E06"));
        assert!(out.csv.is_empty() && out.urls.is_none());
        let mut saw_validation = false;
        while let Ok(e) = rx.try_recv() {
            saw_validation |= matches!(e.event, Event::Validation { .. });
        }
        assert!(saw_validation);
    }

    #[tokio::test]
    async fn a_dead_url_is_an_e15_error_and_nothing_is_exported() {
        let account = reference_account();
        let input = reference_input(&account);
        let dead = account.campaigns[0].ad_groups[0].final_url.clone();
        let web = FakeWeb(HashMap::from([(dead.clone(), 404)]));
        let (events, _rx) = EventSink::channel();
        let out = finalize(&input, &account, &web, false, 50, &events).await;
        assert_eq!(out.exit_code, 3);
        assert!(out.errors.iter().any(|e| e.code == "E15" && e.path == dead));
        assert!(out.csv.is_empty());
        assert!(out.urls.is_some(), "url results are kept for the report");
    }

    #[tokio::test]
    async fn a_bid_strategy_that_does_not_fit_the_kind_stops_before_export() {
        let mut account = reference_account();
        account.campaigns[0].bid_strategy = crate::google::BidStrategy::MaximizeConversions;
        let mut input = reference_input(&account);
        input.business.conversion_tracking = true;
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            &FakeWeb(HashMap::new()),
            true,
            50,
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 3);
        assert!(out.errors.iter().any(|e| e.code == "E17"));
        assert!(out.csv.is_empty());
    }
}
