//! Rules for Performance Max and Demand Gen campaigns (image campaigns spec, section 5).

use std::collections::BTreeSet;

use super::{NAME, Rules, count, dups, length};
use crate::{
    google::{AspectRatio, AssetGroup, Campaign, CampaignKind, ImageBrief, Issue, normalize},
    input::slugify,
};

const BUSINESS_NAME: usize = 25;
const LONG_HEADLINE: usize = 90;
const DESCRIPTION: usize = 90;
const SHORT_DESCRIPTION: usize = 60;
const SEARCH_THEME: usize = 80;
const DEMAND_GEN_HEADLINE: usize = 40;
const MAX_IMAGES: usize = 20;
const PROMPT_MIN: usize = 20;
const PROMPT_MAX: usize = 1500;
/// Words that ask the model to draw text. Google overlays the ad text itself.
const TEXT_WORDS: &[&str] = &[
    "text",
    "logo",
    "caption",
    "headline",
    "words",
    "lettering",
    "typography",
    "slogan",
    "label that reads",
];

/// Counts and lengths of the texts, per kind: (min, max, max chars).
struct TextLimits {
    headlines: (usize, usize, usize),
    long_headlines: (usize, usize),
    descriptions: (usize, usize),
    search_themes: usize,
}

fn limits(kind: CampaignKind) -> TextLimits {
    match kind {
        CampaignKind::DemandGen => TextLimits {
            headlines: (1, 5, DEMAND_GEN_HEADLINE),
            long_headlines: (0, 0),
            descriptions: (1, 5),
            search_themes: 0,
        },
        _ => TextLimits {
            headlines: (3, 15, 30),
            long_headlines: (1, 5),
            descriptions: (2, 5),
            search_themes: 25,
        },
    }
}

fn ratio_count(images: &[ImageBrief], r: AspectRatio) -> usize {
    images.iter().filter(|b| b.ratio == r).count()
}

