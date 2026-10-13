//! One `.xlsx` tab in the package layout: title, subtitle and note on rows 1 to 3, header on
//! row 4, data from row 5. Every workbook of the package is written through `workbook`.

use rust_xlsxwriter::{
    ColNum, DocProperties, ExcelDateTime, Format, Formula, RowNum, Workbook, Worksheet, XlsxError,
};
use std::collections::HashSet;

use crate::google::ExportError;

/// Excel row (1-based) of the first data row, below the header on row 4.
pub(crate) const FIRST_ROW: usize = 5;

/// A tab name as a formula reference: quoted when it holds a space.
pub(crate) fn quote_tab(tab: &str) -> String {
    if tab.contains(' ') {
        format!("'{tab}'")
    } else {
        tab.to_string()
    }
}

/// Reference to column `col` of tab `tab` on Excel row `row`, row relative.
pub(crate) fn cell_ref(tab: &str, col: usize, row: usize) -> String {
    format!("{}!${}{row}", quote_tab(tab), col_letter(col))
}

/// Zero-based worksheet row of the header.
const HEADER_ROW: usize = FIRST_ROW - 2;

/// Fill of the cells a person must complete before going live.
const PENDING_FILL: u32 = 0xFFF2CC;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cell {
    Empty,
    Text(String),
    /// Formula text with the leading `=`, English function names and `,` separators.
    Formula(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sheet {
    pub name: String,
    pub title: String,
    pub subtitle: String,
    pub note: Cell,
    pub header: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// `(row index in rows, column index)` of cells that get the pending fill.
    pub pending: Vec<(usize, usize)>,
}

impl Sheet {
    pub fn new(name: &str, title: &str, subtitle: &str, header: &[&str]) -> Sheet {
        Sheet {
            name: name.to_string(),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            note: Cell::Empty,
            header: header.iter().map(|h| h.to_string()).collect(),
            rows: Vec::new(),
            pending: Vec::new(),
        }
    }
}

/// Column letter of a zero-based index: 0 is `A`, 25 is `Z`, 26 is `AA`.
pub(crate) fn col_letter(index: usize) -> String {
    let mut n = index + 1;
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    out.iter().rev().map(|&b| b as char).collect()
}

/// The `.xlsx` bytes of `sheets`, one tab each, in order. The creation date is fixed so the
/// same package always gives the same bytes.
pub(crate) fn workbook(sheets: &[Sheet]) -> Result<Vec<u8>, ExportError> {
    build(sheets).map_err(|e| ExportError::Xlsx(e.to_string()))
}

fn build(sheets: &[Sheet]) -> Result<Vec<u8>, XlsxError> {
    let mut book = Workbook::new();
    let created = ExcelDateTime::from_ymd(2026, 1, 1)?;
    book.set_properties(&DocProperties::new().set_creation_datetime(&created));
    for sheet in sheets {
        write_sheet(book.add_worksheet(), sheet)?;
    }
    book.save_to_buffer()
}

fn write_sheet(ws: &mut Worksheet, sheet: &Sheet) -> Result<(), XlsxError> {
    let bold = Format::new().set_bold();
    let pending_fill = Format::new().set_background_color(PENDING_FILL);
    let pending: HashSet<(usize, usize)> = sheet.pending.iter().copied().collect();
    ws.set_name(&sheet.name)?;
    ws.write_string_with_format(0, 0, &sheet.title, &bold)?;
    ws.write_string(1, 0, &sheet.subtitle)?;
    write_cell(ws, 2, 0, &sheet.note, None)?;
    for (c, h) in sheet.header.iter().enumerate() {
        ws.write_string_with_format(row_num(HEADER_ROW)?, col_num(c)?, h, &bold)?;
    }
    for (r, row) in sheet.rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let format = pending.contains(&(r, c)).then_some(&pending_fill);
            write_cell(ws, row_num(HEADER_ROW + 1 + r)?, col_num(c)?, cell, format)?;
        }
    }
    // A pending cell past the end of its row still gets the fill.
    for &(r, c) in &sheet.pending {
        if sheet.rows.get(r).is_none_or(|row| c >= row.len()) {
            ws.write_blank(row_num(HEADER_ROW + 1 + r)?, col_num(c)?, &pending_fill)?;
        }
    }
    ws.set_freeze_panes(row_num(HEADER_ROW + 1)?, 0)?;
    Ok(())
}

