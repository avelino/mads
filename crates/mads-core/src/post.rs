use std::collections::BTreeSet;

use futures::{StreamExt, stream};

use crate::{
    events::{Event, EventSink},
    google::{Account, Intent, Issue, Keyword, MatchType, blocks, normalize},
    input::Business,
    web::Web,
};

const URL_CONCURRENCY: usize = 8;

/// Adds brand terms as campaign negatives everywhere but the brand campaign, and competitor
/// names everywhere but the competitor campaign. Returns a note for every term it had to skip
/// because it would block one of the campaign's own keywords. Safe to run twice.
pub fn cross_negatives(account: &mut Account, business: &Business) -> Vec<String> {
    let mut notes = Vec::new();
    for campaign in &mut account.campaigns {
        let mut terms: Vec<&String> = Vec::new();
        if campaign.intent != Intent::Brand {
            terms.extend(&business.brand_terms);
        }
        if campaign.intent != Intent::Competitor {
            terms.extend(&business.competitors);
        }
        for term in terms {
            let neg = Keyword {
                text: normalize(term),
                match_type: MatchType::Phrase,
            };
            if neg.text.is_empty() || campaign.negatives.iter().any(|n| same(n, &neg)) {
                continue;
            }
            let blocked = campaign
                .ad_groups
                .iter()
                .flat_map(|g| &g.keywords)
                .find(|k| blocks(&neg, k));
            match blocked {
                Some(k) => notes.push(format!(
                    "skipped negative '{}' in campaign '{}': it would block keyword '{}'",
                    neg.text, campaign.name, k.text
                )),
                None => campaign.negatives.push(neg),
            }
        }
    }
    notes
}

