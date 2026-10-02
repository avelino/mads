//! Google Ads Editor CSV for image campaigns. Image columns hold paths relative to the CSV.
//! The layout follows Google's documented headers and is not verified against a real template yet.

use super::{Account, AspectRatio, AssetGroup, Campaign, CsvFile, ExportError, export::Sheet};
use crate::input::Input;

/// Path of the file under the platform directory.
pub const EDITOR_CSV: &str = "editor/image-campaigns.csv";
/// Assets per asset group, from Google's limits.
const HEADLINES: usize = 15;
const LONG_HEADLINES: usize = 5;
const DESCRIPTIONS: usize = 5;

/// How many numbered columns each repeated field needs in this account.
struct Widths {
    images: Vec<(AspectRatio, usize)>,
    themes: usize,
}

fn widths(campaigns: &[&Campaign]) -> Widths {
    let groups = || campaigns.iter().flat_map(|c| &c.asset_groups);
    let most = |f: &dyn Fn(&AssetGroup) -> usize| groups().map(f).max().unwrap_or(0).max(1);
    Widths {
        images: AspectRatio::ALL
            .iter()
            .map(|r| {
                (
                    *r,
                    most(&|g| g.images.iter().filter(|b| b.ratio == *r).count()),
                )
            })
            .collect(),
        themes: most(&|g| g.search_themes.len()),
    }
}

fn numbered(out: &mut Vec<String>, name: &str, n: usize) {
    out.extend((1..=n).map(|i| format!("{name} {i}")));
}

fn header(w: &Widths) -> Vec<String> {
    let mut h: Vec<String> = [
        "Campaign",
        "Campaign type",
        "Campaign status",
        "Budget",
        "Budget type",
        "Bid strategy type",
        "Languages",
        "Location",
        "Asset group",
        "Asset group status",
        "Final URL",
        "Business name",
    ]
    .map(String::from)
    .into();
    numbered(&mut h, "Headline", HEADLINES);
    numbered(&mut h, "Long headline", LONG_HEADLINES);
    numbered(&mut h, "Description", DESCRIPTIONS);
    h.push("Logo 1".into());
    for (r, n) in &w.images {
        numbered(&mut h, r.label(), *n);
    }
    numbered(&mut h, "Search theme", w.themes);
    h
}

fn padded(out: &mut Vec<String>, items: &[String], n: usize) {
    out.extend((0..n).map(|i| items.get(i).cloned().unwrap_or_default()));
}

fn campaign_row(input: &Input, c: &Campaign, width: usize) -> Vec<String> {
    let mut row = vec![
        c.name.clone(),
        c.kind.label().into(),
        input.export.status.as_str().into(),
        c.daily_budget.format_budget(input.export.decimal_comma),
        "Daily".into(),
        c.bid_strategy.label().into(),
        input.business.language_primary().into(),
        input
            .business
            .locations
            .first()
            .cloned()
            .unwrap_or_default(),
    ];
    row.resize(width, String::new());
    row
}

fn group_row(c: &Campaign, g: &AssetGroup, w: &Widths, logo: &str) -> Vec<String> {
    let mut row = vec![c.name.clone()];
    row.resize(8, String::new());
    row.extend([
        g.name.clone(),
        "Enabled".into(),
        g.final_url.clone(),
        g.business_name.clone(),
    ]);
    padded(&mut row, &g.headlines, HEADLINES);
    padded(&mut row, &g.long_headlines, LONG_HEADLINES);
    padded(&mut row, &g.descriptions, DESCRIPTIONS);
    row.push(logo.into());
    for (r, n) in &w.images {
        let files: Vec<String> = g
            .images
            .iter()
            .filter(|b| b.ratio == *r)
            .map(|b| b.file.clone().unwrap_or_default())
            .collect();
        padded(&mut row, &files, *n);
    }
    padded(&mut row, &g.search_themes, w.themes);
    row
}