fn write_cell(
    ws: &mut Worksheet,
    row: RowNum,
    col: ColNum,
    cell: &Cell,
    format: Option<&Format>,
) -> Result<(), XlsxError> {
    match (cell, format) {
        (Cell::Empty, None) => {}
        (Cell::Empty, Some(f)) => {
            ws.write_blank(row, col, f)?;
        }
        (Cell::Text(s), None) => {
            ws.write_string(row, col, s)?;
        }
        (Cell::Text(s), Some(f)) => {
            ws.write_string_with_format(row, col, s, f)?;
        }
        (Cell::Formula(s), None) => {
            ws.write_formula(row, col, Formula::new(s))?;
        }
        (Cell::Formula(s), Some(f)) => {
            ws.write_formula_with_format(row, col, Formula::new(s), f)?;
        }
    }
    Ok(())
}

fn row_num(r: usize) -> Result<RowNum, XlsxError> {
    RowNum::try_from(r).map_err(|_| XlsxError::RowColumnLimitError)
}

fn col_num(c: usize) -> Result<ColNum, XlsxError> {
    ColNum::try_from(c).map_err(|_| XlsxError::RowColumnLimitError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
    use std::io::Cursor;

    #[test]
    fn col_letter_counts_like_excel() {
        assert_eq!(col_letter(0), "A");
        assert_eq!(col_letter(25), "Z");
        assert_eq!(col_letter(26), "AA");
        assert_eq!(col_letter(51), "AZ");
        assert_eq!(col_letter(52), "BA");
        assert_eq!(col_letter(701), "ZZ");
        assert_eq!(col_letter(702), "AAA");
    }

    fn sample() -> Sheet {
        let mut s = Sheet::new("Feed", "Feed", "One row per ad group.", &["a", "b", "c"]);
        s.note = Cell::Text("Fill the yellow cells.".into());
        s.rows.push(vec![
            Cell::Text("Don \"Melchor\", 2019".into()),
            Cell::Text("1.50".into()),
            Cell::Formula("=IF(A5<>\"\",\"Add\",\"-\")".into()),
        ]);
        s.pending.push((0, 1));
        s.pending.push((1, 0));
        s
    }

    fn open(bytes: Vec<u8>) -> Xlsx<Cursor<Vec<u8>>> {
        open_workbook_from_rs(Cursor::new(bytes)).expect("valid xlsx")
    }

    #[test]
    fn workbook_reads_back_with_the_layout_rows() {
        let mut book = open(workbook(&[sample()]).expect("written"));
        assert_eq!(book.sheet_names(), ["Feed"]);
        let range = book.worksheet_range("Feed").expect("tab");
        let at = |r: u32, c: u32| range.get_value((r, c)).cloned().unwrap_or(Data::Empty);
        assert_eq!(at(0, 0), Data::String("Feed".into()));
        assert_eq!(at(1, 0), Data::String("One row per ad group.".into()));
        assert_eq!(at(2, 0), Data::String("Fill the yellow cells.".into()));
        assert_eq!(at(3, 0), Data::String("a".into()));
        assert_eq!(at(3, 2), Data::String("c".into()));
        assert_eq!(at(4, 0), Data::String("Don \"Melchor\", 2019".into()));
        assert_eq!(at(4, 1), Data::String("1.50".into()));

        let formulas = book.worksheet_formula("Feed").expect("formulas");
        // calamine returns the formula without the leading `=`.
        assert_eq!(
            formulas.get_value((4, 2)).map(String::as_str),
            Some("IF(A5<>\"\",\"Add\",\"-\")")
        );
    }

    #[test]
    fn workbook_keeps_tab_order_and_is_deterministic() {
        let mut second = sample();
        second.name = "Settings".into();
        let a = workbook(&[sample(), second.clone()]).expect("written");
        let b = workbook(&[sample(), second]).expect("written");
        assert_eq!(a, b);
        assert_eq!(open(a).sheet_names(), ["Feed", "Settings"]);
    }

    #[test]
    fn workbook_reports_an_invalid_tab_name_as_xlsx_error() {
        let mut bad = sample();
        bad.name = "a/b".into();
        assert!(matches!(workbook(&[bad]), Err(ExportError::Xlsx(_))));
    }
}
