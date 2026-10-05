//! One report exported from the Google Ads web interface: a title line, a date range line, a
//! header and rows, with `Total:` rows at the end. Numbers follow the interface language.

use serde::{Deserialize, Serialize};
use time::{Date, Month};

use crate::input::fold;

/// The reports `mads optimize` understands, told apart by their columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    SearchTerms,
    Keywords,
    Campaigns,
    CampaignsByDay,
    Assets,
}

/// The period a report covers, both days included.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Window {
    pub start: String,
    pub end: String,
    pub days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locale {
    /// `1.020,50`: Portuguese, Spanish and most of Europe.
    Comma,
    /// `1,020.50`: English.
    Dot,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub kind: ReportKind,
    pub window: Option<Window>,
    /// Folded header names: `Grupo de anúncios` is `grupo de anuncios`.
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    locale: Locale,
}

/// A row with typed access by column.
pub struct Row<'a> {
    table: &'a Table,
    cells: &'a [String],
}

pub(super) const SEARCH_TERM: &[&str] = &["termo de pesquisa", "search term"];
pub(super) const KEYWORD: &[&str] = &["palavra chave", "keyword", "search keyword"];
pub(super) const MATCH: &[&str] = &["tipo de corresp", "match type"];
pub(super) const CAMPAIGN: &[&str] = &["campanha", "campaign"];
pub(super) const AD_GROUP: &[&str] = &["grupo de anuncios", "ad group"];
pub(super) const ASSET: &[&str] = &["recurso", "asset"];
pub(super) const DAY: &[&str] = &["dia", "day"];

/// Reads one exported report. Err says why the file is not a report mads understands.
pub fn read_table(bytes: &[u8]) -> Result<Table, String> {
    let text = decode(bytes)?;
    let (records, at, kind) = b",\t"
        .iter()
        .find_map(|&d| {
            let r = records(&text, d)?;
            let (at, kind) = header_at(&r)?;
            Some((r, at, kind))
        })
        .ok_or("no known report header (search terms, keywords, campaigns or assets)")?;
    let header: Vec<String> = records[at].iter().map(|h| fold(h)).collect();
    let window = records[..at]
        .iter()
        // `Oct 3, 2026 - Oct 5, 2026` is split by the comma: read the line whole.
        .find_map(|r| window(&r.join(",")));
    let rows = records[at + 1..]
        .iter()
        .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
        .filter(|r| !is_total(r))
        .cloned()
        .collect();
    // Every report has the campaign column, named in the interface language.
    let locale = if header.iter().any(|h| h == "campanha") {
        Locale::Comma
    } else {
        Locale::Dot
    };
    Ok(Table {
        kind,
        window,
        header,
        rows,
        locale,
    })
}

/// Google closes a report with `Total: Account`, `Total: Campanhas` and the like in the first
/// cell. A search term or campaign that merely starts with "total" is a real row.
fn is_total(row: &[String]) -> bool {
    row.first()
        .is_some_and(|c| c.trim_start().to_lowercase().starts_with("total:"))
}

/// UTF-16 LE with a BOM (the "Excel" download) or UTF-8 with or without a BOM.
fn decode(bytes: &[u8]) -> Result<String, String> {
    if let Some(body) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        return String::from_utf16(&units).map_err(|e| e.to_string());
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8(body.to_vec()).map_err(|_| "not UTF-8 or UTF-16 text".to_string())
}

fn records(text: &str, delimiter: u8) -> Option<Vec<Vec<String>>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    reader
        .records()
        .map(|r| r.map(|r| r.iter().map(str::to_string).collect()))
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

/// The first record that is a known report header, and the report kind it names.
fn header_at(records: &[Vec<String>]) -> Option<(usize, ReportKind)> {
    records.iter().enumerate().find_map(|(i, r)| {
        let folded: Vec<String> = r.iter().map(|h| fold(h)).collect();
        kind_of(&folded).map(|k| (i, k))
    })
}

