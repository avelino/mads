use std::{collections::BTreeMap, fmt::Write};

use crate::{
    events::Totals,
    google::{Account, CampaignKind, EDITOR_FILE, Issue},
    images::ImageStepResult,
    input::{ExportStatus, Input},
    post::UrlResult,
    workspace::{MissionState, MissionStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStatus {
    Success,
    /// Validation or URL check errors: nothing was exported.
    Invalid,
    /// Some missions failed: `--resume` retries them.
    Incomplete,
}

pub struct ReportData {
    pub run_id: String,
    pub date: String,
    pub provider: String,
    pub model: Option<String>,
    pub input: Input,
    pub account: Account,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
    pub notes: Vec<String>,
    /// `None` when the URL check was skipped or did not run.
    pub urls: Option<Vec<UrlResult>>,
    pub missions: BTreeMap<String, MissionState>,
    pub totals: Totals,
    pub status: ReportStatus,
    /// What the image step did. Default when the run has no image campaign.
    pub images: ImageStepResult,
}

const EXAMPLES_PER_CODE: usize = 5;

pub fn render_report(d: &ReportData) -> String {
    let mut md = String::new();
    let model = d.model.as_deref().unwrap_or("default");
    let _ = writeln!(md, "# mads report: {}", d.input.business.name);
    let _ = writeln!(
        md,
        "{} | {} | provider {} | model {}\n",
        d.run_id, d.date, d.provider, model
    );
    md.push_str(status_line(d));
    md.push_str("\n\n");
    summary(&mut md, d);
    bids(&mut md, d);
    validation(&mut md, d);
    url_check(&mut md, d);
    restricted(&mut md, d);
    images(&mut md, d);
    usage(&mut md, d);
    import_steps(&mut md, d);
    md
}

/// What to expect from Google's restricted content review, and the exception request to paste.
fn restricted(md: &mut String, d: &ReportData) {
    let b = &d.input.business;
    if b.restricted.is_empty() {
        return;
    }
    let policies: Vec<&str> = b.restricted.iter().map(|r| r.policy()).collect();
    let policy = policies.join(", ");
    let about = b
        .description
        .split(". ")
        .next()
        .unwrap_or(&b.description)
        .trim_end_matches('.');
    let _ = writeln!(
        md,
        "## Restricted categories\n\n\
         `business.toml` lists this business under Google Ads restricted content: {policy}. \
         Google may refuse some keywords and ads at upload even when they only inform, often by product name. \
         In Google Ads Editor they show an error such as `Alcohol sale` after posting.\n\n\
         If the business does not do what the policy restricts (for example, it does not sell the product online), \
         select the refused items in Editor, tick Request exception, paste the text below, and post again. \
         Google reviews it in a few business days. Otherwise remove the items.\n\n\
         > {name} ({url}): {about}. Our ads and keywords give information, reviews and comparisons about products in the {policy} category. We do not sell these products online. Please review them under the {policy} policy.\n",
        name = b.name,
        url = b.url,
    );
}

fn has_images(d: &ReportData) -> bool {
    d.account.campaigns.iter().any(|c| c.kind.has_images())
}

fn images(md: &mut String, d: &ReportData) {
    if !has_images(d) {
        return;
    }
    let model = d.images.model.as_deref().unwrap_or("none");
    let _ = writeln!(
        md,
        "## Images\n\nModel {model}: {} generated in this run, {} reused. Attach each file to its ad or asset group in Google Ads Editor, see How to import.\n",
        d.images.generated, d.images.reused
    );
    md.push_str("| Campaign | Asset group | Image | Ratio | Reference | File |\n|---|---|---|---|---|---|\n");
    for c in d.account.campaigns.iter().filter(|c| c.kind.has_images()) {
        for g in &c.asset_groups {
            for b in &g.images {
                let file = b
                    .file
                    .as_deref()
                    .map_or("missing".to_string(), |f| format!("`editor/{f}`"));
                let reference = b.reference.as_deref().unwrap_or("");
                let _ = writeln!(
                    md,
                    "| {} | {} | {} | {:?} | {reference} | {file} |",
                    c.name, g.name, b.id, b.ratio
                );
            }
        }
    }
    if d.input.logo.is_some()
        && d.account.campaigns.iter().any(|c| {
            matches!(
                c.kind,
                CampaignKind::DemandGen | CampaignKind::PerformanceMax
            )
        })
    {
        md.push_str("\nDemand Gen ads and Performance Max asset groups also take the logo: `editor/images/logo.png`.\n");
    }
    md.push('\n');
}

fn status_line(d: &ReportData) -> &'static str {
    match d.status {
        ReportStatus::Success => {
            "> Ready to import. Review the files in `google-ads/` before uploading."
        }
        ReportStatus::Invalid => {
            "> Not exported: the account has errors. Fix them and run `mads export <run-dir>`, or generate again."
        }
        ReportStatus::Incomplete => {
            "> Incomplete: some missions failed. Run `mads generate --resume <run-dir>` to retry only those."
        }
    }
}

fn summary(md: &mut String, d: &ReportData) {
    let (comma, cur) = (d.input.export.decimal_comma, &d.input.budget.currency);
    md.push_str("## Summary\n\n| Campaign | Format | Intent | Daily budget | Bidding | Groups | Keywords |\n|---|---|---|---|---|---|---|\n");
    for c in &d.account.campaigns {
        let kws: usize = c.ad_groups.iter().map(|g| g.keywords.len()).sum();
        let intent = serde_json::to_value(c.intent)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} {} | {} | {} | {} |",
            c.name,
            c.kind.label(),
            intent,
            c.daily_budget.format_cpc(comma),
            cur,
            c.bid_strategy.label(),
            c.ad_groups.len() + c.asset_groups.len(),
            kws
        );
    }
    md.push('\n');
}