impl Rules<'_> {
    /// Parts of a campaign that belong to the other kind.
    pub(super) fn kind_fit(&self, out: &mut Vec<Issue>, c: &Campaign, path: &str) {
        let wrong = |what: &str| format!("a {} campaign has no {what}", c.kind.label());
        if c.kind.has_images() {
            if !c.ad_groups.is_empty() {
                out.push(Issue::error(
                    "E17",
                    format!("{path}.ad_groups"),
                    wrong("ad groups"),
                ));
            }
            if !c.negatives.is_empty() {
                out.push(Issue::error(
                    "E17",
                    format!("{path}.negatives"),
                    wrong("negatives"),
                ));
            }
            if c.assets.is_some() {
                out.push(Issue::error(
                    "E17",
                    format!("{path}.assets"),
                    wrong("assets"),
                ));
            }
            if !self.has_logo {
                let msg = "image campaigns need a logo: set [brand] logo in business.toml";
                out.push(Issue::error("E18", path, msg));
            }
        } else if !c.asset_groups.is_empty() {
            out.push(Issue::error(
                "E17",
                format!("{path}.asset_groups"),
                wrong("asset groups"),
            ));
        }
    }

    pub(super) fn image_campaign(
        &self,
        out: &mut Vec<Issue>,
        c: &Campaign,
        complete: bool,
        path: &str,
    ) {
        let planned: BTreeSet<String> = c
            .planned_ad_groups
            .iter()
            .map(|p| normalize(&p.name))
            .collect();
        let mut seen = BTreeSet::new();
        for (i, g) in c.asset_groups.iter().enumerate() {
            let at = format!("{path}.asset_groups[{i}]");
            if !planned.contains(&normalize(&g.name)) {
                let msg = format!("asset group '{}' is not in the plan", g.name);
                out.push(Issue::error("E12", format!("{at}.name"), msg));
            }
            if !seen.insert(normalize(&g.name)) {
                let msg = format!("duplicate name '{}'", g.name);
                out.push(Issue::error("E12", format!("{at}.name"), msg));
            }
            self.asset_group(out, c.kind, g, complete, &at);
        }
        if complete {
            for p in c
                .planned_ad_groups
                .iter()
                .filter(|p| !seen.contains(&normalize(&p.name)))
            {
                let msg = format!("planned asset group '{}' is missing", p.name);
                out.push(Issue::error("E12", format!("{path}.asset_groups"), msg));
            }
        }
    }

    pub fn asset_group(
        &self,
        out: &mut Vec<Issue>,
        kind: CampaignKind,
        g: &AssetGroup,
        complete: bool,
        path: &str,
    ) {
        length(out, &format!("{path}.name"), &g.name, NAME, "E01");
        self.url(out, &format!("{path}.final_url"), &g.final_url);
        self.focus_url(out, &format!("{path}.final_url"), &g.final_url);
        self.ad_text(
            out,
            &format!("{path}.business_name"),
            &g.business_name,
            BUSINESS_NAME,
        );
        self.asset_texts(out, kind, g, path);
        self.briefs(out, kind, &g.images, complete, &format!("{path}.images"));
    }

    fn asset_texts(&self, out: &mut Vec<Issue>, kind: CampaignKind, g: &AssetGroup, path: &str) {
        self.asset_headlines(out, kind, g, path);
        self.asset_descriptions(out, kind, g, path);
        let themes = &g.search_themes;
        let at = format!("{path}.search_themes");
        count(
            out,
            &at,
            themes.len(),
            0,
            limits(kind).search_themes,
            "search themes",
        );
        for (i, t) in themes.iter().enumerate() {
            length(out, &format!("{at}[{i}]"), t, SEARCH_THEME, "E01");
            self.focus_terms(out, &format!("{at}[{i}]"), t);
        }
        dups(out, &at, themes);
        dups(out, &format!("{path}.descriptions"), &g.descriptions);
    }

    fn asset_headlines(
        &self,
        out: &mut Vec<Issue>,
        kind: CampaignKind,
        g: &AssetGroup,
        path: &str,
    ) {
        let l = limits(kind);
        let (hmin, hmax, hlen) = l.headlines;
        let at = format!("{path}.headlines");
        count(out, &at, g.headlines.len(), hmin, hmax, "headlines");
        for (i, h) in g.headlines.iter().enumerate() {
            let here = format!("{at}[{i}]");
            if kind == CampaignKind::DemandGen {
                self.ad_text(out, &here, h, hlen);
            } else {
                self.headline(out, &here, h);
            }
        }
        dups(out, &at, &g.headlines);
        let (lmin, lmax) = l.long_headlines;
        let at = format!("{path}.long_headlines");
        count(
            out,
            &at,
            g.long_headlines.len(),
            lmin,
            lmax,
            "long headlines",
        );
        for (i, h) in g.long_headlines.iter().enumerate() {
            self.ad_text(out, &format!("{at}[{i}]"), h, LONG_HEADLINE);
        }
        dups(out, &at, &g.long_headlines);
    }

    fn asset_descriptions(
        &self,
        out: &mut Vec<Issue>,
        kind: CampaignKind,
        g: &AssetGroup,
        path: &str,
    ) {
        let (dmin, dmax) = limits(kind).descriptions;
        let at = format!("{path}.descriptions");
        count(out, &at, g.descriptions.len(), dmin, dmax, "descriptions");
        for (i, d) in g.descriptions.iter().enumerate() {
            self.ad_text(out, &format!("{at}[{i}]"), d, DESCRIPTION);
        }
        let short = g
            .descriptions
            .iter()
            .any(|d| super::char_len(d.trim()) <= SHORT_DESCRIPTION);
        if kind == CampaignKind::PerformanceMax && !g.descriptions.is_empty() && !short {
            let msg = format!("one description must have {SHORT_DESCRIPTION} characters or fewer");
            out.push(Issue::error("E13", at, msg));
        }
    }

    fn briefs(
        &self,
        out: &mut Vec<Issue>,
        kind: CampaignKind,
        images: &[ImageBrief],
        complete: bool,
        path: &str,
    ) {
        count(out, path, images.len(), 0, MAX_IMAGES, "images");
        if complete || !images.is_empty() {
            ratio_minimum(out, kind, images, path);
        }
        let mut ids = BTreeSet::new();
        for (i, b) in images.iter().enumerate() {
            let at = format!("{path}[{i}]");
            self.brief(out, kind, b, &at);
            if !ids.insert(b.id.as_str()) {
                out.push(Issue::error(
                    "E16",
                    format!("{at}.id"),
                    format!("duplicate id '{}'", b.id),
                ));
            }
            if complete && b.file.is_none() {
                out.push(Issue::error(
                    "E20",
                    at,
                    format!("image '{}' has no file", b.id),
                ));
            }
        }
        recommended(out, kind, images, path);
    }

    fn brief(&self, out: &mut Vec<Issue>, kind: CampaignKind, b: &ImageBrief, at: &str) {
        if b.id.is_empty() || slugify(&b.id) != b.id {
            let msg = format!("id '{}' must be a slug such as wine-on-table", b.id);
            out.push(Issue::error("E16", format!("{at}.id"), msg));
        }
        if kind == CampaignKind::PerformanceMax && b.ratio == AspectRatio::Vertical {
            let msg = "Performance Max takes no vertical (9:16) image";
            out.push(Issue::error("E16", format!("{at}.ratio"), msg));
        }
        let n = super::char_len(b.prompt.trim());
        if !(PROMPT_MIN..=PROMPT_MAX).contains(&n) {
            let msg = format!("prompt has {n} chars, expected {PROMPT_MIN} to {PROMPT_MAX}");
            out.push(Issue::error("E16", format!("{at}.prompt"), msg));
        }
        let words = normalize(&b.prompt);
        if let Some(w) = TEXT_WORDS
            .iter()
            .find(|w| super::contains_word_sequence(&words, w))
        {
            let msg =
                format!("prompt mentions '{w}': Google adds the text, keep the picture clean");
            out.push(Issue::warning("W07", format!("{at}.prompt"), msg));
        }
        if let Some(id) = &b.reference {
            match self.catalog_photos.get(id) {
                Some(true) => {}
                Some(false) => {
                    let msg = format!("catalog item '{id}' has no image to use as reference");
                    out.push(Issue::error("E19", format!("{at}.reference"), msg));
                }
                None => {
                    let msg = format!("unknown catalog id '{id}'");
                    out.push(Issue::error("E19", format!("{at}.reference"), msg));
                }
            }
        }
    }
}

