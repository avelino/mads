mod images;

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Account, AdGroup, Assets, BidStrategy, BrandKit, Campaign, CampaignKind, Cents, Intent, Issue,
    Keyword, MatchType, Rsa, char_len, contains_word_sequence, merge_rsa, normalize,
};
use crate::input::{AllowedUrls, Budget, Business, Input, fold};

const HEADLINE: usize = 30;
const DESCRIPTION: usize = 90;
const PATH: usize = 15;
const SITELINK_TEXT: usize = 25;
const SITELINK_DESC: usize = 35;
const CALLOUT: usize = 25;
const SNIPPET_VALUE: usize = 25;
const NAME: usize = 255;
const KEYWORD: usize = 80;
const KEYWORD_WORDS: usize = 10;
const KEYWORD_FORBIDDEN: &str = "!@%^*(){};~`<>?\\|,[]\"";
const MIN_CAMPAIGN_BUDGET: Cents = Cents(100);

/// Validation rules (spec section 6.4). Pure: no IO, no mutation.
pub struct Rules<'a> {
    business: &'a Business,
    budget: &'a Budget,
    allowed: AllowedUrls,
    third_party_terms: Vec<String>,
    avoid: Vec<String>,
    brand_words: BTreeSet<String>,
    max_ad_groups: usize,
    /// Catalog id to whether the item has a real photo.
    catalog_photos: BTreeMap<String, bool>,
    has_logo: bool,
    has_app: bool,
    /// Formats `business.toml` requires, at least one campaign each.
    formats: &'a [CampaignKind],
    /// With `[focus]`, the only final URLs allowed.
    focus: Option<AllowedUrls>,
    /// `focus.terms`, folded: every keyword needs one term of each group.
    focus_terms: Vec<Vec<String>>,
}

