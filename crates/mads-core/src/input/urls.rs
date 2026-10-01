use std::collections::BTreeSet;

use url::Url;

/// Canonical form used to compare URLs: lowercase scheme and host, default port and fragment removed.
pub fn normalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

/// URLs an ad is allowed to point at. Anything outside the set may be a hallucination.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowedUrls(BTreeSet<String>);

impl AllowedUrls {
    pub fn new<I, S>(urls: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut set = Self::default();
        for u in urls {
            set.insert(u.as_ref());
        }
        set
    }

    pub fn insert(&mut self, raw: &str) {
        if let Some(n) = normalize_url(raw) {
            self.0.insert(n);
        }
    }

    pub fn contains(&self, raw: &str) -> bool {
        normalize_url(raw).is_some_and(|n| self.0.contains(&n))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_scheme_host_port_and_fragment() {
        assert_eq!(
            normalize_url("HTTPS://Vinellu.com:443/w/abc?x=1#frag").unwrap(),
            "https://vinellu.com/w/abc?x=1"
        );
    }

    #[test]
    fn keeps_path_case_and_query() {
        assert_eq!(
            normalize_url("https://v.com/W/AbC?Q=1").unwrap(),
            "https://v.com/W/AbC?Q=1"
        );
    }

    #[test]
    fn rejects_relative_and_non_http() {
        assert!(normalize_url("/rel").is_none());
        assert!(normalize_url("ftp://x.com/a").is_none());
        assert!(normalize_url("javascript:alert(1)").is_none());
    }

    #[test]
    fn allowed_set_matches_after_normalization() {
        let set = AllowedUrls::new(["https://vinellu.com", "https://vinellu.com/w/a"]);
        assert!(set.contains("HTTPS://VINELLU.COM:443/w/a#x"));
        assert!(!set.contains("https://vinellu.com/w/b"));
        assert!(!set.contains("https://evil.com/w/a"));
        assert!(!set.contains("garbage"));
    }

    #[test]
    fn insert_extends_the_set() {
        let mut set = AllowedUrls::default();
        assert!(!set.contains("https://a.com/x"));
        set.insert("https://a.com/x");
        assert!(set.contains("https://a.com/x"));
    }
}