fn ratio_minimum(out: &mut Vec<Issue>, kind: CampaignKind, images: &[ImageBrief], path: &str) {
    let (land, square) = (
        ratio_count(images, AspectRatio::Landscape),
        ratio_count(images, AspectRatio::Square),
    );
    let missing = match kind {
        CampaignKind::PerformanceMax if land == 0 || square == 0 => {
            Some("Performance Max needs at least 1 landscape and 1 square image")
        }
        CampaignKind::DemandGen if land + square == 0 => {
            Some("Demand Gen needs at least 1 landscape or square image")
        }
        _ => None,
    };
    if let Some(msg) = missing {
        out.push(Issue::error("E16", path, msg));
    }
}

fn recommended(out: &mut Vec<Issue>, kind: CampaignKind, images: &[ImageBrief], path: &str) {
    let want: &[(AspectRatio, usize)] = match kind {
        CampaignKind::PerformanceMax => &[
            (AspectRatio::Landscape, 4),
            (AspectRatio::Square, 4),
            (AspectRatio::Portrait, 2),
        ],
        _ => &[
            (AspectRatio::Landscape, 1),
            (AspectRatio::Square, 1),
            (AspectRatio::Portrait, 1),
        ],
    };
    let short: Vec<String> = want
        .iter()
        .filter(|(r, n)| ratio_count(images, *r) < *n)
        .map(|(r, n)| format!("{n} {r:?}"))
        .collect();
    if !images.is_empty() && !short.is_empty() {
        let msg = format!(
            "recommended for ad strength: {}",
            short.join(", ").to_lowercase()
        );
        out.push(Issue::warning("W06", path, msg));
    }
}

#[cfg(test)]
mod tests {
    use crate::google::*;
    use crate::input::Input;

    fn input(logo: bool) -> Input {
        let mut i = crate::testutil::input();
        i.logo = logo.then(|| "/tmp/logo.png".to_string());
        i.catalog[0].image = Some("https://cdn.vinellu.com/alamos.jpg".into());
        i
    }