impl<'a> Rules<'a> {
    pub fn new(input: &'a Input, max_ad_groups: usize) -> Self {
        let mut allowed = AllowedUrls::default();
        allowed.insert(&input.business.url);
        input
            .business
            .pages
            .iter()
            .for_each(|p| allowed.insert(&p.url));
        input.catalog.iter().for_each(|c| allowed.insert(&c.url));
        let focus = input.focus.as_ref().map(|f| AllowedUrls::new(&f.urls));
        input
            .focus
            .iter()
            .flat_map(|f| &f.urls)
            .for_each(|u| allowed.insert(u));

        let mut third_party_terms: Vec<String> = input
            .business
            .competitors
            .iter()
            .map(|c| normalize(c))
            .collect();
        for item in input.catalog.iter().filter(|c| c.third_party) {
            third_party_terms.push(normalize(&item.name));
            third_party_terms.extend(item.aliases.iter().map(|a| normalize(a)));
        }
        third_party_terms.retain(|t| !t.is_empty());

        let brand_words = input
            .business
            .brand_terms
            .iter()
            .flat_map(|t| {
                normalize(t)
                    .split_whitespace()
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .collect();
        Self {
            business: &input.business,
            budget: &input.budget,
            allowed,
            third_party_terms,
            avoid: input.business.avoid.iter().map(|a| normalize(a)).collect(),
            brand_words,
            max_ad_groups,
            catalog_photos: input
                .catalog
                .iter()
                .map(|c| (c.id.clone(), c.image.is_some()))
                .collect(),
            has_logo: input.logo.is_some(),
            has_app: input.app.is_some(),
            formats: &input.formats,
            focus,
            focus_terms: input
                .focus
                .iter()
                .flat_map(|f| &f.terms)
                .map(|g| {
                    g.iter()
                        .map(|t| fold(t))
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .collect(),
        }
    }

    pub fn url_allowed(&self, url: &str) -> bool {
        self.allowed.contains(url)
    }

    /// True without `[focus]`, or when the URL is one of the focus pages.
    pub fn in_focus(&self, url: &str) -> bool {
        self.focus.as_ref().is_none_or(|f| f.contains(url))
    }

    /// E23: a search that is not about the focus, such as a competitor name without the route.
    pub(crate) fn focus_terms(&self, out: &mut Vec<Issue>, path: &str, text: &str) {
        let words = fold(text);
        let missing: Vec<&str> = self
            .focus_terms
            .iter()
            .filter(|g| !g.iter().any(|t| contains_word_sequence(&words, t)))
            .filter_map(|g| g.first().map(String::as_str))
            .collect();
        if !missing.is_empty() {
            let msg = format!(
                "'{text}' is not about the focus: add a word like {}",
                missing.join(" and ")
            );
            out.push(Issue::error("E23", path, msg));
        }
    }

    /// E22: a landing page outside `[focus]`. Sitelinks are not landing pages and may go elsewhere.
    pub(crate) fn focus_url(&self, out: &mut Vec<Issue>, path: &str, url: &str) {
        if !self.in_focus(url) {
            let msg = format!("final URL is not a [focus] page: {url}");
            out.push(Issue::error("E22", path, msg));
        }
    }

    /// `complete` is true at `finish` time: every planned ad group must exist.
    pub fn account(&self, a: &Account, complete: bool) -> Vec<Issue> {
        let mut out = Vec::new();
        count(&mut out, "campaigns", a.campaigns.len(), 1, 5, "campaigns");
        self.required_formats(&mut out, a);
        self.account_budget(&mut out, a);
        dup_names(
            &mut out,
            "campaigns",
            a.campaigns.iter().map(|c| c.name.as_str()),
        );
        let planned: usize = a.campaigns.iter().map(|c| c.planned_ad_groups.len()).sum();
        if planned > self.max_ad_groups {
            let msg = format!(
                "{planned} planned ad groups, limit is {}",
                self.max_ad_groups
            );
            out.push(Issue::error("E13", "campaigns", msg));
        }
        match &a.brand_kit {
            Some(kit) => out.extend(self.brand_kit(kit)),
            None if complete => out.push(Issue::error("E12", "brand_kit", "brand kit is missing")),
            None => {}
        }
        for (i, c) in a.campaigns.iter().enumerate() {
            out.extend(self.campaign(
                c,
                a.brand_kit.as_ref(),
                complete,
                &format!("campaigns[{i}]"),
            ));
        }
        out
    }

    fn required_formats(&self, out: &mut Vec<Issue>, a: &Account) {
        for f in self.formats {
            if !a.campaigns.iter().any(|c| c.kind == *f) {
                let msg = format!(
                    "business.toml asks for a {} campaign and the plan has none",
                    f.label()
                );
                out.push(Issue::error("E21", "campaigns", msg));
            }
        }
    }

    fn account_budget(&self, out: &mut Vec<Issue>, a: &Account) {
        if a.campaigns.is_empty() {
            return;
        }
        let sum: u64 = a.campaigns.iter().map(|c| c.daily_budget.0).sum();
        if sum != self.budget.daily.0 {
            let msg = format!(
                "campaign budgets sum to {sum} cents, budget.daily is {} cents",
                self.budget.daily.0
            );
            out.push(Issue::error("E06", "campaigns", msg));
        }
        for (i, c) in a.campaigns.iter().enumerate() {
            if c.daily_budget < MIN_CAMPAIGN_BUDGET {
                let msg = format!("daily budget {} cents is below 100", c.daily_budget.0);
                out.push(Issue::error(
                    "E06",
                    format!("campaigns[{i}].daily_budget"),
                    msg,
                ));
            }
        }
    }

    pub fn brand_kit(&self, kit: &BrandKit) -> Vec<Issue> {
        let mut out = Vec::new();
        count(
            &mut out,
            "brand_kit.headlines",
            kit.headlines.len(),
            8,
            12,
            "brand kit headlines",
        );
        count(
            &mut out,
            "brand_kit.descriptions",
            kit.descriptions.len(),
            2,
            3,
            "brand kit descriptions",
        );
        for (i, h) in kit.headlines.iter().enumerate() {
            self.headline(&mut out, &format!("brand_kit.headlines[{i}]"), h);
        }
        for (i, d) in kit.descriptions.iter().enumerate() {
            self.ad_text(
                &mut out,
                &format!("brand_kit.descriptions[{i}]"),
                d,
                DESCRIPTION,
            );
        }
        dups(&mut out, "brand_kit.headlines", &kit.headlines);
        dups(&mut out, "brand_kit.descriptions", &kit.descriptions);
        out
    }

    pub fn campaign(
        &self,
        c: &Campaign,
        kit: Option<&BrandKit>,
        complete: bool,
        path: &str,
    ) -> Vec<Issue> {
        let mut out = Vec::new();
        length(&mut out, &format!("{path}.name"), &c.name, NAME, "E01");
        self.bid_strategy(&mut out, c, path);
        self.kind_fit(&mut out, c, path);
        if c.kind.has_images() {
            self.image_campaign(&mut out, c, complete, path);
            return out;
        }
        self.structure(&mut out, c, complete, path);
        count(
            &mut out,
            &format!("{path}.negatives"),
            c.negatives.len(),
            0,
            100,
            "campaign negatives",
        );
        for (i, n) in c.negatives.iter().enumerate() {
            keyword_text(&mut out, &format!("{path}.negatives[{i}]"), &n.text);
        }
        if let Some(assets) = &c.assets {
            self.assets(&mut out, assets, &format!("{path}.assets"));
        }
        for (i, ag) in c.ad_groups.iter().enumerate() {
            out.extend(self.ad_group(ag, &c.negatives, kit, &format!("{path}.ad_groups[{i}]")));
        }
        same_keyword_in_two_groups(&mut out, c, path);
        if c.intent != Intent::Brand {
            one_word_phrases(&mut out, c, path);
        }
        out
    }

    fn bid_strategy(&self, out: &mut Vec<Issue>, c: &Campaign, path: &str) {
        let at = format!("{path}.bid_strategy");
        match c.bid_strategy {
            // App installs are tracked by the store, not by the site's conversion tag.
            BidStrategy::MaximizeConversions
                if !self.business.conversion_tracking && c.kind != CampaignKind::AppInstalls =>
            {
                out.push(Issue::error(
                    "E10",
                    at.clone(),
                    "maximize_conversions needs business.conversion_tracking = true",
                ));
            }
            BidStrategy::MaximizeClicks { max_cpc: Some(cap) } => self.cpc(out, &at, cap),
            _ => {}
        }
        let fits = match c.kind {
            CampaignKind::Search => c.bid_strategy == BidStrategy::ManualCpc,
            CampaignKind::PerformanceMax => c.bid_strategy == BidStrategy::MaximizeConversions,
            CampaignKind::DemandGen => c.bid_strategy != BidStrategy::ManualCpc,
            CampaignKind::AppInstalls => c.bid_strategy == BidStrategy::MaximizeConversions,
        };
        if !fits {
            let msg = format!(
                "{:?} does not fit a {} campaign",
                c.bid_strategy,
                c.kind.label()
            );
            out.push(Issue::error("E17", at, msg));
        }
    }

    fn cpc(&self, out: &mut Vec<Issue>, path: &str, cpc: Cents) {
        if cpc.0 == 0 {
            out.push(Issue::error("E11", path, "CPC must be greater than 0"));
        } else if self.budget.max_cpc.is_some_and(|cap| cpc > cap) {
            let cap = self.budget.max_cpc.map_or(0, |c| c.0);
            out.push(Issue::error(
                "E11",
                path,
                format!("CPC {} cents is above budget.max_cpc {cap} cents", cpc.0),
            ));
        }
    }

    fn structure(&self, out: &mut Vec<Issue>, c: &Campaign, complete: bool, path: &str) {
        dup_names(
            out,
            &format!("{path}.ad_groups"),
            c.ad_groups.iter().map(|g| g.name.as_str()),
        );
        let planned: BTreeSet<String> = c
            .planned_ad_groups
            .iter()
            .map(|p| normalize(&p.name))
            .collect();
        for (i, ag) in c.ad_groups.iter().enumerate() {
            if !planned.contains(&normalize(&ag.name)) {
                let msg = format!("ad group '{}' is not in the plan", ag.name);
                out.push(Issue::error(
                    "E12",
                    format!("{path}.ad_groups[{i}].name"),
                    msg,
                ));
            }
        }
        if !complete {
            return;
        }
        let built: BTreeSet<String> = c.ad_groups.iter().map(|g| normalize(&g.name)).collect();
        for p in c
            .planned_ad_groups
            .iter()
            .filter(|p| !built.contains(&normalize(&p.name)))
        {
            out.push(Issue::error(
                "E12",
                format!("{path}.ad_groups"),
                format!("planned ad group '{}' is missing", p.name),
            ));
        }
    }

    pub fn ad_group(
        &self,
        ag: &AdGroup,
        campaign_negatives: &[Keyword],
        kit: Option<&BrandKit>,
        path: &str,
    ) -> Vec<Issue> {
        let mut out = Vec::new();
        length(&mut out, &format!("{path}.name"), &ag.name, NAME, "E01");
        self.cpc(&mut out, &format!("{path}.default_cpc"), ag.default_cpc);
        self.url(&mut out, &format!("{path}.final_url"), &ag.final_url);
        self.focus_url(&mut out, &format!("{path}.final_url"), &ag.final_url);
        self.ad_group_keywords(&mut out, ag, campaign_negatives, path);
        self.rsa(&mut out, &ag.rsa, &format!("{path}.rsa"));
        if let Some(kit) = kit {
            let merged = merge_rsa(&ag.rsa, kit);
            if merged.headlines.len() < 15 || merged.descriptions.len() < 4 {
                let msg = format!(
                    "merged ad has {} headlines and {} descriptions (15 and 4 recommended)",
                    merged.headlines.len(),
                    merged.descriptions.len()
                );
                out.push(Issue::warning("W04", format!("{path}.rsa"), msg));
            }
        }
        out
    }

    fn ad_group_keywords(
        &self,
        out: &mut Vec<Issue>,
        ag: &AdGroup,
        campaign_negatives: &[Keyword],
        path: &str,
    ) {
        if ag.keywords.is_empty() {
            out.push(Issue::error(
                "E12",
                format!("{path}.keywords"),
                "ad group has no keywords",
            ));
        } else {
            count(
                out,
                &format!("{path}.keywords"),
                ag.keywords.len(),
                1,
                50,
                "keywords",
            );
        }
        count(
            out,
            &format!("{path}.negatives"),
            ag.negatives.len(),
            0,
            100,
            "ad group negatives",
        );
        for (i, k) in ag.keywords.iter().enumerate() {
            let at = format!("{path}.keywords[{i}]");
            keyword_text(out, &at, &k.text);
            self.focus_terms(out, &at, &k.text);
            let blocker = campaign_negatives
                .iter()
                .chain(&ag.negatives)
                .find(|n| blocks(n, k));
            if let Some(n) = blocker {
                out.push(Issue::error(
                    "E09",
                    at,
                    format!("negative '{}' blocks keyword '{}'", n.text, k.text),
                ));
            }
        }
        for (i, n) in ag.negatives.iter().enumerate() {
            keyword_text(out, &format!("{path}.negatives[{i}]"), &n.text);
        }
    }

    fn rsa(&self, out: &mut Vec<Issue>, rsa: &Rsa, path: &str) {
        count(
            out,
            &format!("{path}.headlines"),
            rsa.headlines.len(),
            3,
            7,
            "specific headlines",
        );
        count(
            out,
            &format!("{path}.descriptions"),
            rsa.descriptions.len(),
            1,
            2,
            "specific descriptions",
        );
        for (i, h) in rsa.headlines.iter().enumerate() {
            self.headline(out, &format!("{path}.headlines[{i}]"), h);
        }
        for (i, d) in rsa.descriptions.iter().enumerate() {
            self.ad_text(out, &format!("{path}.descriptions[{i}]"), d, DESCRIPTION);
        }
        dups(out, &format!("{path}.headlines"), &rsa.headlines);
        dups(out, &format!("{path}.descriptions"), &rsa.descriptions);
        if let Some(p1) = &rsa.path1 {
            length(out, &format!("{path}.path1"), p1, PATH, "E01");
        }
        if let Some(p2) = &rsa.path2 {
            length(out, &format!("{path}.path2"), p2, PATH, "E01");
            if rsa.path1.is_none() {
                out.push(Issue::error(
                    "E14",
                    format!("{path}.path2"),
                    "path2 requires path1",
                ));
            }
        }
    }

    fn assets(&self, out: &mut Vec<Issue>, a: &Assets, path: &str) {
        count(
            out,
            &format!("{path}.sitelinks"),
            a.sitelinks.len(),
            2,
            8,
            "sitelinks",
        );
        count(
            out,
            &format!("{path}.callouts"),
            a.callouts.len(),
            2,
            10,
            "callouts",
        );
        count(
            out,
            &format!("{path}.snippets"),
            a.snippets.len(),
            0,
            2,
            "structured snippets",
        );
        for (i, s) in a.sitelinks.iter().enumerate() {
            let at = format!("{path}.sitelinks[{i}]");
            self.ad_text(out, &format!("{at}.text"), &s.text, SITELINK_TEXT);
            for (n, d) in [(1, &s.description1), (2, &s.description2)] {
                if let Some(d) = d {
                    self.ad_text(out, &format!("{at}.description{n}"), d, SITELINK_DESC);
                }
            }
            if s.description1.is_some() != s.description2.is_some() {
                out.push(Issue::error(
                    "E14",
                    at.clone(),
                    "a sitelink needs both descriptions or none",
                ));
            }
            self.url(out, &format!("{at}.url"), &s.url);
        }
        let sitelink_texts: Vec<String> = a.sitelinks.iter().map(|s| s.text.clone()).collect();
        dups(out, &format!("{path}.sitelinks"), &sitelink_texts);
        for (i, c) in a.callouts.iter().enumerate() {
            self.ad_text(out, &format!("{path}.callouts[{i}]"), c, CALLOUT);
        }
        dups(out, &format!("{path}.callouts"), &a.callouts);
        for (i, sn) in a.snippets.iter().enumerate() {
            let at = format!("{path}.snippets[{i}]");
            count(
                out,
                &format!("{at}.values"),
                sn.values.len(),
                3,
                10,
                "snippet values",
            );
            for (j, v) in sn.values.iter().enumerate() {
                self.ad_text(out, &format!("{at}.values[{j}]"), v, SNIPPET_VALUE);
            }
            dups(out, &format!("{at}.values"), &sn.values);
        }
        if a.sitelinks.len() < 4 || a.callouts.len() < 4 || a.snippets.is_empty() {
            out.push(Issue::warning(
                "W05",
                path,
                "recommended: 4+ sitelinks, 4+ callouts and 1 structured snippet",
            ));
        }
    }

    fn headline(&self, out: &mut Vec<Issue>, path: &str, text: &str) {
        if text.contains('!') {
            out.push(Issue::error("E04", path, "headlines cannot contain '!'"));
        }
        self.ad_text(out, path, text, HEADLINE);
    }

    /// Length, emptiness, avoid terms and policy warnings for text a user will read.
    fn ad_text(&self, out: &mut Vec<Issue>, path: &str, text: &str, limit: usize) {
        if !length(out, path, text, limit, "E01") {
            return;
        }
        let norm = normalize(text);
        if let Some(term) = self
            .avoid
            .iter()
            .find(|t| !t.is_empty() && norm.contains(t.as_str()))
        {
            out.push(Issue::error(
                "E05",
                path,
                format!("text contains avoided term '{term}'"),
            ));
        }
        if let Some(term) = self
            .third_party_terms
            .iter()
            .find(|t| contains_word_sequence(&norm, t))
        {
            out.push(Issue::warning(
                "W01",
                path,
                format!("third-party term '{term}' in ad text (trademark policy risk)"),
            ));
        }
        if self.shouts(text) {
            out.push(Issue::warning("W02", path, "word in all caps"));
        }
    }

    fn shouts(&self, text: &str) -> bool {
        text.split(|c: char| !c.is_alphabetic()).any(|w| {
            w.chars().count() >= 4
                && w.chars().all(char::is_uppercase)
                && !self.brand_words.contains(&normalize(w))
        })
    }

    fn url(&self, out: &mut Vec<Issue>, path: &str, url: &str) {
        if !self.allowed.contains(url) {
            out.push(Issue::error(
                "E07",
                path,
                format!("URL not in business.url, business.pages or catalog: {url}"),
            ));
        }
    }
}

fn count(out: &mut Vec<Issue>, path: &str, n: usize, min: usize, max: usize, what: &str) {
    if n < min || n > max {
        out.push(Issue::error(
            "E13",
            path,
            format!("{what}: {n} found, expected {min} to {max}"),
        ));
    }
}

/// E02 for empty text, `code` (E01) for text over the limit. Returns true when the text is usable.
fn length(out: &mut Vec<Issue>, path: &str, text: &str, limit: usize, code: &str) -> bool {
    if text.trim().is_empty() {
        out.push(Issue::error("E02", path, "text is empty"));
        return false;
    }
    let n = char_len(text.trim());
    if n > limit {
        out.push(Issue::error(
            code,
            path,
            format!("{n} chars, limit is {limit}"),
        ));
        return false;
    }
    true
}

fn keyword_text(out: &mut Vec<Issue>, path: &str, text: &str) {
    if !length(out, path, text, KEYWORD, "E01") {
        return;
    }
    let words = text.split_whitespace().count();
    if words > KEYWORD_WORDS {
        out.push(Issue::error(
            "E08",
            path,
            format!("{words} words, limit is {KEYWORD_WORDS}"),
        ));
    }
    if let Some(c) = text.chars().find(|c| KEYWORD_FORBIDDEN.contains(*c)) {
        out.push(Issue::error(
            "E08",
            path,
            format!("character '{c}' is not allowed in keywords"),
        ));
    }
}

fn dups(out: &mut Vec<Issue>, path: &str, items: &[String]) {
    let mut first_seen = std::collections::BTreeMap::new();
    for (i, t) in items.iter().enumerate() {
        let key = normalize(t);
        if key.is_empty() {
            continue;
        }
        if let Some(prev) = first_seen.insert(key, i) {
            out.push(Issue::error(
                "E03",
                format!("{path}[{i}]"),
                format!("duplicate of [{prev}]"),
            ));
            first_seen.insert(normalize(t), prev);
        }
    }
}

fn dup_names<'n>(out: &mut Vec<Issue>, path: &str, names: impl Iterator<Item = &'n str>) {
    let mut seen = BTreeSet::new();
    for (i, n) in names.enumerate() {
        if !seen.insert(normalize(n)) {
            out.push(Issue::error(
                "E12",
                format!("{path}[{i}].name"),
                format!("duplicate name '{n}'"),
            ));
        }
    }
}

/// A negative blocks a keyword when its words appear contiguously (phrase) or the text is equal (exact).
pub(crate) fn blocks(neg: &Keyword, kw: &Keyword) -> bool {
    let (n, k) = (normalize(&neg.text), normalize(&kw.text));
    match neg.match_type {
        MatchType::Exact => n == k,
        MatchType::Phrase => contains_word_sequence(&k, &n),
    }
}

/// W09: a one-word phrase keyword matches any search that has the word, whatever else it says.
fn one_word_phrases(out: &mut Vec<Issue>, c: &Campaign, path: &str) {
    for (g, ag) in c.ad_groups.iter().enumerate() {
        for (i, k) in ag.keywords.iter().enumerate() {
            if k.match_type == MatchType::Phrase && !k.text.trim().contains(' ') {
                let msg = format!(
                    "'{}' as phrase matches any search with that word: make it exact or add words",
                    k.text
                );
                out.push(Issue::warning(
                    "W09",
                    format!("{path}.ad_groups[{g}].keywords[{i}]"),
                    msg,
                ));
            }
        }
    }
}

/// E26: keyword variants are ways to write one search. Variants that share no word, prefix or
/// initials are different searches: one ad and one landing page cannot fit them all, and Google
/// rates such a group low quality. As a warning, agents ignored it on a live account.
pub fn variant_themes(out: &mut Vec<Issue>, path: &str, variants: &[String]) {
    let folded: Vec<String> = variants
        .iter()
        .map(|v| fold(v))
        .filter(|v| !v.is_empty())
        .collect();
    let themes = theme_count(&folded);
    if themes > 1 {
        let msg = format!(
            "variants look like {themes} different searches: keep the spellings of one search in variants, put a synonym in extra, and give another search its own ad group"
        );
        out.push(Issue::error("E26", path, msg));
    }
}

fn theme_count(items: &[String]) -> usize {
    let mut group: Vec<usize> = (0..items.len()).collect();
    for i in 0..items.len() {
        for j in 0..i {
            if related(&items[i], &items[j]) {
                let (from, to) = (group[i], group[j]);
                group
                    .iter_mut()
                    .filter(|g| **g == from)
                    .for_each(|g| *g = to);
            }
        }
    }
    group.iter().collect::<BTreeSet<_>>().len()
}

/// Two folded variants name the same thing: a shared word, one inside the other, the same
/// start, or one is the initials of the other.
fn related(a: &str, b: &str) -> bool {
    let words = |s: &str| -> BTreeSet<String> {
        s.split_whitespace()
            .filter(|w| w.chars().count() >= 3)
            .map(str::to_string)
            .collect()
    };
    let initials = |s: &str| -> String {
        let w: Vec<&str> = s.split_whitespace().collect();
        if w.len() < 2 {
            return String::new();
        }
        w.iter().filter_map(|w| w.chars().next()).collect()
    };
    let (ca, cb) = (a.replace(' ', ""), b.replace(' ', ""));
    let (short, long) = if ca.len() <= cb.len() {
        (&ca, &cb)
    } else {
        (&cb, &ca)
    };
    let prefix = |s: &str| s.chars().take(4).collect::<String>();
    !words(a).is_disjoint(&words(b))
        || (short.chars().count() >= 4 && long.contains(short.as_str()))
        || (short.chars().count() >= 4 && prefix(&ca) == prefix(&cb))
        || ca == initials(b)
        || cb == initials(a)
}

fn same_keyword_in_two_groups(out: &mut Vec<Issue>, c: &Campaign, path: &str) {
    let mut seen: std::collections::BTreeMap<(String, bool), usize> =
        std::collections::BTreeMap::new();
    for (g, ag) in c.ad_groups.iter().enumerate() {
        let own: BTreeSet<_> = ag
            .keywords
            .iter()
            .map(|k| (normalize(&k.text), k.match_type == MatchType::Exact))
            .collect();
        for key in own {
            if let Some(first) = seen.get(&key) {
                let msg = format!("keyword '{}' is also in ad_groups[{first}]", key.0);
                out.push(Issue::warning(
                    "W03",
                    format!("{path}.ad_groups[{g}].keywords"),
                    msg,
                ));
            } else {
                seen.insert(key, g);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::MatchType::{Exact, Phrase};
    use crate::google::*;
    use crate::input::{Budget, Business, CatalogItem, ExportConfig, ExportStatus, Input, Page};

    fn s(t: &str) -> String {
        t.to_string()
    }

    fn input() -> Input {
        Input {
            logo: None,
            formats: Vec::new(),
            design: String::new(),
            focus: None,
            app: None,
            business: Business {
                name: s("Vinellu"),
                url: s("https://vinellu.com"),
                language: s("pt-BR"),
                locations: vec![s("Brazil")],
                goal: s("cadastros"),
                description: s("App social de vinhos com reviews e safras."),
                conversion_tracking: false,
                restricted: vec![],
                brand_terms: vec![s("vinellu")],
                competitors: vec![s("Vivino")],
                avoid: vec![s("melhor do mundo")],
                pages: ["app", "sobre", "blog", "ajuda"]
                    .iter()
                    .map(|p| Page {
                        name: s(p),
                        url: format!("https://vinellu.com/{p}"),
                    })
                    .collect(),
            },
            budget: Budget {
                daily: Cents(5000),
                currency: s("BRL"),
                max_cpc: Some(Cents(300)),
            },
            export: ExportConfig {
                status: ExportStatus::Paused,
                url_suffix: s(""),
                eu_political_ads: false,
                decimal_comma: true,
            },
            research: String::new(),
            catalog: vec![CatalogItem {
                image: None,
                id: s("alamos"),
                name: s("Alamos Malbec"),
                url: s("https://vinellu.com/w/a"),
                category: s(""),
                aliases: vec![s("alamos")],
                third_party: true,
                notes: s(""),
            }],
        }
    }

    fn texts(prefix: &str, n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("{prefix} numero {i}")).collect()
    }

    fn kit() -> BrandKit {
        BrandKit {
            headlines: texts("Titulo da marca", 10),
            descriptions: texts("Descricao da marca com chamada pra acao", 3),
        }
    }

    fn kw(t: &str, m: MatchType) -> Keyword {
        Keyword {
            text: s(t),
            match_type: m,
        }
    }

    fn ad_group(name: &str) -> AdGroup {
        AdGroup {
            name: s(name),
            default_cpc: Cents(150),
            cpc_rationale: s("estimativa"),
            final_url: s("https://vinellu.com/w/a"),
            keywords: vec![kw("vinho tinto", Phrase), kw("vinho tinto", Exact)],
            negatives: vec![],
            rsa: Rsa {
                headlines: texts("Titulo do grupo", 5),
                descriptions: texts("Descricao do grupo para teste", 1),
                path1: Some(s("vinhos")),
                path2: None,
            },
        }
    }

    fn assets() -> Assets {
        Assets {
            sitelinks: ["app", "sobre", "blog", "ajuda"]
                .iter()
                .map(|p| Sitelink {
                    text: format!("Link {p}"),
                    description1: None,
                    description2: None,
                    url: format!("https://vinellu.com/{p}"),
                })
                .collect(),
            callouts: texts("Callout", 4),
            snippets: vec![Snippet {
                header: SnippetHeader::Types,
                values: texts("Tipo", 3),
            }],
        }
    }

    fn campaign(name: &str, budget: u64) -> Campaign {
        Campaign {
            kind: Default::default(),
            asset_groups: Vec::new(),
            name: s(name),
            slug: crate::input::slugify(name),
            intent: Intent::Catalog,
            daily_budget: Cents(budget),
            bid_strategy: BidStrategy::ManualCpc,
            rationale: s("r"),
            planned_ad_groups: vec![PlannedAdGroup {
                name: s("g1"),
                theme: s("t"),
                entity_ids: vec![],
                final_url: s("https://vinellu.com/w/a"),
            }],
            ad_groups: vec![ad_group("g1")],
            negatives: vec![],
            assets: Some(assets()),
        }
    }

    fn account() -> Account {
        Account {
            brand_kit: Some(kit()),
            campaigns: vec![campaign("Camp A", 5000)],
        }
    }

    fn run(a: &Account) -> Vec<Issue> {
        let inp = input();
        Rules::new(&inp, 50).account(a, true)
    }

    fn codes(issues: &[Issue]) -> Vec<String> {
        issues.iter().map(|i| i.code.clone()).collect()
    }

    fn assert_has(a: &Account, code: &str) {
        let issues = run(a);
        assert!(
            issues.iter().any(|i| i.code == code),
            "expected {code}, got {:?}",
            codes(&issues)
        );
    }

    #[test]
    fn good_fixture_has_no_issues() {
        assert_eq!(run(&account()), vec![]);
    }

    #[test]
    fn e01_headline_over_30_chars() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = "x".repeat(31);
        let issues = run(&a);
        let i = issues.iter().find(|i| i.code == "E01").unwrap();
        assert_eq!(i.path, "campaigns[0].ad_groups[0].rsa.headlines[0]");
        assert!(i.message.contains("31") && i.message.contains("30"));
    }

    #[test]
    fn e01_counts_accented_chars_once_in_nfc_and_nfd() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = "é".repeat(30);
        assert!(run(&a).is_empty());
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = "e\u{301}".repeat(30);
        assert!(run(&a).is_empty());
    }

    #[test]
    fn e01_other_limits() {
        for (limit_case, mutate) in [
            (
                "description",
                Box::new(|a: &mut Account| {
                    a.campaigns[0].ad_groups[0].rsa.descriptions[0] = "x".repeat(91)
                }) as Box<dyn Fn(&mut Account)>,
            ),
            (
                "path",
                Box::new(|a| a.campaigns[0].ad_groups[0].rsa.path1 = Some("x".repeat(16))),
            ),
            (
                "sitelink",
                Box::new(|a| {
                    a.campaigns[0].assets.as_mut().unwrap().sitelinks[0].text = "x".repeat(26)
                }),
            ),
            (
                "callout",
                Box::new(|a| a.campaigns[0].assets.as_mut().unwrap().callouts[0] = "x".repeat(26)),
            ),
            (
                "snippet",
                Box::new(|a| {
                    a.campaigns[0].assets.as_mut().unwrap().snippets[0].values[0] = "x".repeat(26)
                }),
            ),
            (
                "keyword",
                Box::new(|a| a.campaigns[0].ad_groups[0].keywords[0].text = "x".repeat(81)),
            ),
            ("name", Box::new(|a| a.campaigns[0].name = "x".repeat(256))),
        ] {
            let mut a = account();
            mutate(&mut a);
            assert!(run(&a).iter().any(|i| i.code == "E01"), "case {limit_case}");
        }
    }

    #[test]
    fn e02_empty_text() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[1] = s("   ");
        assert_has(&a, "E02");
    }

    #[test]
    fn e03_duplicate_headline_ignores_case() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[1] =
            a.campaigns[0].ad_groups[0].rsa.headlines[0].to_uppercase();
        assert_has(&a, "E03");
    }

    #[test]
    fn e03_duplicate_in_brand_kit_and_callouts() {
        let mut a = account();
        a.brand_kit.as_mut().unwrap().descriptions[1] =
            a.brand_kit.as_ref().unwrap().descriptions[0].clone();
        assert_has(&a, "E03");
        let mut a = account();
        a.campaigns[0].assets.as_mut().unwrap().callouts[1] =
            a.campaigns[0].assets.as_ref().unwrap().callouts[0].clone();
        assert_has(&a, "E03");
    }

    #[test]
    fn e04_exclamation_in_headline_but_not_in_description() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = s("Compre agora!");
        assert_has(&a, "E04");
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.descriptions[0] = s("Baixe grátis agora mesmo!");
        assert!(run(&a).is_empty());
    }