fn kind_of(header: &[String]) -> Option<ReportKind> {
    let any = |names: &[&str]| header.iter().any(|h| names.contains(&h.as_str()));
    if any(SEARCH_TERM) {
        Some(ReportKind::SearchTerms)
    } else if any(KEYWORD) && any(MATCH) {
        Some(ReportKind::Keywords)
    } else if any(ASSET) {
        Some(ReportKind::Assets)
    } else if any(CAMPAIGN) && !any(AD_GROUP) {
        Some(if any(DAY) {
            ReportKind::CampaignsByDay
        } else {
            ReportKind::Campaigns
        })
    } else {
        None
    }
}

/// `3 de outubro de 2026 - 5 de outubro de 2026` or `Oct 3, 2026 - Oct 5, 2026`.
fn window(cell: &str) -> Option<Window> {
    let (a, b) = cell.split_once(" - ")?;
    let (start, end) = (date(a)?, date(b)?);
    let days = u32::try_from((end - start).whole_days() + 1).ok()?;
    Some(Window {
        start: start.to_string(),
        end: end.to_string(),
        days,
    })
}

fn date(text: &str) -> Option<Date> {
    let words = fold(text);
    let mut day = None;
    let mut year = None;
    let mut month = None;
    for w in words.split_whitespace() {
        match w.parse::<u32>() {
            Ok(n) if n >= 1000 => year = i32::try_from(n).ok(),
            Ok(n) if (1..=31).contains(&n) => day = u8::try_from(n).ok(),
            _ => month = month.or_else(|| month_of(w)),
        }
    }
    Date::from_calendar_date(year?, month?, day?).ok()
}

fn month_of(word: &str) -> Option<Month> {
    const NAMES: [&[&str]; 12] = [
        &["jan"],
        &["fev", "feb"],
        &["mar"],
        &["abr", "apr"],
        &["mai", "may"],
        &["jun"],
        &["jul"],
        &["ago", "aug"],
        &["set", "sep"],
        &["out", "oct"],
        &["nov"],
        &["dez", "dec"],
    ];
    let prefix: String = word.chars().take(3).collect();
    let i = NAMES.iter().position(|n| n.contains(&prefix.as_str()))?;
    Month::try_from(u8::try_from(i + 1).ok()?).ok()
}

impl Table {
    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(|cells| Row { table: self, cells })
    }

    /// Index of the first column whose folded name is one of `names`.
    fn col(&self, names: &[&str]) -> Option<usize> {
        self.header.iter().position(|h| names.contains(&h.as_str()))
    }

    /// Index of the first column whose name has every word of one of the groups.
    fn col_words(&self, groups: &[&[&str]]) -> Option<usize> {
        self.header.iter().position(|h| {
            let words: Vec<&str> = h.split_whitespace().collect();
            groups.iter().any(|g| g.iter().all(|w| words.contains(w)))
        })
    }

    fn number(&self, cell: &str) -> Option<f64> {
        let s = cell.trim().trim_end_matches('%').trim();
        if s.is_empty() || s == "--" {
            return None;
        }
        let plain = match self.locale {
            Locale::Comma => s.replace('.', "").replace(',', "."),
            Locale::Dot => s.replace(',', ""),
        };
        plain.parse().ok()
    }
}

impl Row<'_> {
    fn at(&self, i: Option<usize>) -> Option<&str> {
        i.and_then(|i| self.cells.get(i)).map(String::as_str)
    }

    /// The text of the first column named one of `names`, trimmed. Empty when absent.
    pub fn text(&self, names: &[&str]) -> String {
        cell_text(self.at(self.table.col(names)))
    }

    pub fn number(&self, names: &[&str]) -> Option<f64> {
        self.at(self.table.col(names))
            .and_then(|c| self.table.number(c))
    }

    pub fn text_words(&self, groups: &[&[&str]]) -> String {
        cell_text(self.at(self.table.col_words(groups)))
    }

    pub fn number_words(&self, groups: &[&[&str]]) -> Option<f64> {
        self.at(self.table.col_words(groups))
            .and_then(|c| self.table.number(c))
    }
}

