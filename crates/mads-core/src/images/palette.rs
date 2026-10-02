//! Brand colors: the dominant colors of a logo, color names, and the palette a DESIGN.md declares.

use std::collections::BTreeMap;

use image::GenericImageView;

const MAX_COLORS: usize = 3;
/// Pixels sampled per side: enough for a palette, cheap for any logo size.
const SAMPLE: u32 = 64;
/// A color needs this share of the opaque, colored pixels to count.
const MIN_SHARE: f64 = 0.05;

/// `#RRGGBB` for an RGB triple.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Parses `#RGB` or `#RRGGBB`, case-insensitive.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    let full: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_string(),
        _ => return None,
    };
    let byte = |i: usize| u8::from_str_radix(full.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Hue in degrees, saturation and lightness in 0..=1.
fn hsl([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s, l)
}

/// A plain English name an image model understands, such as `pink` or `dark blue`.
pub fn color_name(rgb: [u8; 3]) -> String {
    let (h, s, l) = hsl(rgb);
    if s < 0.15 {
        return match l {
            l if l < 0.15 => "black",
            l if l > 0.9 => "white",
            _ => "gray",
        }
        .into();
    }
    let base = match h {
        h if (330.0..355.0).contains(&h) => "pink",
        h if (h >= 355.0 || h < 15.0) && l > 0.75 => "pink",
        h if h >= 355.0 || h < 15.0 => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 165.0 => "green",
        h if h < 200.0 => "teal",
        h if h < 255.0 => "blue",
        h if h < 290.0 => "purple",
        _ => "pink",
    };
    match l {
        l if l < 0.4 => format!("dark {base}"),
        l if l > 0.8 => format!("light {base}"),
        _ => base.into(),
    }
}

/// White, black, gray and transparent pixels are background, not brand.
fn is_brand_pixel(rgba: [u8; 4]) -> bool {
    let (_, s, l) = hsl([rgba[0], rgba[1], rgba[2]]);
    rgba[3] >= 128 && s >= 0.15 && (0.1..=0.92).contains(&l)
}

/// Up to 3 dominant brand colors of a PNG or JPEG, most used first. Empty for a gray logo.
pub fn logo_palette(bytes: &[u8]) -> Result<Vec<String>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("cannot decode logo: {e}"))?;
    let (w, h) = img.dimensions();
    let small = img.thumbnail(SAMPLE.min(w), SAMPLE.min(h)).to_rgba8();
    let mut bins: BTreeMap<[u8; 3], (u64, [u64; 3])> = BTreeMap::new();
    let mut total = 0u64;
    for p in small.pixels().filter(|p| is_brand_pixel(p.0)) {
        let key = [p.0[0] >> 5, p.0[1] >> 5, p.0[2] >> 5];
        let bin = bins.entry(key).or_insert((0, [0; 3]));
        bin.0 += 1;
        (0..3).for_each(|i| bin.1[i] += u64::from(p.0[i]));
        total += 1;
    }
    let mut ranked: Vec<(u64, [u8; 3])> = bins
        .values()
        .map(|(n, sum)| (*n, [0, 1, 2].map(|i| (sum[i] / n) as u8)))
        .filter(|(n, _)| *n as f64 >= total as f64 * MIN_SHARE)
        .collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.0));
    let mut kept: Vec<[u8; 3]> = Vec::new();
    for (_, rgb) in ranked {
        if kept.len() < MAX_COLORS && kept.iter().all(|k| distance(*k, rgb) >= SAME_COLOR) {
            kept.push(rgb);
        }
    }
    Ok(kept.into_iter().map(hex).collect())
}

/// Two shades closer than this read as one color.
const SAME_COLOR: f64 = 48.0;

fn distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    (0..3)
        .map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// `(name, hex)` of every color listed under the `## Colors` heading of a DESIGN.md.