    #[test]
    fn e05_avoid_term_in_ad_text() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.descriptions[0] = s("O MELHOR  do mundo em vinhos");
        assert_has(&a, "E05");
    }

    #[test]
    fn e06_budget_must_sum_to_daily_and_each_at_least_one() {
        let mut a = account();
        a.campaigns[0].daily_budget = Cents(4000);
        assert_has(&a, "E06");
        let mut a = account();
        a.campaigns = vec![campaign("Camp A", 4950), campaign("Camp B", 50)];
        assert_has(&a, "E06");
    }

    #[test]
    fn e07_url_outside_allowed_set() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].final_url = s("https://evil.com/x");
        assert_has(&a, "E07");
        let mut a = account();
        a.campaigns[0].assets.as_mut().unwrap().sitelinks[0].url =
            s("https://vinellu.com/inventada");
        assert_has(&a, "E07");
    }

    #[test]
    fn e08_keyword_with_forbidden_char_or_too_many_words() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].keywords[0].text = s("vinho (tinto)");
        assert_has(&a, "E08");
        let mut a = account();
        a.campaigns[0].ad_groups[0].keywords[0].text = s("a b c d e f g h i j k");
        assert_has(&a, "E08");
    }

    #[test]
    fn e09_phrase_negative_blocks_contiguous_words_only() {
        let mut a = account();
        a.campaigns[0].negatives = vec![kw("tinto", Phrase)];
        assert_has(&a, "E09");
        let mut a = account();
        a.campaigns[0].negatives = vec![kw("tinto vinho", Phrase)];
        assert!(run(&a).is_empty());
    }

    #[test]
    fn e09_exact_negative_blocks_only_equal_text_and_ad_group_scope_is_local() {
        let mut a = account();
        a.campaigns[0].negatives = vec![kw("vinho", Exact)];
        assert!(run(&a).is_empty());
        a.campaigns[0].negatives = vec![kw("vinho tinto", Exact)];
        assert_has(&a, "E09");
        let mut a = account();
        a.campaigns[0].ad_groups.push(ad_group("g2"));
        a.campaigns[0].planned_ad_groups.push(PlannedAdGroup {
            name: s("g2"),
            theme: s("t"),
            entity_ids: vec![],
            final_url: s("https://vinellu.com/w/a"),
        });
        a.campaigns[0].ad_groups[1].keywords = vec![kw("queijo", Phrase)];
        a.campaigns[0].ad_groups[0].negatives = vec![kw("queijo", Phrase)];
        assert!(run(&a).iter().all(|i| i.code != "E09"));
    }

    #[test]
    fn e10_maximize_conversions_needs_conversion_tracking() {
        let mut a = account();
        a.campaigns[0].bid_strategy = BidStrategy::MaximizeConversions;
        assert_has(&a, "E10");
        let mut inp = input();
        inp.business.conversion_tracking = true;
        assert!(
            Rules::new(&inp, 50)
                .account(&a, true)
                .iter()
                .all(|i| i.code != "E10")
        );
    }

    #[test]
    fn e11_cpc_zero_or_above_cap() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].default_cpc = Cents(0);
        assert_has(&a, "E11");
        let mut a = account();
        a.campaigns[0].ad_groups[0].default_cpc = Cents(301);
        assert_has(&a, "E11");
        let mut a = account();
        a.campaigns[0].bid_strategy = BidStrategy::MaximizeClicks {
            max_cpc: Some(Cents(400)),
        };
        assert_has(&a, "E11");
    }

    #[test]
    fn e12_structure() {
        let mut a = account();
        a.campaigns.push(campaign("camp a", 0));
        assert_has(&a, "E12");
        let mut a = account();
        a.campaigns[0].ad_groups.push(ad_group("g1"));
        assert_has(&a, "E12");
        let mut a = account();
        a.campaigns[0].ad_groups[0].name = s("nao planejado");
        assert_has(&a, "E12");
        let mut a = account();
        a.campaigns[0].ad_groups.clear();
        assert_has(&a, "E12");
        let mut a = account();
        a.campaigns[0].ad_groups[0].keywords.clear();
        assert_has(&a, "E12");
    }

    #[test]
    fn missing_planned_ad_group_only_matters_when_complete() {
        let mut a = account();
        a.campaigns[0].ad_groups.clear();
        let inp = input();
        assert!(
            Rules::new(&inp, 50)
                .account(&a, false)
                .iter()
                .all(|i| i.code != "E12")
        );
    }

    #[test]
    fn e13_counts() {
        let mut a = account();
        a.brand_kit.as_mut().unwrap().headlines.truncate(7);
        assert_has(&a, "E13");
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines = texts("T", 2);
        assert_has(&a, "E13");
        let mut a = account();
        a.campaigns[0]
            .assets
            .as_mut()
            .unwrap()
            .sitelinks
            .truncate(1);
        assert_has(&a, "E13");
        let mut a = account();
        a.campaigns[0].assets.as_mut().unwrap().snippets[0]
            .values
            .truncate(2);
        assert_has(&a, "E13");
        let mut a = account();
        a.campaigns = (0..6)
            .map(|i| campaign(&format!("C{i}"), 5000 / 6))
            .collect();
        assert_has(&a, "E13");
    }

    #[test]
    fn e14_path2_without_path1_and_single_sitelink_description() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.path1 = None;
        a.campaigns[0].ad_groups[0].rsa.path2 = Some(s("x"));
        assert_has(&a, "E14");
        let mut a = account();
        a.campaigns[0].assets.as_mut().unwrap().sitelinks[0].description1 = Some(s("so uma"));
        assert_has(&a, "E14");
    }

    #[test]
    fn w01_third_party_term_in_ad_text_is_warning_only() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = s("Alamos Malbec vale a pena?");
        let issues = run(&a);
        let w = issues.iter().find(|i| i.code == "W01").unwrap();
        assert!(!w.is_error());
        assert!(issues.iter().all(|i| !i.is_error()));
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = s("Vivino é bom?");
        assert!(run(&a).iter().any(|i| i.code == "W01"));
    }

    #[test]
    fn w02_shouting_word_but_not_brand_term() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = s("COMPRE agora");
        assert_has(&a, "W02");
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = s("VINELLU é bom");
        assert!(run(&a).iter().all(|i| i.code != "W02"));
    }

    #[test]
    fn w03_same_keyword_in_two_ad_groups() {
        let mut a = account();
        a.campaigns[0].ad_groups.push(ad_group("g2"));
        a.campaigns[0].planned_ad_groups.push(PlannedAdGroup {
            name: s("g2"),
            theme: s("t"),
            entity_ids: vec![],
            final_url: s("https://vinellu.com/w/a"),
        });
        assert_has(&a, "W03");
    }

    #[test]
    fn e26_variants_of_different_searches() {
        let cases: &[(&[&str], bool)] = &[
            (&["malbec", "cabernet sauvignon", "merlot"], true),
            (&["vinho para peixe", "harmonizacao churrasco"], true),
            (
                &["cabernet sauvignon", "Cabernét", "cab sauvignon", "cs"],
                false,
            ),
            (&["vinellu", "vinelu", "vinellu app"], false),
            (&["São Paulo", "sao paulo", "sp"], false),
            (&["alamos malbec"], false),
        ];
        for (variants, warns) in cases {
            let v: Vec<String> = variants.iter().map(|s| s.to_string()).collect();
            let mut out = Vec::new();
            variant_themes(&mut out, "keywords.variants", &v);
            assert_eq!(!out.is_empty(), *warns, "{variants:?}");
            assert!(out.iter().all(|i| i.code == "E26" && i.is_error()));
        }
    }

    #[test]
    fn w09_one_word_phrase_keyword_outside_brand() {
        let mut a = account();
        a.campaigns[0].ad_groups[0]
            .keywords
            .push(kw("malbec", Phrase));
        assert_has(&a, "W09");
        a.campaigns[0].intent = Intent::Brand;
        assert!(run(&a).iter().all(|i| i.code != "W09"));
        a.campaigns[0].intent = Intent::Generic;
        a.campaigns[0].ad_groups[0].keywords.pop();
        a.campaigns[0].ad_groups[0]
            .keywords
            .push(kw("malbec", Exact));
        assert!(run(&a).iter().all(|i| i.code != "W09"));
    }

    #[test]
    fn w04_merged_rsa_short() {
        let mut a = account();
        a.brand_kit.as_mut().unwrap().headlines.truncate(8);
        assert_has(&a, "W04");
    }

    #[test]
    fn w05_few_assets() {
        let mut a = account();
        a.campaigns[0].assets.as_mut().unwrap().callouts.truncate(2);
        assert_has(&a, "W05");
    }

    #[test]
    fn every_issue_has_a_path_and_message() {
        let mut a = account();
        a.campaigns[0].ad_groups[0].rsa.headlines[0] = "x".repeat(40);
        for i in run(&a) {
            assert!(!i.path.is_empty() && !i.message.is_empty());
        }
    }

    #[test]
    fn max_ad_groups_is_enforced_across_the_plan() {
        let a = account();
        let inp = input();
        let issues = Rules::new(&inp, 0).account(&a, true);
        assert!(issues.iter().any(|i| i.code == "E13"));
    }
}
