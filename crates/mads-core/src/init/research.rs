use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{google::Issue, input::normalize_url};

pub const RESEARCH_FILE: &str = "research.md";

const LEVELS: [&str; 3] = ["high", "medium", "low"];
const INTENTS: [&str; 4] = ["brand", "catalog", "generic", "competitor"];
const MAX_OPPORTUNITIES: usize = 20;
const MAX_SEARCHES: usize = 10;
const MAX_SOURCES: usize = 10;
const MAX_QUESTIONS: usize = 10;

/// What the `init` agent learned about the business and where cheap, high-intent demand is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchDraft {
    /// How the business works, who its customers are and how they search, in plain words. 40 to 3000 characters.
    pub summary: String,
    /// Campaign ideas, best expected return first. At most 20.
    #[serde(default)]
    pub opportunities: Vec<Opportunity>,
    /// What could not be confirmed and the operator should check. At most 10.
    #[serde(default)]
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Opportunity {
    /// Short name of the idea. 1 to 80 characters.
    pub name: String,
    /// brand, catalog, generic or competitor.
    pub intent: String,
    /// Example searches people type. 1 to 10, each 1 to 80 characters.
    pub searches: Vec<String>,
    /// high, medium or low: how many people search this.
    pub demand: String,
    /// high, medium or low: how many advertisers compete for these searches.
    pub competition: String,
    /// Why this should bring customers at a low cost, with the facts found. 1 to 600 characters.
    pub evidence: String,
    /// Pages consulted, on the site or elsewhere. At most 10 absolute URLs.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Names found outside the site (rankings, bestsellers) that you looked up with search_site. 0 when none.
    #[serde(default)]
    pub names_checked: u32,
    /// How many of those names have a page on the site.
    #[serde(default)]
    pub names_found: u32,
}

fn length_issue(path: &str, s: &str, min: usize, max: usize) -> Option<Issue> {
    let n = s.trim().chars().count();
    (!(min..=max).contains(&n)).then(|| {
        Issue::error(
            "INPUT",
            path,
            format!("must be {min} to {max} chars, got {n}"),
        )
    })
}

fn one_of(path: &str, value: &str, allowed: &[&str]) -> Option<Issue> {
    (!allowed.contains(&value)).then(|| {
        Issue::error(
            "INPUT",
            path,
            format!("must be one of {}, got '{value}'", allowed.join(", ")),
        )
    })
}

fn too_many(path: &str, n: usize, max: usize) -> Option<Issue> {
    (n > max).then(|| Issue::error("INPUT", path, format!("at most {max}, got {n}")))
}

impl Opportunity {
    fn coverage_issue(&self, at: &str) -> Option<Issue> {
        (self.names_found > self.names_checked).then(|| {
            Issue::error(
                "INPUT",
                format!("{at}.names_found"),
                format!(
                    "{} found is more than the {} names checked",
                    self.names_found, self.names_checked
                ),
            )
        })
    }

    fn validate(&self, at: &str) -> Vec<Issue> {
        let p = |field: &str| format!("{at}.{field}");
        let mut out: Vec<Issue> = [
            length_issue(&p("name"), &self.name, 1, 80),
            one_of(&p("intent"), &self.intent, &INTENTS),
            (self.searches.is_empty() || self.searches.len() > MAX_SEARCHES).then(|| {
                Issue::error(
                    "INPUT",
                    p("searches"),
                    format!("needs 1 to {MAX_SEARCHES} searches"),
                )
            }),
        ]
        .into_iter()
        .flatten()
        .collect();
        out.extend(
            self.searches
                .iter()
                .enumerate()
                .filter_map(|(i, s)| length_issue(&p(&format!("searches[{i}]")), s, 1, 80)),
        );
        out.extend(
            [
                one_of(&p("demand"), &self.demand, &LEVELS),
                one_of(&p("competition"), &self.competition, &LEVELS),
                length_issue(&p("evidence"), &self.evidence, 1, 600),
                too_many(&p("sources"), self.sources.len(), MAX_SOURCES),
            ]
            .into_iter()
            .flatten(),
        );
        out.extend(
            self.sources
                .iter()
                .enumerate()
                .filter(|(_, u)| normalize_url(u).is_none())
                .map(|(i, _)| {
                    Issue::error(
                        "INPUT",
                        p(&format!("sources[{i}]")),
                        "must be an absolute http(s) URL",
                    )
                }),
        );
        out
    }

