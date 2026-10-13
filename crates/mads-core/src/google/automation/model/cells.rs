//! Cell values the package shares with Google Ads Editor's column vocabulary.

use crate::google::MatchType;
use crate::input::Input;

/// Blank when the id is missing or is the `000-000-0000` placeholder. Never write that placeholder.
pub(crate) fn customer_cell(id: &str) -> &str {
    let id = id.trim();
    let digits: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.chars().all(|c| c == '0') {
        ""
    } else {
        id
    }
}

/// `google_ads.tracking_template` when set. Otherwise `{lpurl}?` plus `export.url_suffix`.
/// `{mads_campaign}` becomes `campaign`.
pub(crate) fn tracking_template(input: &Input, campaign: &str) -> String {
    let explicit = input.google_ads.tracking_template.trim();
    let suffix = input.export.url_suffix.trim();
    let raw = if !explicit.is_empty() {
        explicit.to_string()
    } else if suffix.is_empty() || suffix.starts_with("{lpurl}") {
        suffix.to_string()
    } else {
        format!("{{lpurl}}?{suffix}")
    };
    raw.replace("{mads_campaign}", campaign)
}

pub(crate) fn match_label(m: MatchType) -> &'static str {
    match m {
        MatchType::Phrase => "Phrase",
        MatchType::Exact => "Exact",
    }
}

/// A package name goes to Google Play. Digits only go to the Apple App Store.
pub(crate) fn store_of(app_id: &str) -> &'static str {
    if app_id.chars().all(|c| c.is_ascii_digit()) {
        "Apple App Store"
    } else {
        "Google Play"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_cell_blanks_the_placeholder_and_empty_ids() {
        assert_eq!(customer_cell("000-000-0000"), "");
        assert_eq!(customer_cell("  "), "");
        assert_eq!(customer_cell(" 123-456-7890 "), "123-456-7890");
    }

    #[test]
    fn store_of_tells_apple_from_google_play() {
        assert_eq!(store_of("123456"), "Apple App Store");
        assert_eq!(store_of("com.vinellu.app"), "Google Play");
    }

    #[test]
    fn match_labels() {
        assert_eq!(match_label(MatchType::Phrase), "Phrase");
        assert_eq!(match_label(MatchType::Exact), "Exact");
    }
}