fn bids(md: &mut String, d: &ReportData) {
    let comma = d.input.export.decimal_comma;
    md.push_str("## Budget and bids\n\nBudget shares and CPCs are estimates. mads has no auction data, so check them against the Keyword Planner before turning campaigns on.\n\n");
    for c in &d.account.campaigns {
        let _ = writeln!(md, "**{}**: {}\n", c.name, c.rationale);
        for g in &c.ad_groups {
            let _ = writeln!(
                md,
                "- {}: CPC {} ({})",
                g.name,
                g.default_cpc.format_cpc(comma),
                g.cpc_rationale
            );
        }
        md.push('\n');
    }
}

fn validation(md: &mut String, d: &ReportData) {
    md.push_str("## Validation\n\n");
    if d.errors.is_empty() && d.warnings.is_empty() && d.notes.is_empty() {
        md.push_str("No errors and no warnings.\n\n");
        return;
    }
    group(md, "Errors", &d.errors);
    group(md, "Warnings", &d.warnings);
    if !d.notes.is_empty() {
        md.push_str("### Notes\n\n");
        d.notes.iter().for_each(|n| {
            let _ = writeln!(md, "- {n}");
        });
        md.push('\n');
    }
}

fn group(md: &mut String, title: &str, issues: &[Issue]) {
    if issues.is_empty() {
        return;
    }
    let _ = writeln!(md, "### {title}\n");
    let mut by_code: BTreeMap<&str, Vec<&Issue>> = BTreeMap::new();
    issues
        .iter()
        .for_each(|i| by_code.entry(i.code.as_str()).or_default().push(i));
    for (code, list) in by_code {
        let _ = writeln!(md, "**{code} ({})**\n", list.len());
        for i in list.iter().take(EXAMPLES_PER_CODE) {
            let _ = writeln!(md, "- `{}`: {}", i.path, i.message);
        }
        if list.len() > EXAMPLES_PER_CODE {
            let _ = writeln!(md, "- and {} more", list.len() - EXAMPLES_PER_CODE);
        }
        md.push('\n');
    }
}

fn url_check(md: &mut String, d: &ReportData) {
    md.push_str("## URL check\n\n");
    let Some(urls) = &d.urls else {
        md.push_str("skipped\n\n");
        return;
    };
    md.push_str("| URL | Status |\n|---|---|\n");
    for u in urls {
        let status = u
            .status
            .map_or("unreachable".to_string(), |s| s.to_string());
        let _ = writeln!(md, "| {} | {} |", u.url, status);
    }
    md.push('\n');
}