pub fn design_palette(design: &str) -> Vec<(String, String)> {
    let mut in_colors = false;
    let mut out: Vec<(String, String)> = Vec::new();
    for line in design.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_colors = heading.trim().eq_ignore_ascii_case("colors");
            continue;
        }
        if !in_colors {
            continue;
        }
        for word in line.split(|c: char| !(c == '#' || c.is_ascii_hexdigit())) {
            if let Some(rgb) = parse_hex(word).filter(|_| word.starts_with('#')) {
                let h = hex(rgb);
                if !out.iter().any(|(_, x)| *x == h) {
                    out.push((color_name(rgb), h));
                }
            }
        }
    }
    out
}

/// The line every image prompt gets, or None when DESIGN.md declares no color.
pub fn palette_line(design: &str) -> Option<String> {
    let colors = design_palette(design);
    if colors.is_empty() {
        return None;
    }
    let list: Vec<String> = colors.iter().map(|(n, h)| format!("{n} ({h})")).collect();
    Some(format!(
        "Brand colors: {}. Use them on the main object, clothing, props or light accents, so the picture looks like this brand.",
        list.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn logo(fg: [u8; 4], bg: [u8; 4]) -> Vec<u8> {
        let img =
            image::RgbaImage::from_fn(200, 200, |x, _| image::Rgba(if x < 120 { fg } else { bg }));
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn hex_round_trips_and_short_form_expands() {
        assert_eq!(parse_hex("#f0476a"), Some([0xF0, 0x47, 0x6A]));
        assert_eq!(parse_hex("#abc"), Some([0xAA, 0xBB, 0xCC]));
        assert_eq!(parse_hex("f0476a"), None);
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(hex([0xAD, 0x14, 0x57]), "#AD1457");
    }

    #[test]
    fn names_cover_the_usual_brand_colors() {
        let name = |h: &str| color_name(parse_hex(h).unwrap());
        assert_eq!(name("#F0476A"), "pink");
        assert_eq!(name("#AD1457"), "dark pink");
        assert_eq!(name("#E53935"), "red");
        assert_eq!(name("#1E88E5"), "blue");
        assert_eq!(name("#43A047"), "green");
        assert_eq!(name("#FB8C00"), "orange");
        assert_eq!(name("#FFFFFF"), "white");
        assert_eq!(name("#777777"), "gray");
    }

    #[test]
    fn the_logo_palette_skips_white_and_transparent() {
        let pink = [0xF0, 0x47, 0x6A, 255];
        let on_white = logo_palette(&logo(pink, [255, 255, 255, 255])).unwrap();
        assert_eq!(on_white.len(), 1);
        assert_eq!(color_name(parse_hex(&on_white[0]).unwrap()), "pink");
        let on_clear = logo_palette(&logo(pink, [0, 0, 0, 0])).unwrap();
        assert_eq!(on_clear, on_white);
        assert!(
            logo_palette(&logo([20, 20, 20, 255], [255; 4]))
                .unwrap()
                .is_empty()
        );
        let two_pinks = logo([0xEE, 0x39, 0x5D, 255], [0xEF, 0x4D, 0x6D, 255]);
        assert_eq!(
            logo_palette(&two_pinks).unwrap().len(),
            1,
            "near shades merge"
        );
    }

    #[test]
    fn the_design_palette_reads_only_the_colors_section() {
        let md = "# Design\n\n#FFFFFF in the intro\n\n## Colors\n\n- Pink #F0476A: logo\n- Dark pink #AD1457: site theme color\n- again #f0476a\n\n## Style\n\nUses #000000 text.\n";
        assert_eq!(
            design_palette(md),
            [
                ("pink".to_string(), "#F0476A".to_string()),
                ("dark pink".to_string(), "#AD1457".to_string())
            ]
        );
        let line = palette_line(md).unwrap();
        assert!(line.starts_with("Brand colors: pink (#F0476A), dark pink (#AD1457)."));
        assert_eq!(palette_line("## Style\nclean"), None);
    }
}
