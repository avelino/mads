use crate::{
    agent::RunControl,
    events::{Event, EventSink},
    google::{
        Account, CsvFile, ExportError, ExportLayout, Issue, Rules, automation_warnings,
        export_automation, export_csvs, export_editor_live,
    },
    input::Input,
    perf::Live,
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
    /// `(from, to)` copies of existing files, both relative to the platform directory, made
    /// after `csv` is written: the pictures of the automation package. Empty for `bulk` and on
    /// failure.
    pub copies: Vec<(String, String)>,
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
        copies: Vec::new(),
    }
}

/// Knobs for validation and the CSV export.
#[derive(Debug, Clone)]
pub struct ExportOpts {
    pub skip_url_check: bool,
    pub max_ad_groups: usize,
    pub layout: ExportLayout,
    /// Polled between URL checks so a cancel stops the remaining network work.
    pub control: RunControl,
}

/// Bulk upload files 1 to 5 for Search, then the Editor file for image campaigns.
fn export_all(
    input: &Input,
    account: &Account,
    live: Option<&Live>,
) -> Result<Vec<CsvFile>, ExportError> {
    let mut files = export_csvs(input, account)?;
    files.extend(export_editor_live(input, account, live));
    Ok(files)
}

pub async fn finalize(
    input: &Input,
    account: &Account,
    live: Option<&Live>,
    web: &dyn Web,
    opts: ExportOpts,
    events: &EventSink,
) -> Finalized {
    step(events, "validate", "checking limits and policies");
    let (mut errors, mut warnings): (Vec<_>, Vec<_>) = Rules::new(input, opts.max_ad_groups)
        .account(account, true)
        .into_iter()
        .partition(Issue::is_error);
    if opts.layout == ExportLayout::DriveFolders {
        // Package-specific gaps become W12 omissions instead of blocking E16/E18/E20/E24.
        // google_ads.app_id also satisfies the App campaign requirement that Rules checks via [app].
        drop_drive_layout_blockers(input, account, &mut errors);
        warnings.extend(automation_warnings(input, account));
    }
    events.emit(Event::Validation {
        errors: errors.clone(),
        warnings: warnings.clone(),
    });
    if !errors.is_empty() {
        return invalid(errors, warnings, None);
    }

    let urls = if opts.skip_url_check {
        step(events, "url-check", "skipped");
        None
    } else if opts.control.is_cancelled() {
        return Finalized {
            exit_code: 1,
            errors: Vec::new(),
            warnings,
            urls: None,
            csv: Vec::new(),
            copies: Vec::new(),
        };
    } else {
        let list = collect_urls(account);
        step(events, "url-check", &format!("{} URLs", list.len()));
        Some(check_urls(web, &list, events, &opts.control).await)
    };
    let url_errors = urls.as_deref().map(url_issues).unwrap_or_default();
    if !url_errors.is_empty() {
        events.emit(Event::Validation {
            errors: url_errors.clone(),
            warnings: Vec::new(),
        });
        return invalid(url_errors, warnings, urls);
    }

    step(events, "export", "writing files");
    let exported = match opts.layout {
        ExportLayout::Bulk => export_all(input, account, live).map(|files| (files, Vec::new())),
        ExportLayout::DriveFolders => {
            export_automation(input, account, live).map(|out| (out.files, out.pictures))
        }
    };
    match exported {
        Ok((csv, copies)) => Finalized {
            exit_code: 0,
            errors: Vec::new(),
            warnings,
            urls,
            csv,
            copies,
        },
        Err(e) => invalid(vec![export_issue(&e)], warnings, urls),
    }
}

/// Removes errors that `--layout drive-folders` turns into W12 omissions (or that
/// `google_ads.app_id` already covers for App campaigns). Bulk keeps them as hard errors.
fn drop_drive_layout_blockers(input: &Input, account: &Account, errors: &mut Vec<Issue>) {
    use crate::google::image_campaign_gaps;
    use std::collections::BTreeSet;

    let gaps: BTreeSet<usize> = image_campaign_gaps(input, account)
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    let has_app_fallback = !input.google_ads.app_id.trim().is_empty();
    errors.retain(|e| !drive_layout_omittable(e, &gaps, has_app_fallback));
}

fn drive_layout_omittable(
    e: &Issue,
    gaps: &std::collections::BTreeSet<usize>,
    has_app_fallback: bool,
) -> bool {
    match e.code.as_str() {
        "E16" | "E18" | "E20" => campaign_index(&e.path).is_some_and(|i| gaps.contains(&i)),
        "E24" => has_app_fallback || campaign_index(&e.path).is_some_and(|i| gaps.contains(&i)),
        _ => false,
    }
}

/// `campaigns[N]` or `campaigns[N].…` → N.
fn campaign_index(path: &str) -> Option<usize> {
    let rest = path.strip_prefix("campaigns[")?;
    let (n, _) = rest.split_once(']')?;
    n.parse().ok()
}