    fn to_markdown(&self, rank: usize) -> String {
        let searches: Vec<String> = self.searches.iter().map(|s| format!("`{s}`")).collect();
        let mut md = format!(
            "### {rank}. {}\n\n- Intent: {}\n- Demand: {}. Competition: {}.\n- Searches: {}\n- Evidence: {}\n",
            self.name,
            self.intent,
            self.demand,
            self.competition,
            searches.join(", "),
            self.evidence.trim()
        );
        if self.names_checked > 0 {
            md.push_str(&format!(
                "- Found on the site: {} of {} names checked.\n",
                self.names_found, self.names_checked
            ));
        }
        if !self.sources.is_empty() {
            md.push_str(&format!("- Sources: {}\n", self.sources.join(", ")));
        }
        md
    }
}

impl ResearchDraft {
    /// Every problem at once, so one retry can fix them all.
    pub fn validate(&self) -> Vec<Issue> {
        let mut out: Vec<Issue> = length_issue("summary", &self.summary, 40, 3000)
            .into_iter()
            .collect();
        out.extend(too_many(
            "opportunities",
            self.opportunities.len(),
            MAX_OPPORTUNITIES,
        ));
        for (i, o) in self.opportunities.iter().enumerate() {
            let at = format!("opportunities[{i}]");
            out.extend(o.validate(&at));
            out.extend(o.coverage_issue(&at));
        }
        out.extend(too_many(
            "open_questions",
            self.open_questions.len(),
            MAX_QUESTIONS,
        ));
        out.extend(
            self.open_questions
                .iter()
                .enumerate()
                .filter_map(|(i, q)| length_issue(&format!("open_questions[{i}]"), q, 1, 300)),
        );
        out
    }

