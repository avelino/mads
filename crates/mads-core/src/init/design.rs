//! DESIGN.md: the brand identity generated pictures follow. Colors come from code, words from the agent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    google::Issue,
    images::{color_name, parse_hex},
};

pub const DESIGN_FILE: &str = "DESIGN.md";
const TEXT_MAX: usize = 600;
const AVOID_MAX: usize = 10;

/// What the agent learned about how the brand looks and sounds. Only what the site or the web shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignDraft {
    /// The visual feel of the brand in a few words: mood, energy, modern or classic, premium or popular.
    pub style: String,
    /// What the brand's own photos show: people, places, light, framing. Empty when the site has none.
    #[serde(default)]
    pub imagery: String,
    /// How the brand talks to customers.
    #[serde(default)]
    pub voice: String,
    /// What pictures of this brand must never show, up to 10 short items.
    #[serde(default)]
    pub avoid: Vec<String>,
}

impl DesignDraft {
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let fields = [
            ("style", &self.style, 1),
            ("imagery", &self.imagery, 0),
            ("voice", &self.voice, 0),
        ];
        for (name, text, min) in fields {
            let n = text.trim().chars().count();
            if n < min || n > TEXT_MAX {
                let msg = format!("{n} chars, expected {min} to {TEXT_MAX}");
                out.push(Issue::error("E13", name, msg));
            }
        }
        if self.avoid.len() > AVOID_MAX {
            let msg = format!("{} items, at most {AVOID_MAX}", self.avoid.len());
            out.push(Issue::error("E13", "avoid", msg));
        }
        out
    }
}

/// One color and where it was found, such as `logo` or `theme color of https://site`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorNote {
    pub hex: String,
    pub source: String,
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn section(md: &mut String, title: &str, body: &str) {
    if !body.trim().is_empty() {
        md.push_str(&format!("## {title}\n\n{}\n\n", body.trim()));
    }
}

/// The file `init` writes. None when there is neither a color nor a draft.
pub fn render_design(
    business: &str,
    colors: &[ColorNote],
    draft: Option<&DesignDraft>,
) -> Option<String> {
    if colors.is_empty() && draft.is_none() {
        return None;
    }
    let mut md = format!(
        "# Design: {business}\n\nThe brand identity mads gives to image campaigns. Edit it freely: mads reads the hex codes under Colors and sends the whole file to the agents that write image briefs.\n\n"
    );
    if !colors.is_empty() {
        md.push_str("## Colors\n\n");
        for c in colors {
            let name = parse_hex(&c.hex).map(color_name).unwrap_or_default();
            md.push_str(&format!(
                "- {} {}: {}\n",
                capitalized(&name),
                c.hex,
                c.source
            ));
        }
        md.push('\n');
    }
    if let Some(d) = draft {
        section(&mut md, "Style", &d.style);
        section(&mut md, "Imagery", &d.imagery);
        section(&mut md, "Voice", &d.voice);
        let avoid: Vec<String> = d.avoid.iter().map(|a| format!("- {}", a.trim())).collect();
        section(&mut md, "Avoid", &avoid.join("\n"));
    }
    Some(md.trim_end().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::images::design_palette;

    fn draft() -> DesignDraft {
        DesignDraft {
            style: "Young, colorful, close to the customer.".into(),
            imagery: "Real travelers smiling, daylight.".into(),
            voice: String::new(),
            avoid: vec!["luxury cars".into()],
        }
    }

    #[test]
    fn the_file_lists_colors_the_image_step_can_read_back() {
        let colors = vec![
            ColorNote {
                hex: "#F0476A".into(),
                source: "logo".into(),
            },
            ColorNote {
                hex: "#AD1457".into(),
                source: "theme color of https://x.com".into(),
            },
        ];
        let md = render_design("Acme", &colors, Some(&draft())).unwrap();
        assert!(md.starts_with("# Design: Acme\n"));
        assert!(
            md.contains(
                "- Pink #F0476A: logo\n- Dark pink #AD1457: theme color of https://x.com\n"
            )
        );
        assert!(
            md.contains("## Style\n\nYoung, colorful") && md.contains("## Avoid\n\n- luxury cars")
        );
        assert!(!md.contains("## Voice"), "empty sections are left out");
        assert_eq!(design_palette(&md).len(), 2);
    }

    #[test]
    fn nothing_to_write_without_colors_or_draft() {
        assert_eq!(render_design("Acme", &[], None), None);
        assert!(render_design("Acme", &[], Some(&draft())).is_some());
    }

    #[test]
    fn a_draft_needs_a_style_and_short_texts() {
        assert!(draft().validate().is_empty());
        let mut d = draft();
        d.style = String::new();
        d.avoid = vec!["x".into(); 11];
        let paths: Vec<String> = d.validate().into_iter().map(|i| i.path).collect();
        assert_eq!(paths, ["style", "avoid"]);
    }
}