fn same(a: &Keyword, b: &Keyword) -> bool {
    normalize(&a.text) == normalize(&b.text) && a.match_type == b.match_type
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UrlResult {
    pub url: String,
    pub status: Option<u16>,
    pub ok: bool,
}

/// Every distinct final URL and sitelink URL of the account, sorted.
pub fn collect_urls(account: &Account) -> Vec<String> {
    let mut urls = BTreeSet::new();
    for c in &account.campaigns {
        urls.extend(c.ad_groups.iter().map(|g| g.final_url.clone()));
        if let Some(assets) = &c.assets {
            urls.extend(assets.sitelinks.iter().map(|s| s.url.clone()));
        }
    }
    urls.into_iter().collect()
}

/// Checks the URLs with bounded concurrency; a network error is retried once.
pub async fn check_urls(web: &dyn Web, urls: &[String], events: &EventSink) -> Vec<UrlResult> {
    let mut results: Vec<UrlResult> = stream::iter(urls)
        .map(|url| async move {
            let first = web.check_url(url).await;
            let outcome = if first.is_err() {
                web.check_url(url).await
            } else {
                first
            };
            let status = outcome.ok();
            let ok = status.is_some_and(|s| (200..300).contains(&s));
            events.emit(Event::UrlChecked {
                url: url.clone(),
                status,
                ok,
            });
            UrlResult {
                url: url.clone(),
                status,
                ok,
            }
        })
        .buffer_unordered(URL_CONCURRENCY)
        .collect()
        .await;
    results.sort_by(|a, b| a.url.cmp(&b.url));
    results
}

pub fn url_issues(results: &[UrlResult]) -> Vec<Issue> {
    results
        .iter()
        .filter(|r| !r.ok)
        .map(|r| {
            let why = r
                .status
                .map_or("unreachable".to_string(), |s| format!("HTTP {s}"));
            Issue::error("E15", r.url.clone(), format!("URL check failed: {why}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;

    use super::*;
    use crate::{
        events::EventSink,
        google::{AdGroup, BidStrategy, Campaign, Cents, Intent, Keyword, MatchType, Rsa},
        testutil,
        web::Web,
    };

    fn kw(t: &str) -> Keyword {
        Keyword {
            text: t.into(),
            match_type: MatchType::Phrase,
        }
    }

    fn group(keywords: &[&str]) -> AdGroup {
        AdGroup {
            name: "g".into(),
            default_cpc: Cents(100),
            cpc_rationale: String::new(),
            final_url: "https://vinellu.com/w/alamos".into(),
            keywords: keywords.iter().map(|k| kw(k)).collect(),
            negatives: vec![],
            rsa: Rsa::default(),
        }
    }

    fn campaign(slug: &str, intent: Intent, keywords: &[&str]) -> Campaign {
        Campaign {
            name: slug.into(),
            slug: slug.into(),
            intent,
            daily_budget: Cents(1000),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: String::new(),
            planned_ad_groups: vec![],
            ad_groups: vec![group(keywords)],
            negatives: vec![],
            assets: None,
        }
    }

    fn account(campaigns: Vec<Campaign>) -> Account {
        Account {
            brand_kit: None,
            campaigns,
        }
    }

    fn texts(c: &Campaign) -> Vec<String> {
        c.negatives.iter().map(|n| n.text.clone()).collect()
    }

    #[test]
    fn brand_and_competitor_terms_go_to_the_campaigns_they_do_not_belong_to() {
        let input = testutil::input();
        let mut a = account(vec![
            campaign("brand", Intent::Brand, &["vinellu app"]),
            campaign("catalog", Intent::Catalog, &["alamos review"]),
            campaign("competitor", Intent::Competitor, &["vivino alternativa"]),
        ]);
        let notes = cross_negatives(&mut a, &input.business);
        assert_eq!(
            texts(&a.campaigns[0]),
            ["vivino"],
            "brand campaign only excludes competitors"
        );
        assert_eq!(texts(&a.campaigns[1]), ["vinellu", "vivino"]);
        assert_eq!(
            texts(&a.campaigns[2]),
            ["vinellu"],
            "competitor campaign only excludes brand terms"
        );
        assert!(
            a.campaigns
                .iter()
                .flat_map(|c| &c.negatives)
                .all(|n| n.match_type == MatchType::Phrase)
        );
        assert!(
            notes.is_empty(),
            "no negative blocks a keyword here: {notes:?}"
        );
    }

    #[test]
    fn a_term_that_would_block_an_own_keyword_is_skipped_with_a_note() {
        let input = testutil::input();
        let mut a = account(vec![campaign(
            "generic",
            Intent::Generic,
            &["vinellu app", "vinho tinto"],
        )]);
        let notes = cross_negatives(&mut a, &input.business);
        assert_eq!(texts(&a.campaigns[0]), ["vivino"]);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("vinellu") && n.contains("vinellu app")),
            "{notes:?}"
        );
    }

    #[test]
    fn existing_negatives_are_not_duplicated() {
        let input = testutil::input();
        let mut c = campaign("catalog", Intent::Catalog, &["alamos review"]);
        c.negatives = vec![kw("Vinellu")];
        let mut a = account(vec![c]);
        cross_negatives(&mut a, &input.business);
        assert_eq!(texts(&a.campaigns[0]), ["Vinellu", "vivino"]);
    }

    #[test]
    fn running_twice_is_idempotent() {
        let input = testutil::input();
        let mut a = account(vec![campaign(
            "catalog",
            Intent::Catalog,
            &["alamos review"],
        )]);
        cross_negatives(&mut a, &input.business);
        let once = a.clone();
        cross_negatives(&mut a, &input.business);
        assert_eq!(a, once);
    }

    struct FakeWeb {
        responses: HashMap<String, Vec<Result<u16, String>>>,
        calls: Mutex<HashMap<String, usize>>,
        total: AtomicUsize,
    }

    impl FakeWeb {
        fn new(rows: &[(&str, Vec<Result<u16, String>>)]) -> Self {
            Self {
                responses: rows
                    .iter()
                    .map(|(u, r)| ((*u).to_string(), r.clone()))
                    .collect(),
                calls: Mutex::new(HashMap::new()),
                total: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Web for FakeWeb {
        async fn check_url(&self, url: &str) -> Result<u16, String> {
            self.total.fetch_add(1, Ordering::SeqCst);
            let mut calls = self.calls.lock().unwrap();
            let n = calls.entry(url.to_string()).or_default();
            let seq = self
                .responses
                .get(url)
                .cloned()
                .unwrap_or_else(|| vec![Ok(200)]);
            let r = seq.get(*n).or(seq.last()).cloned().unwrap();
            *n += 1;
            r
        }
    }

    #[test]
    fn collects_distinct_final_and_sitelink_urls() {
        let mut c = campaign("catalog", Intent::Catalog, &["a"]);
        c.ad_groups.push(group(&["b"]));
        c.assets = Some(crate::google::Assets {
            sitelinks: vec![crate::google::Sitelink {
                text: "t".into(),
                description1: None,
                description2: None,
                url: "https://vinellu.com/app".into(),
            }],
            ..Default::default()
        });
        let urls = collect_urls(&account(vec![c]));
        assert_eq!(
            urls,
            ["https://vinellu.com/app", "https://vinellu.com/w/alamos"]
        );
    }

    #[tokio::test]
    async fn check_urls_reports_each_url_once_and_marks_non_2xx_as_failed() {
        let web = FakeWeb::new(&[
            ("https://a.com/ok", vec![Ok(200)]),
            ("https://a.com/gone", vec![Ok(404)]),
        ]);
        let (events, mut rx) = EventSink::channel();
        let urls = vec![
            "https://a.com/ok".to_string(),
            "https://a.com/gone".to_string(),
        ];
        let results = check_urls(&web, &urls, &events).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().find(|r| r.url.ends_with("ok")).unwrap().ok);
        let gone = results.iter().find(|r| r.url.ends_with("gone")).unwrap();
        assert!(!gone.ok);
        assert_eq!(gone.status, Some(404));
        assert_eq!(web.total.load(Ordering::SeqCst), 2);
        let mut seen = 0;
        while rx.try_recv().is_ok() {
            seen += 1;
        }
        assert_eq!(seen, 2);
    }

    #[tokio::test]
    async fn a_network_error_is_retried_once() {
        let web = FakeWeb::new(&[("https://a.com/x", vec![Err("timeout".into()), Ok(200)])]);
        let (events, _rx) = EventSink::channel();
        let results = check_urls(&web, &["https://a.com/x".to_string()], &events).await;
        assert!(results[0].ok);
        assert_eq!(web.total.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn two_network_errors_fail_the_url() {
        let web = FakeWeb::new(&[("https://a.com/x", vec![Err("timeout".into())])]);
        let (events, _rx) = EventSink::channel();
        let results = check_urls(&web, &["https://a.com/x".to_string()], &events).await;
        assert!(!results[0].ok);
        assert_eq!(results[0].status, None);
        assert_eq!(web.total.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn failed_urls_become_e15_errors() {
        let results = vec![
            UrlResult {
                url: "https://a.com/ok".into(),
                status: Some(200),
                ok: true,
            },
            UrlResult {
                url: "https://a.com/gone".into(),
                status: Some(404),
                ok: false,
            },
            UrlResult {
                url: "https://a.com/down".into(),
                status: None,
                ok: false,
            },
        ];
        let issues = url_issues(&results);
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.code == "E15" && i.is_error()));
        assert!(issues[0].message.contains("404") && issues[1].message.contains("unreachable"));
    }
}