/// The Editor CSV of every image campaign, or None when the account has none.
/// Every brief must have its file and the input a logo: validation guarantees both.
pub fn export_editor_csv(input: &Input, account: &Account) -> Result<Option<CsvFile>, ExportError> {
    let campaigns: Vec<&Campaign> = account
        .campaigns
        .iter()
        .filter(|c| c.kind.has_images())
        .collect();
    if campaigns.is_empty() {
        return Ok(None);
    }
    let logo = crate::images::logo_rel_path(input).ok_or(ExportError::MissingLogo)?;
    if let Some(b) = campaigns
        .iter()
        .flat_map(|c| &c.asset_groups)
        .flat_map(|g| &g.images)
        .find(|b| b.file.is_none())
    {
        return Err(ExportError::MissingImage(b.id.clone()));
    }
    let w = widths(&campaigns);
    let head = header(&w);
    let mut sheet = Sheet::new(&head)?;
    for c in campaigns {
        sheet.row(campaign_row(input, c, head.len()))?;
        for g in &c.asset_groups {
            sheet.row(group_row(c, g, &w, &logo))?;
        }
    }
    Ok(Some(sheet.finish(EDITOR_CSV)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::*;

    fn brief(id: &str, ratio: AspectRatio) -> ImageBrief {
        ImageBrief {
            id: id.into(),
            ratio,
            prompt: "p".into(),
            reference: None,
            file: Some(format!("images/c/g-{id}.jpg")),
        }
    }

    fn account() -> Account {
        let group = AssetGroup {
            name: "tintos".into(),
            final_url: "https://vinellu.com/w/a".into(),
            business_name: "Vinellu".into(),
            headlines: vec!["H1".into(), "H2, com virgula".into()],
            long_headlines: vec!["Long".into()],
            descriptions: vec!["D1".into(), "D2".into()],
            search_themes: vec!["vinho".into(), "malbec".into()],
            images: vec![
                brief("a", AspectRatio::Landscape),
                brief("b", AspectRatio::Square),
                brief("c", AspectRatio::Square),
            ],
        };
        Account {
            brand_kit: None,
            campaigns: vec![Campaign {
                name: "Vinellu - PMax".into(),
                slug: "vinellu-pmax".into(),
                kind: CampaignKind::PerformanceMax,
                intent: Intent::Generic,
                daily_budget: Cents(2050),
                bid_strategy: BidStrategy::MaximizeConversions,
                rationale: String::new(),
                planned_ad_groups: vec![],
                ad_groups: vec![],
                asset_groups: vec![group],
                negatives: vec![],
                assets: None,
            }],
        }
    }

    fn input() -> Input {
        let mut i = crate::testutil::input();
        i.logo = Some("/x/brand/logo.png".into());
        i
    }

    fn lines(f: &CsvFile) -> Vec<String> {
        let text = String::from_utf8(f.bytes.clone()).unwrap();
        assert!(text.ends_with("\r\n"));
        text.split("\r\n")
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    }

    #[test]
    fn writes_a_campaign_row_then_one_row_per_asset_group() {
        let f = export_editor_csv(&input(), &account()).unwrap().unwrap();
        assert_eq!(f.name, "editor/image-campaigns.csv");
        let l = lines(&f);
        assert_eq!(l.len(), 3);
        let head: Vec<&str> = l[0].split(',').collect();
        assert_eq!(
            &head[..4],
            ["Campaign", "Campaign type", "Campaign status", "Budget"]
        );
        assert!(head.contains(&"Square image 2") && !head.contains(&"Square image 3"));
        assert!(head.contains(&"Vertical image 1") && head.contains(&"Search theme 2"));
        assert!(l[1].starts_with(
            "Vinellu - PMax,Performance Max,Paused,\"20,50\",Daily,Maximize conversions,pt,Brazil,"
        ));
        assert!(
            l[2].contains(
                ",tintos,Enabled,https://vinellu.com/w/a,Vinellu,H1,\"H2, com virgula\","
            )
        );
        assert!(
            l[2].contains(",images/logo.png,images/c/g-a.jpg,images/c/g-b.jpg,images/c/g-c.jpg,")
        );
        assert!(l[2].ends_with(",vinho,malbec"));
        let cells = |row: &str| {
            csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(row.as_bytes())
                .records()
                .next()
                .unwrap()
                .unwrap()
                .len()
        };
        assert_eq!(cells(&l[1]), head.len());
        assert_eq!(cells(&l[2]), head.len());
    }

    #[test]
    fn nothing_without_image_campaigns() {
        let mut a = account();
        a.campaigns[0].kind = CampaignKind::Search;
        assert_eq!(export_editor_csv(&input(), &a).unwrap(), None);
    }

    #[test]
    fn a_missing_file_or_logo_is_an_error() {
        let mut a = account();
        a.campaigns[0].asset_groups[0].images[1].file = None;
        assert_eq!(
            export_editor_csv(&input(), &a),
            Err(ExportError::MissingImage("b".into()))
        );
        let mut i = input();
        i.logo = None;
        assert_eq!(
            export_editor_csv(&i, &account()),
            Err(ExportError::MissingLogo)
        );
    }
}
