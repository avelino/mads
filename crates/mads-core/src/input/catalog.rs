use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{business::InputError, normalize_url, slugify};

const MAX_ROWS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub category: String,
    pub aliases: Vec<String>,
    pub third_party: bool,
    pub notes: String,
}

fn invalid(row: usize, column: &str, message: impl Into<String>) -> InputError {
    InputError::Invalid {
        key: format!("catalog.csv row {row} {column}"),
        message: message.into(),
    }
}

pub fn parse_catalog(bytes: &[u8]) -> Result<Vec<CatalogItem>, InputError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| InputError::Parse(format!("catalog.csv is not UTF-8: {e}")))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| InputError::Parse(format!("catalog.csv header: {e}")))?
        .iter()
        .map(|h| h.to_lowercase())
        .collect();
    let col = |name: &str| headers.iter().position(|h| h == name);
    let (Some(name_i), Some(url_i)) = (col("name"), col("url")) else {
        return Err(InputError::Invalid {
            key: "catalog.csv header".into(),
            message: "required columns: name, url".into(),
        });
    };
    let (cat_i, alias_i, third_i, notes_i) = (
        col("category"),
        col("aliases"),
        col("third_party"),
        col("notes"),
    );

    let mut items = Vec::new();
    let mut seen_urls = BTreeSet::new();
    let mut id_counts: BTreeMap<String, usize> = BTreeMap::new();
    for (n, rec) in rdr.records().enumerate() {
        let row = n + 2;
        if n >= MAX_ROWS {
            return Err(invalid(row, "", format!("more than {MAX_ROWS} rows")));
        }
        let rec = rec.map_err(|e| InputError::Parse(format!("catalog.csv row {row}: {e}")))?;
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).unwrap_or("").to_string();

        let name = get(Some(name_i));
        let len = name.chars().count();
        if !(1..=120).contains(&len) {
            return Err(invalid(row, "name", "must be 1 to 120 chars"));
        }
        let url = get(Some(url_i));
        let Some(norm) = normalize_url(&url) else {
            return Err(invalid(
                row,
                "url",
                format!("not an absolute http(s) URL: {url}"),
            ));
        };
        if !seen_urls.insert(norm) {
            return Err(invalid(row, "url", format!("duplicate url: {url}")));
        }
        let third_party = match get(third_i).to_lowercase().as_str() {
            "" | "false" => false,
            "true" => true,
            other => {
                return Err(invalid(
                    row,
                    "third_party",
                    format!("expected true or false, got {other}"),
                ));
            }
        };
        let notes = get(notes_i);
        if notes.chars().count() > 500 {
            return Err(invalid(row, "notes", "at most 500 chars"));
        }
        let aliases = get(alias_i)
            .split('|')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(String::from)
            .collect();

        let base = slugify(&name);
        let count = id_counts.entry(base.clone()).or_insert(0);
        *count += 1;
        let id = if *count == 1 {
            base
        } else {
            format!("{base}-{count}")
        };

        items.push(CatalogItem {
            id,
            name,
            url,
            category: get(cat_i),
            aliases,
            third_party,
            notes,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Vec<CatalogItem>, InputError> {
        parse_catalog(s.as_bytes())
    }

    #[test]
    fn parses_columns_and_aliases() {
        let items = parse(
            "name,url,category,aliases,third_party,notes\n\
             DV Catena,https://x.com/a,blend,d.v. catena|dv catena cabernet,true,nota\n",
        )
        .unwrap();
        assert_eq!(items[0].id, "dv-catena");
        assert_eq!(items[0].aliases, vec!["d.v. catena", "dv catena cabernet"]);
        assert!(items[0].third_party);
        assert_eq!(items[0].notes, "nota");
    }

    #[test]
    fn optional_columns_may_be_absent() {
        let items = parse("name,url\nAlamos,https://x.com/a\n").unwrap();
        assert!(!items[0].third_party);
        assert!(items[0].aliases.is_empty());
        assert_eq!(items[0].category, "");
    }

    #[test]
    fn duplicate_names_get_numbered_ids_in_file_order() {
        let items = parse(
            "name,url\nAlamos,https://x.com/a\nAlamos,https://x.com/b\nAlamos,https://x.com/c\n",
        )
        .unwrap();
        let ids: Vec<_> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["alamos", "alamos-2", "alamos-3"]);
    }

    #[test]
    fn duplicate_url_rejected() {
        assert!(matches!(
            parse("name,url\nA,https://x.com/a\nB,https://x.com/a\n"),
            Err(InputError::Invalid { .. })
        ));
    }

    #[test]
    fn relative_url_rejected_with_row_in_key() {
        match parse("name,url\nA,/rel\n") {
            Err(InputError::Invalid { key, .. }) => assert_eq!(key, "catalog.csv row 2 url"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_required_header_rejected() {
        assert!(matches!(
            parse("name\nA\n"),
            Err(InputError::Invalid { .. })
        ));
    }

    #[test]
    fn bom_and_crlf_are_accepted() {
        let items = parse("\u{feff}name,url\r\nA,https://x.com/a\r\n").unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn empty_name_rejected() {
        assert!(matches!(
            parse("name,url\n ,https://x.com/a\n"),
            Err(InputError::Invalid { .. })
        ));
    }

    #[test]
    fn bad_third_party_value_rejected() {
        assert!(matches!(
            parse("name,url,third_party\nA,https://x.com/a,maybe\n"),
            Err(InputError::Invalid { .. })
        ));
    }

    #[test]
    fn more_than_5000_rows_rejected() {
        let mut s = String::from("name,url\n");
        for i in 0..5001 {
            s.push_str(&format!("n{i},https://x.com/{i}\n"));
        }
        assert!(matches!(parse(&s), Err(InputError::Invalid { .. })));
    }
}