fn usage(md: &mut String, d: &ReportData) {
    md.push_str("## Usage\n\n| Mission | Attempts | Input tokens | Output tokens | Cost | Result |\n|---|---|---|---|---|---|\n");
    for (id, m) in &d.missions {
        let cost = m
            .usage
            .cost_usd
            .map_or("n/a".to_string(), |c| format!("${c:.4}"));
        let result = match &m.status {
            MissionStatus::Finished => "finished".to_string(),
            MissionStatus::Failed { reason } => format!("failed: {reason}"),
            MissionStatus::Pending => "pending".to_string(),
            MissionStatus::Running => "running".to_string(),
        };
        let _ = writeln!(
            md,
            "| {id} | {} | {} | {} | {cost} | {result} |",
            m.attempts, m.usage.input_tokens, m.usage.output_tokens
        );
    }
    let t = &d.totals;
    let total_cost = t.cost_usd.map_or("n/a".to_string(), |c| format!("${c:.4}"));
    let _ = writeln!(
        md,
        "\nTotal: {} input tokens, {} output tokens, cost {total_cost}.\n",
        t.input_tokens, t.output_tokens
    );
}

fn import_steps(md: &mut String, d: &ReportData) {
    let last = match d.input.export.status {
        ExportStatus::Paused => "Campaigns arrive paused. Review them before enabling.",
        ExportStatus::Enabled => {
            "Campaigns arrive enabled. They start serving as soon as Google approves the ads."
        }
    };
    let _ = writeln!(
        md,
        "## How to import\n\nPick one way. Doing both creates every Search campaign twice. {last}\n\n\
         ### Google Ads Editor, the whole account\n\n\
         1. Open Google Ads Editor and get the recent changes of the account.\n\
         2. Account, Import, From file, and pick `google-ads/{EDITOR_FILE}`. Review the changes and keep them."
    );
    if has_images(d) {
        md.push_str(
            "3. Account, Import, Image assets from files, select the folder `google-ads/editor/images`, and choose to import image assets to the root folder: the pictures sit in one subfolder per campaign, which the account does not have, and the default skips them.\n\
             4. For every ad and asset group in the Images table, open its Images field and pick the files listed there. Editor does not take images from a CSV, so this step is by hand. Do it before posting: an App or Demand Gen ad without its pictures (and a Demand Gen ad without the logo) fails to post, its ad group goes up empty and Google Ads shows it with no active ads.\n\
             5. Post the changes. If an ad failed, filter Ads by errors in Editor, attach what is missing and post again.\n",
        );
    } else {
        md.push_str("3. Post the changes.\n");
    }
    let unverified = d
        .account
        .campaigns
        .iter()
        .any(|c| c.kind == CampaignKind::PerformanceMax);
    if unverified {
        md.push_str("\n> Performance Max rows follow Google's documented Editor headers and have not gone through a real import yet. Check the import preview.\n");
    }
    md.push_str(
        "\n### Google Ads on the web, Search campaigns only\n\n\
         1. Open Google Ads, then Tools, Bulk actions, Uploads.\n\
         2. Upload files 1 to 5 of `google-ads/` in numeric order.\n\
         3. Preview the changes, fix anything Google flags, then apply.\n",
    );
    if has_images(d) {
        md.push_str("\nImage and app campaigns are not in files 1 to 5: the web bulk upload takes no image files.\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        google::{AdGroup, BidStrategy, Campaign, Cents, Intent, Issue, Keyword, MatchType, Rsa},
        testutil,
        usage::Usage,
        workspace::{MissionState, MissionStatus},
    };

    fn account() -> Account {
        let group = AdGroup {
            name: "alamos".into(),
            default_cpc: Cents(150),
            cpc_rationale: "marca conhecida, clique barato".into(),
            final_url: "https://vinellu.com/w/alamos".into(),
            keywords: vec![
                Keyword {
                    text: "alamos".into(),
                    match_type: MatchType::Phrase
                };
                14
            ],
            negatives: vec![],
            rsa: Rsa::default(),
        };
        Account {
            brand_kit: None,
            campaigns: vec![Campaign {
                kind: Default::default(),
                asset_groups: Vec::new(),
                name: "Vinellu - Catalogo".into(),
                slug: "vinellu-catalogo".into(),
                intent: Intent::Catalog,
                daily_budget: Cents(3000),
                bid_strategy: BidStrategy::ManualCpc,
                rationale: "intencao alta, maior fatia".into(),
                planned_ad_groups: vec![],
                ad_groups: vec![group],
                negatives: vec![],
                assets: None,
            }],
        }
    }

    fn data(input: &crate::input::Input, account: &Account) -> ReportData {
        let mut missions = std::collections::BTreeMap::new();
        missions.insert(
            "plan".to_string(),
            MissionState {
                status: MissionStatus::Finished,
                attempts: 1,
                usage: Usage {
                    input_tokens: 1200,
                    output_tokens: 300,
                    cost_usd: None,
                },
            },
        );
        missions.insert(
            "campaign:vinellu-catalogo".to_string(),
            MissionState {
                status: MissionStatus::Finished,
                attempts: 2,
                usage: Usage {
                    input_tokens: 5000,
                    output_tokens: 900,
                    cost_usd: Some(0.1234),
                },
            },
        );
        ReportData {
            run_id: "20261001-101200-abc123".into(),
            date: "2026-10-01".into(),
            provider: "anthropic".into(),
            model: Some("claude-sonnet-5-5".into()),
            input: input.clone(),
            account: account.clone(),
            errors: vec![],
            warnings: vec![],
            notes: vec![],
            urls: None,
            missions,
            totals: Totals::default(),
            status: ReportStatus::Success,
            images: Default::default(),
        }
    }

    fn render_default() -> String {
        let input = testutil::input();
        render_report(&data(&input, &account()))
    }

    #[test]
    fn header_and_status_line() {
        let md = render_default();
        assert!(md.starts_with("# mads report: Vinellu\n"));
        assert!(
            md.contains("20261001-101200-abc123")
                && md.contains("anthropic")
                && md.contains("claude-sonnet-5-5")
        );
        assert!(md.contains("Ready to import"));
    }

    #[test]
    fn summary_table_has_one_row_per_campaign() {
        let md = render_default();
        let row = md
            .lines()
            .find(|l| l.starts_with("| Vinellu - Catalogo"))
            .expect("campaign row");
        for cell in ["catalog", "30,00 BRL", "Manual CPC", "| 1 |", "| 14 |"] {
            assert!(row.contains(cell), "{cell} missing in {row}");
        }
    }

    #[test]
    fn bids_are_labeled_as_estimates_with_their_rationale() {
        let md = render_default();
        assert!(md.contains("estimates"), "must say bids are estimates");
        assert!(
            md.contains("alamos")
                && md.contains("1,50")
                && md.contains("marca conhecida, clique barato")
        );
        assert!(md.contains("intencao alta, maior fatia"));
    }

    #[test]
    fn validation_groups_by_code_with_counts_and_at_most_five_examples() {
        let input = testutil::input();
        let mut d = data(&input, &account());
        d.errors = (0..7)
            .map(|i| {
                Issue::error(
                    "E01",
                    format!("campaigns[0].h[{i}]"),
                    "31 chars, limit is 30",
                )
            })
            .collect();
        d.warnings = vec![Issue::warning("W01", "campaigns[0].x", "third-party term")];
        d.status = ReportStatus::Invalid;
        let md = render_report(&d);
        assert!(md.contains("E01 (7)") && md.contains("W01 (1)"));
        assert_eq!(md.matches("campaigns[0].h[").count(), 5);
        assert!(md.contains("and 2 more"));
        assert!(md.contains("Not exported"));
    }

    #[test]
    fn notes_are_listed_under_validation() {
        let input = testutil::input();
        let mut d = data(&input, &account());
        d.notes = vec!["skipped negative 'vinellu'".into()];
        assert!(render_report(&d).contains("skipped negative 'vinellu'"));
    }

    #[test]
    fn url_check_is_skipped_or_listed() {
        let input = testutil::input();
        let mut d = data(&input, &account());
        assert!(render_report(&d).contains("skipped"));
        d.urls = Some(vec![
            UrlResult {
                url: "https://vinellu.com/w/alamos".into(),
                status: Some(200),
                ok: true,
            },
            UrlResult {
                url: "https://vinellu.com/gone".into(),
                status: Some(404),
                ok: false,
            },
        ]);
        let md = render_report(&d);
        assert!(
            md.contains("| https://vinellu.com/w/alamos | 200 |")
                && md.contains("| https://vinellu.com/gone | 404 |")
        );
    }

    #[test]
    fn usage_lists_missions_and_shows_cost_only_when_known() {
        let md = render_default();
        let plan = md
            .lines()
            .find(|l| l.starts_with("| plan "))
            .expect("plan row");
        assert!(
            plan.contains("1200") && plan.contains("300") && plan.contains("n/a"),
            "{plan}"
        );
        let camp = md
            .lines()
            .find(|l| l.starts_with("| campaign:vinellu-catalogo"))
            .expect("campaign row");
        assert!(camp.contains("| 2 |") && camp.contains("$0.1234"), "{camp}");
    }

    #[test]
    fn failed_missions_are_listed_with_the_resume_hint() {
        let input = testutil::input();
        let mut d = data(&input, &account());
        d.missions.insert(
            "campaign:x".into(),
            MissionState {
                status: MissionStatus::Failed {
                    reason: "max turns".into(),
                },
                attempts: 2,
                usage: Usage::default(),
            },
        );
        d.status = ReportStatus::Incomplete;
        let md = render_report(&d);
        assert!(md.contains("campaign:x") && md.contains("max turns") && md.contains("--resume"));
    }

    #[test]
    fn import_instructions_are_present_and_there_is_no_em_dash() {
        let md = render_default();
        assert!(md.contains("Bulk actions") && md.contains("numeric order"));
        assert!(!md.contains('\u{2014}'));
    }

    #[test]
    fn restricted_categories_get_their_section_and_exception_text() {
        let mut input = testutil::input();
        assert!(!render_report(&data(&input, &account())).contains("## Restricted categories"));
        input.business.restricted = vec![crate::input::RestrictedCategory::Alcohol];
        let md = render_report(&data(&input, &account()));
        assert!(md.contains("## Restricted categories"), "{md}");
        assert!(md.contains("Google Ads restricted content: Alcohol."));
        assert!(md.contains("> Vinellu (https://vinellu.com): App social de vinhos com reviews, safras e harmonização."));
        assert!(md.contains("Request exception"));
    }

    fn with_demand_gen(mut account: Account) -> Account {
        use crate::google::{AspectRatio, AssetGroup, CampaignKind, ImageBrief};
        let brief = ImageBrief {
            id: "brinde".into(),
            ratio: AspectRatio::Square,
            prompt: "two adults toasting".into(),
            reference: None,
            file: Some("images/feed/tintos-brinde.jpg".into()),
        };
        let mut c = account.campaigns[0].clone();
        c.name = "Feed".into();
        c.slug = "feed".into();
        c.kind = CampaignKind::DemandGen;
        c.ad_groups = vec![];
        c.asset_groups = vec![AssetGroup {
            name: "Tintos".into(),
            final_url: "https://vinellu.com".into(),
            business_name: "Vinellu".into(),
            headlines: vec!["H".into()],
            long_headlines: vec![],
            descriptions: vec!["D".into()],
            search_themes: vec![],
            images: vec![brief],
        }];
        account.campaigns.push(c);
        account
    }

    #[test]
    fn image_ads_must_get_their_pictures_before_posting() {
        let mut input = testutil::input();
        input.logo = Some("logo.png".into());
        let md = render_report(&data(&input, &with_demand_gen(account())));
        assert!(md.contains("before posting"), "{md}");
        assert!(md.contains("no active ads"), "{md}");
        assert!(md.contains("filter Ads by errors"), "{md}");
        assert!(
            md.contains("|\n\nDemand Gen ads and Performance Max asset groups also take the logo"),
            "the logo line must not join the table: {md}"
        );
    }

    #[test]
    fn import_steps_follow_the_export_status() {
        let mut input = testutil::input();
        let paused = render_report(&data(&input, &account()));
        assert!(paused.contains("arrive paused"), "{paused}");
        input.export.status = crate::input::ExportStatus::Enabled;
        let enabled = render_report(&data(&input, &account()));
        assert!(!enabled.contains("arrive paused"), "{enabled}");
        assert!(enabled.contains("arrive enabled"), "{enabled}");
    }
}