    fn texts(prefix: &str, n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("{prefix} {i}")).collect()
    }

    fn brief(id: &str, ratio: AspectRatio) -> ImageBrief {
        ImageBrief {
            id: id.into(),
            ratio,
            prompt: "A glass of red wine on a wooden table, warm evening light".into(),
            reference: None,
            file: Some(format!("images/x/{id}.jpg")),
        }
    }

    fn group() -> AssetGroup {
        AssetGroup {
            name: "alamos".into(),
            final_url: "https://vinellu.com/w/alamos".into(),
            business_name: "Vinellu".into(),
            headlines: texts("Titulo", 5),
            long_headlines: texts("Titulo longo do grupo", 2),
            descriptions: vec!["Descricao curta".into(), "Outra descricao do grupo".into()],
            search_themes: vec!["vinho alamos".into()],
            images: vec![
                brief("mesa", AspectRatio::Landscape),
                brief("taca", AspectRatio::Square),
            ],
        }
    }

    fn campaign(kind: CampaignKind) -> Campaign {
        let bid = match kind {
            CampaignKind::Search => BidStrategy::ManualCpc,
            CampaignKind::PerformanceMax => BidStrategy::MaximizeConversions,
            CampaignKind::DemandGen => BidStrategy::MaximizeClicks { max_cpc: None },
        };
        Campaign {
            name: "Vinellu - Imagem".into(),
            slug: "vinellu-imagem".into(),
            kind,
            intent: Intent::Catalog,
            daily_budget: Cents(5000),
            bid_strategy: bid,
            rationale: "r".into(),
            planned_ad_groups: vec![PlannedAdGroup {
                name: "alamos".into(),
                theme: "t".into(),
                entity_ids: vec!["alamos-malbec".into()],
                final_url: "https://vinellu.com/w/alamos".into(),
            }],
            ad_groups: vec![],
            asset_groups: vec![group()],
            negatives: vec![],
            assets: None,
        }
    }

    fn codes(inp: &Input, c: &Campaign) -> Vec<String> {
        let mut inp = inp.clone();
        inp.business.conversion_tracking = true;
        Rules::new(&inp, 50)
            .campaign(c, None, true, "c")
            .into_iter()
            .filter(Issue::is_error)
            .map(|i| i.code)
            .collect()
    }

    fn warnings(c: &Campaign) -> Vec<String> {
        Rules::new(&input(true), 50)
            .campaign(c, None, true, "c")
            .into_iter()
            .filter(|i| !i.is_error())
            .map(|i| i.code)
            .collect()
    }

    #[test]
    fn a_valid_pmax_and_demand_gen_campaign_has_no_errors() {
        assert!(codes(&input(true), &campaign(CampaignKind::PerformanceMax)).is_empty());
        let mut dg = campaign(CampaignKind::DemandGen);
        dg.asset_groups[0].long_headlines.clear();
        dg.asset_groups[0].search_themes.clear();
        assert!(
            codes(&input(true), &dg).is_empty(),
            "{:?}",
            codes(&input(true), &dg)
        );
    }

    #[test]
    fn e16_pmax_needs_landscape_and_square_and_no_vertical() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].images = vec![brief("mesa", AspectRatio::Landscape)];
        assert!(codes(&input(true), &c).contains(&"E16".to_string()));
        c.asset_groups[0].images = vec![
            brief("mesa", AspectRatio::Landscape),
            brief("taca", AspectRatio::Square),
            brief("story", AspectRatio::Vertical),
        ];
        assert!(codes(&input(true), &c).contains(&"E16".to_string()));
    }

    #[test]
    fn e16_ids_are_unique_slugs_and_prompts_have_a_length() {
        for edit in [
            |b: &mut ImageBrief| b.id = "Mesa Posta".into(),
            |b: &mut ImageBrief| b.id = "taca".into(),
            |b: &mut ImageBrief| b.prompt = "short".into(),
        ] {
            let mut c = campaign(CampaignKind::PerformanceMax);
            edit(&mut c.asset_groups[0].images[0]);
            assert!(codes(&input(true), &c).contains(&"E16".to_string()));
        }
    }

    #[test]
    fn e17_the_bid_and_the_parts_must_fit_the_kind() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.bid_strategy = BidStrategy::ManualCpc;
        assert!(codes(&input(true), &c).contains(&"E17".to_string()));
        let mut c = campaign(CampaignKind::DemandGen);
        c.negatives = vec![Keyword {
            text: "gratis".into(),
            match_type: MatchType::Phrase,
        }];
        assert!(codes(&input(true), &c).contains(&"E17".to_string()));
        let mut c = campaign(CampaignKind::Search);
        c.ad_groups.clear();
        assert!(codes(&input(true), &c).contains(&"E17".to_string()));
    }

    #[test]
    fn e18_image_campaigns_need_a_logo() {
        let c = campaign(CampaignKind::PerformanceMax);
        assert!(codes(&input(false), &c).contains(&"E18".to_string()));
        assert!(!codes(&input(true), &c).contains(&"E18".to_string()));
    }

    #[test]
    fn e19_a_reference_needs_a_catalog_item_with_a_photo() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].images[0].reference = Some("alamos-malbec".into());
        assert!(!codes(&input(true), &c).contains(&"E19".to_string()));
        for id in ["luigi-bosca", "nope"] {
            c.asset_groups[0].images[0].reference = Some(id.into());
            assert!(codes(&input(true), &c).contains(&"E19".to_string()), "{id}");
        }
    }

    #[test]
    fn e20_every_brief_needs_a_file_when_complete() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].images[1].file = None;
        assert!(codes(&input(true), &c).contains(&"E20".to_string()));
        let partial = Rules::new(&input(true), 50).campaign(&c, None, false, "c");
        assert!(!partial.iter().any(|i| i.code == "E20"));
    }

    #[test]
    fn texts_follow_the_limits_of_each_kind() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].headlines = texts("Titulo", 2);
        assert!(codes(&input(true), &c).contains(&"E13".to_string()));
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].descriptions = vec!["x".repeat(70), "y".repeat(70)];
        assert!(codes(&input(true), &c).contains(&"E13".to_string()));
        let mut c = campaign(CampaignKind::DemandGen);
        c.asset_groups[0].search_themes.clear();
        assert!(
            codes(&input(true), &c).contains(&"E13".to_string()),
            "long headlines"
        );
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].business_name = "x".repeat(26);
        assert!(codes(&input(true), &c).contains(&"E01".to_string()));
    }

    #[test]
    fn e12_asset_groups_follow_the_plan() {
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].name = "outro".into();
        assert!(codes(&input(true), &c).contains(&"E12".to_string()));
    }

    #[test]
    fn e21_every_required_format_needs_a_campaign() {
        let mut inp = input(true);
        inp.business.conversion_tracking = true;
        inp.formats = vec![CampaignKind::Search, CampaignKind::PerformanceMax];
        let account = |kinds: &[CampaignKind]| Account {
            brand_kit: None,
            campaigns: kinds.iter().map(|k| campaign(*k)).collect(),
        };
        let e21 = |a: &Account| {
            Rules::new(&inp, 50)
                .account(a, false)
                .iter()
                .filter(|i| i.code == "E21")
                .count()
        };
        assert_eq!(e21(&account(&[CampaignKind::Search])), 1);
        assert_eq!(e21(&account(&[CampaignKind::DemandGen])), 2);
        assert_eq!(
            e21(&account(&[
                CampaignKind::Search,
                CampaignKind::PerformanceMax
            ])),
            0
        );
    }

    #[test]
    fn e22_landing_pages_stay_inside_the_focus() {
        let c = campaign(CampaignKind::PerformanceMax);
        let mut inp = input(true);
        inp.focus = Some(crate::input::Focus {
            name: "Alamos".into(),
            urls: vec!["https://vinellu.com/w/alamos".into()],
            terms: vec![],
        });
        assert!(!codes(&inp, &c).contains(&"E22".to_string()));
        inp.focus = Some(crate::input::Focus {
            name: "Luigi".into(),
            urls: vec!["https://vinellu.com/w/luigi".into()],
            terms: vec![],
        });
        assert!(codes(&inp, &c).contains(&"E22".to_string()));
        assert!(
            !codes(&input(true), &c).contains(&"E22".to_string()),
            "no focus, no rule"
        );
    }

    #[test]
    fn w06_and_w07_are_warnings() {
        let c = campaign(CampaignKind::PerformanceMax);
        assert!(warnings(&c).contains(&"W06".to_string()));
        let mut c = campaign(CampaignKind::PerformanceMax);
        c.asset_groups[0].images[0].prompt = "A bottle with a label that reads Alamos".into();
        assert!(warnings(&c).contains(&"W07".to_string()));
        assert!(!warnings(&campaign(CampaignKind::PerformanceMax)).contains(&"W07".to_string()));
    }
}