    /// `research.md`: read by the operator, and by the plan agent through `get_business`.
    pub fn to_markdown(&self, business: &str) -> String {
        let mut md = format!(
            "# Research: {business}\n\n{}\n\n## Opportunities\n\n",
            self.summary.trim()
        );
        if self.opportunities.is_empty() {
            md.push_str("No opportunity was found.\n");
        } else {
            md.push_str(
                "Best expected return first. Demand and competition are the agent's estimates from the site and the web, not keyword planner data.\n",
            );
            for (i, o) in self.opportunities.iter().enumerate() {
                md.push('\n');
                md.push_str(&o.to_markdown(i + 1));
            }
        }
        if !self.open_questions.is_empty() {
            md.push_str("\n## Open questions\n\n");
            for q in &self.open_questions {
                md.push_str(&format!("- {}\n", q.trim()));
            }
        }
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opportunity() -> Opportunity {
        Opportunity {
            name: "Famous labels by name".into(),
            intent: "catalog".into(),
            searches: vec!["alamos malbec preço".into(), "casillero del diablo".into()],
            demand: "high".into(),
            competition: "low".into(),
            evidence: "Labels sold in supermarkets get searched by name, few apps bid on them."
                .into(),
            sources: vec!["https://example.com/top-wines".into()],
            names_checked: 0,
            names_found: 0,
        }
    }

    fn draft() -> ResearchDraft {
        ResearchDraft {
            summary: "A social app for wine lovers. People rate labels and follow friends.".into(),
            opportunities: vec![opportunity()],
            open_questions: vec!["Is there a value per install?".into()],
        }
    }

    fn paths(d: &ResearchDraft) -> Vec<String> {
        d.validate().into_iter().map(|i| i.path).collect()
    }

    #[test]
    fn a_complete_draft_is_valid() {
        assert!(draft().validate().is_empty(), "{:?}", draft().validate());
    }

    #[test]
    fn the_summary_must_explain_the_business() {
        let mut d = draft();
        d.summary = "short".into();
        assert_eq!(paths(&d), ["summary"]);
        d.summary = "x".repeat(3001);
        assert_eq!(paths(&d), ["summary"]);
    }

    #[test]
    fn levels_and_intents_come_from_fixed_lists() {
        let mut d = draft();
        d.opportunities[0].demand = "huge".into();
        d.opportunities[0].competition = "LOW".into();
        d.opportunities[0].intent = "display".into();
        assert_eq!(
            paths(&d),
            [
                "opportunities[0].intent",
                "opportunities[0].demand",
                "opportunities[0].competition"
            ]
        );
    }

    #[test]
    fn an_opportunity_needs_searches_and_evidence() {
        let mut d = draft();
        d.opportunities[0].searches.clear();
        d.opportunities[0].evidence = " ".into();
        d.opportunities[0].name = String::new();
        assert_eq!(
            paths(&d),
            [
                "opportunities[0].name",
                "opportunities[0].searches",
                "opportunities[0].evidence"
            ]
        );
    }

    #[test]
    fn sources_must_be_absolute_urls() {
        let mut d = draft();
        d.opportunities[0].sources = vec!["example.com".into()];
        assert_eq!(paths(&d), ["opportunities[0].sources[0]"]);
    }

    #[test]
    fn coverage_cannot_find_more_names_than_it_checked() {
        let mut d = draft();
        d.opportunities[0].names_checked = 40;
        d.opportunities[0].names_found = 41;
        assert_eq!(paths(&d), ["opportunities[0].names_found"]);
    }

    #[test]
    fn markdown_shows_the_coverage_only_when_names_were_checked() {
        let mut d = draft();
        assert!(!d.to_markdown("Vinellu").contains("Found on the site"));
        d.opportunities[0].names_checked = 40;
        d.opportunities[0].names_found = 34;
        assert!(
            d.to_markdown("Vinellu")
                .contains("- Found on the site: 34 of 40 names checked.")
        );
    }

    #[test]
    fn list_sizes_are_capped() {
        let mut d = draft();
        d.opportunities = vec![opportunity(); MAX_OPPORTUNITIES + 1];
        d.open_questions = vec!["why?".into(); MAX_QUESTIONS + 1];
        assert_eq!(paths(&d), ["opportunities", "open_questions"]);
    }

    #[test]
    fn markdown_keeps_the_order_and_says_the_numbers_are_estimates() {
        let mut d = draft();
        let mut second = opportunity();
        second.name = "Grape guides".into();
        second.intent = "generic".into();
        d.opportunities.push(second);
        let md = d.to_markdown("Vinellu");
        assert!(md.starts_with("# Research: Vinellu\n"), "{md}");
        assert!(md.contains("A social app for wine lovers."));
        assert!(
            md.contains("estimates"),
            "the reader must know these are not planner data"
        );
        let first = md.find("### 1. Famous labels by name").unwrap();
        let next = md.find("### 2. Grape guides").unwrap();
        assert!(first < next);
        assert!(md.contains("- Demand: high. Competition: low."));
        assert!(md.contains("`alamos malbec preço`, `casillero del diablo`"));
        assert!(md.contains("https://example.com/top-wines"));
        assert!(md.contains("## Open questions\n\n- Is there a value per install?"));
    }

    #[test]
    fn markdown_without_opportunities_says_so() {
        let mut d = draft();
        d.opportunities.clear();
        d.open_questions.clear();
        let md = d.to_markdown("Vinellu");
        assert!(md.contains("No opportunity was found."), "{md}");
        assert!(!md.contains("## Open questions"));
    }
}
