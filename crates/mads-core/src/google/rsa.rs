use std::collections::BTreeSet;

use super::{BrandKit, Rsa, normalize};

pub const MAX_HEADLINES: usize = 15;
pub const MAX_DESCRIPTIONS: usize = 4;

/// The ad as it is exported.
#[derive(Debug, Clone, PartialEq)]
pub struct MergedRsa {
    pub headlines: Vec<String>,
    pub descriptions: Vec<String>,
    pub path1: Option<String>,
    pub path2: Option<String>,
}

/// Ad group specific texts first, brand kit completing up to 15 headlines and 4 descriptions.
/// A brand kit text equal to a specific one is dropped.
pub fn merge_rsa(rsa: &Rsa, kit: &BrandKit) -> MergedRsa {
    MergedRsa {
        headlines: merge(&rsa.headlines, &kit.headlines, MAX_HEADLINES),
        descriptions: merge(&rsa.descriptions, &kit.descriptions, MAX_DESCRIPTIONS),
        path1: rsa.path1.clone(),
        path2: rsa.path2.clone(),
    }
}

fn merge(specific: &[String], shared: &[String], cap: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    specific
        .iter()
        .chain(shared)
        .filter(|t| seen.insert(normalize(t)))
        .take(cap)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::google::{BrandKit, Rsa};

    fn kit(h: &[&str], d: &[&str]) -> BrandKit {
        BrandKit {
            headlines: h.iter().map(|s| s.to_string()).collect(),
            descriptions: d.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn rsa(h: &[&str], d: &[&str]) -> Rsa {
        Rsa {
            headlines: h.iter().map(|s| s.to_string()).collect(),
            descriptions: d.iter().map(|s| s.to_string()).collect(),
            path1: None,
            path2: None,
        }
    }

    #[test]
    fn specific_texts_come_first_then_brand_kit() {
        let m = merge_rsa(&rsa(&["s1", "s2"], &["sd"]), &kit(&["k1", "k2"], &["kd"]));
        assert_eq!(m.headlines, ["s1", "s2", "k1", "k2"]);
        assert_eq!(m.descriptions, ["sd", "kd"]);
    }

    #[test]
    fn specific_wins_over_equal_brand_kit_text() {
        let m = merge_rsa(&rsa(&["Same"], &[]), &kit(&["same", "k2"], &[]));
        assert_eq!(m.headlines, ["Same", "k2"]);
    }

    #[test]
    fn caps_at_15_headlines_and_4_descriptions() {
        let h: Vec<String> = (0..20).map(|i| format!("h{i}")).collect();
        let d: Vec<String> = (0..9).map(|i| format!("d{i}")).collect();
        let hr: Vec<&str> = h.iter().map(String::as_str).collect();
        let dr: Vec<&str> = d.iter().map(String::as_str).collect();
        let m = merge_rsa(&rsa(&hr[..5], &dr[..1]), &kit(&hr[5..], &dr[1..]));
        assert_eq!(m.headlines.len(), 15);
        assert_eq!(m.descriptions.len(), 4);
        assert_eq!(m.headlines[14], "h14");
    }

    #[test]
    fn reproduces_every_rsa_row_of_the_reference_csv() {
        let csv_text = include_str!("../../../../data/5-responsive-search-ads.csv");
        let mut rdr = csv::Reader::from_reader(csv_text.as_bytes());
        let rows: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 12);
        // columns: 6..=20 headlines, 21..=24 descriptions, 25 path1, 26 path2
        let col = |r: &csv::StringRecord, a: usize, b: usize| -> Vec<String> {
            (a..=b)
                .map(|i| r[i].to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        let first = &rows[0];
        let brand = BrandKit {
            headlines: col(first, 11, 20),
            descriptions: col(first, 22, 24),
        };
        for r in &rows {
            let specific = Rsa {
                headlines: col(r, 6, 10),
                descriptions: col(r, 21, 21),
                path1: Some(r[25].to_string()),
                path2: Some(r[26].to_string()),
            };
            let merged = merge_rsa(&specific, &brand);
            assert_eq!(merged.headlines, col(r, 6, 20), "headlines of {}", &r[4]);
            assert_eq!(
                merged.descriptions,
                col(r, 21, 24),
                "descriptions of {}",
                &r[4]
            );
        }
    }
}