/// `E29` when the automation package is over capacity, `EXPORT` for any other failure.
fn export_issue(e: &ExportError) -> Issue {
    let code = match e {
        ExportError::Capacity(_) => "E29",
        _ => "EXPORT",
    };
    Issue::error(code, "", e.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;

    use super::*;
    use crate::{
        events::{Event, EventSink},
        google::{
            Account, AspectRatio, AssetGroup, BidStrategy, Campaign, CampaignKind, ImageBrief,
            Intent,
        },
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

    fn opts(skip_url_check: bool, max_ad_groups: usize, layout: ExportLayout) -> ExportOpts {
        ExportOpts {
            skip_url_check,
            max_ad_groups,
            layout,
            control: RunControl::default(),
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
            google_ads: Default::default(),
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
            None,
            &FakeWeb(HashMap::new()),
            opts(false, 50, ExportLayout::Bulk),
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 0, "{:?}", out.errors);
        let names: Vec<&str> = out.csv.iter().map(|f| f.name.as_str()).collect();
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
        assert!(out.copies.is_empty(), "bulk copies nothing");
    }

    #[tokio::test]
    async fn skipping_the_url_check_leaves_urls_empty() {
        let account = reference_account();
        let input = reference_input(&account);
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::Bulk),
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
            None,
            &FakeWeb(HashMap::new()),
            opts(false, 50, ExportLayout::Bulk),
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
        let out = finalize(
            &input,
            &account,
            None,
            &web,
            opts(false, 50, ExportLayout::Bulk),
            &events,
        )
        .await;
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
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::Bulk),
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 3);
        assert!(out.errors.iter().any(|e| e.code == "E17"));
        assert!(out.csv.is_empty());
    }

    #[tokio::test]
    async fn drive_layout_edits_a_live_account() {
        let account = reference_account();
        let input = reference_input(&account);
        let name = account.campaigns[0].name.clone();
        let mut performance = crate::perf::Performance::default();
        performance.campaigns.push(crate::perf::CampaignPerf {
            name: name.clone(),
            live_status: Some(crate::perf::LiveStatus::Enabled),
            ..Default::default()
        });
        let live = Live {
            baseline: account.clone(),
            performance,
        };
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            Some(&live),
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 0, "{:?}", out.errors);
        let feed = out
            .csv
            .iter()
            .find(|f| f.name == "automation/Vinellu Ads Automation/0. Feed.xlsx")
            .expect("feed");
        let rows = campaign_rows(&feed.bytes);
        let row = rows
            .iter()
            .find(|r| r[2] == name)
            .expect("live campaign row");
        assert_eq!(row.last().map(String::as_str), Some("Edit"));
    }

    /// Data rows of the `Campaigns` tab of `0. Feed.xlsx`, as text.
    fn campaign_rows(bytes: &[u8]) -> Vec<Vec<String>> {
        use calamine::{Reader, Xlsx, open_workbook_from_rs};
        let mut book: Xlsx<_> =
            open_workbook_from_rs(std::io::Cursor::new(bytes.to_vec())).expect("xlsx");
        let range = book.worksheet_range("Campaigns").expect("Campaigns tab");
        range
            .rows()
            .skip(4)
            .map(|r| r.iter().map(ToString::to_string).collect())
            .collect()
    }

    #[tokio::test]
    async fn an_over_capacity_package_is_an_e29_error_and_nothing_is_exported() {
        let mut account = reference_account();
        let mut input = reference_input(&account);
        input.catalog = (0..700)
            .map(|i| crate::input::CatalogItem {
                id: format!("w{i}"),
                name: format!("Wine {i}"),
                url: format!("https://vinellu.com/w/{i}"),
                category: "red".into(),
                aliases: vec![],
                third_party: false,
                notes: String::new(),
                image: None,
            })
            .collect();
        account.campaigns[0].planned_ad_groups[0].entity_ids =
            input.catalog.iter().map(|c| c.id.clone()).collect();
        let (events, _rx) = EventSink::channel();
        let out = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 3);
        assert!(
            out.errors.iter().any(|e| e.code == "E29"),
            "{:?}",
            out.errors
        );
        assert!(out.csv.is_empty());
    }

    #[test]
    fn other_export_failures_keep_the_export_code() {
        assert_eq!(export_issue(&ExportError::MissingBrandKit).code, "EXPORT");
        assert_eq!(export_issue(&ExportError::Capacity("x".into())).code, "E29");
    }

    #[tokio::test]
    async fn drive_layout_keeps_character_limits_and_warns_without_a_customer_id() {
        let mut account = reference_account();
        account.campaigns[0].ad_groups[0].rsa.headlines[0] = "x".repeat(31);
        let input = reference_input(&account);
        let (events, _rx) = EventSink::channel();
        let blocked = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(blocked.exit_code, 3);
        assert!(blocked.errors.iter().any(|e| e.code == "E01"));
        assert!(blocked.csv.is_empty());
        assert!(blocked.warnings.iter().any(|w| w.code == "W11"));

        let account = reference_account();
        let input = reference_input(&account);
        let out = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(out.exit_code, 0, "{:?}", out.errors);
        assert!(
            out.csv
                .iter()
                .any(|f| f.name.ends_with("Structure/S1. New Campaigns.xlsx"))
        );
        assert!(out.csv.iter().all(|f| !f.name.ends_with("1-campaign.csv")));
        assert!(
            out.warnings
                .iter()
                .any(|w| w.code == "W11" && w.path == "google_ads.customer_id")
        );

        let limited = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 1, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(limited.exit_code, 3);
        assert!(limited.errors.iter().any(|e| e.code == "E13"));
        assert!(limited.csv.is_empty());
    }

    fn empty_asset_group(name: &str) -> AssetGroup {
        AssetGroup {
            name: name.into(),
            final_url: "https://vinellu.com".into(),
            business_name: "Vinellu".into(),
            headlines: vec![
                "Reviews reais".into(),
                "Escaneie o rotulo".into(),
                "Notas de quem bebeu".into(),
            ],
            long_headlines: vec!["Veja a nota antes de abrir a garrafa".into()],
            descriptions: vec!["Notas de quem bebeu.".into(), "Reviews reais.".into()],
            search_themes: vec![],
            images: vec![],
        }
    }

    fn image_campaign(kind: CampaignKind, name: &str, slug: &str) -> Campaign {
        let group = empty_asset_group("group");
        Campaign {
            name: name.into(),
            slug: slug.into(),
            kind,
            intent: Intent::Generic,
            daily_budget: Cents(1000),
            bid_strategy: BidStrategy::MaximizeConversions,
            rationale: String::new(),
            planned_ad_groups: vec![crate::google::PlannedAdGroup {
                name: group.name.clone(),
                theme: String::new(),
                entity_ids: vec![],
                final_url: String::new(),
            }],
            ad_groups: vec![],
            asset_groups: vec![group],
            negatives: vec![],
            assets: None,
        }
    }

    #[tokio::test]
    async fn drive_layout_omits_incomplete_image_campaigns_with_w12() {
        let mut account = reference_account();
        account.campaigns[0].daily_budget = Cents(4000);
        account
            .campaigns
            .push(image_campaign(CampaignKind::PerformanceMax, "PMax", "pmax"));
        let mut input = reference_input(&account);
        input.budget.daily = Cents(5000);
        input.business.conversion_tracking = true;
        let (events, _rx) = EventSink::channel();

        let bulk = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::Bulk),
            &events,
        )
        .await;
        assert_eq!(bulk.exit_code, 3, "{:?}", bulk.errors);
        assert!(
            bulk.errors
                .iter()
                .any(|e| e.code == "E18" || e.code == "E16"),
            "{:?}",
            bulk.errors
        );

        let drive = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(drive.exit_code, 0, "{:?}", drive.errors);
        assert!(
            drive.warnings.iter().any(|w| w.code == "W12"),
            "{:?}",
            drive.warnings
        );
        assert!(
            drive.csv.iter().any(|f| f.name.ends_with("0. Feed.xlsx")),
            "package still exports"
        );
    }

    #[tokio::test]
    async fn drive_layout_accepts_google_ads_app_id_without_app_table() {
        let mut account = reference_account();
        account.campaigns[0].daily_budget = Cents(4000);
        let mut app = image_campaign(CampaignKind::AppInstalls, "App", "app");
        app.asset_groups[0].images = vec![ImageBrief {
            id: "shot".into(),
            ratio: AspectRatio::Landscape,
            prompt: "A phone showing a wine label scan".into(),
            reference: None,
            file: Some("images/app/shot.jpg".into()),
        }];
        app.asset_groups[0].business_name = String::new();
        app.asset_groups[0].long_headlines.clear();
        app.asset_groups[0].final_url =
            "https://play.google.com/store/apps/details?id=com.vinellu.app".into();
        account.campaigns.push(app);

        let mut input = reference_input(&account);
        input.budget.daily = Cents(5000);
        input.app = None;
        input.google_ads.app_id = "com.vinellu.app".into();
        let (events, _rx) = EventSink::channel();

        let bulk = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::Bulk),
            &events,
        )
        .await;
        assert_eq!(bulk.exit_code, 3, "{:?}", bulk.errors);
        assert!(
            bulk.errors.iter().any(|e| e.code == "E24"),
            "{:?}",
            bulk.errors
        );

        let drive = finalize(
            &input,
            &account,
            None,
            &FakeWeb(HashMap::new()),
            opts(true, 50, ExportLayout::DriveFolders),
            &events,
        )
        .await;
        assert_eq!(drive.exit_code, 0, "{:?}", drive.errors);
        assert!(
            drive
                .warnings
                .iter()
                .filter(|w| w.code == "W12")
                .all(|w| !w.message.contains("App campaign")),
            "app_id fallback should keep the App campaign: {:?}",
            drive.warnings
        );
    }
}