/// Trimmed text, with Google's `--` for "no value" read as empty.
fn cell_text(cell: Option<&str>) -> String {
    cell.map(str::trim)
        .filter(|s| *s != "--")
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMS_PT: &str = "Relatório de termos de pesquisa\n3 de outubro de 2026 - 5 de outubro de 2026\nTermo de pesquisa,Tipo de corresp.,Adicionada/excluída,Campanha,Grupo de anúncios,Código da moeda,Cliques,Impr.,Custo,Conversões\nvinho malbec,Correspondência de frase,Nenhum,Catálogo,Uvas tintas,BRL,5,106,\"5,85\",\"0,00\"\nTotal: Conta,,,,,BRL,5,\"1.106\",\"5,85\",\"0,00\"\n";

    #[test]
    fn reads_a_portuguese_search_terms_report() {
        let t = read_table(TERMS_PT.as_bytes()).unwrap();
        assert_eq!(t.kind, ReportKind::SearchTerms);
        assert_eq!(
            t.window,
            Some(Window {
                start: "2026-10-03".into(),
                end: "2026-10-05".into(),
                days: 3
            })
        );
        let rows: Vec<Row> = t.rows().collect();
        assert_eq!(rows.len(), 1, "the Total row is dropped");
        assert_eq!(rows[0].text(SEARCH_TERM), "vinho malbec");
        assert_eq!(rows[0].text(AD_GROUP), "Uvas tintas");
        assert_eq!(rows[0].number(&["custo", "cost"]), Some(5.85));
        assert_eq!(rows[0].number(&["impr"]), Some(106.0));
    }

    #[test]
    fn only_total_rows_are_dropped_not_names_that_start_with_total() {
        let csv = "Campanha,Termo de pesquisa,Custo\nTotal Wine,totalpass academia,\"1,00\"\nTotal: Conta,,\"1,00\"\nTotal: Account,,\"1,00\"\n";
        let t = read_table(csv.as_bytes()).unwrap();
        let rows: Vec<Row> = t.rows().collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text(CAMPAIGN), "Total Wine");
        assert_eq!(rows[0].text(SEARCH_TERM), "totalpass academia");
    }

    #[test]
    fn reads_english_numbers_and_dates() {
        let csv = "Campaign report\nOct 3, 2026 - Oct 16, 2026\nCampaign status,Campaign,Budget,Cost,Clicks,Impr.\nEnabled,Brand,25.00,\"1,316.14\",864,\"5,354\"\n";
        let t = read_table(csv.as_bytes()).unwrap();
        assert_eq!(t.kind, ReportKind::Campaigns);
        assert_eq!(t.window.as_ref().map(|w| w.days), Some(14));
        let row = t.rows().next().unwrap();
        assert_eq!(row.number(&["cost"]), Some(1316.14));
        assert_eq!(row.number(&["impr"]), Some(5354.0));
    }

    #[test]
    fn reads_utf16_tab_separated_files() {
        let text = "Keyword report\nKeyword\tMatch type\tCampaign\tAd group\tClicks\n\"malbec\"\tPhrase match\tC\tG\t65\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let t = read_table(&bytes).unwrap();
        assert_eq!(t.kind, ReportKind::Keywords);
        assert_eq!(t.window, None);
        assert_eq!(t.rows().next().unwrap().number(&["clicks"]), Some(65.0));
    }

    #[test]
    fn tells_report_kinds_apart() {
        let kind = |header: &str| read_table(format!("{header}\nx\n").as_bytes()).map(|t| t.kind);
        assert_eq!(kind("Campanha,Dia,Custo"), Ok(ReportKind::CampaignsByDay));
        assert_eq!(
            kind("Recurso,Campanha,Grupo de anúncios"),
            Ok(ReportKind::Assets)
        );
        assert!(kind("Campanha,Grupo de anúncios,Custo").is_err());
        assert!(kind("foo,bar").is_err());
    }

    #[test]
    fn missing_values_are_none() {
        let t = read_table(TERMS_PT.as_bytes()).unwrap();
        assert_eq!(t.number(" -- "), None);
        assert_eq!(t.number(""), None);
        assert_eq!(t.number("19,62%"), Some(19.62));
    }

    #[test]
    fn column_by_words_finds_long_names() {
        let csv = "Campanha,Parc. impr. perdidas na rede de pesquisa (orçamento)\nC,\"12,5%\"\n";
        let t = read_table(csv.as_bytes()).unwrap();
        let row = t.rows().next().unwrap();
        assert_eq!(
            row.number_words(&[&["perdidas", "orcamento"], &["lost", "budget"]]),
            Some(12.5)
        );
    }
}
