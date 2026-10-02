//! Pure image processing: no IO. Decodes PNG and JPEG, writes JPEG pictures and PNG logos.

use std::io::Cursor;

use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage, imageops::FilterType};

use crate::google::AspectRatio;

/// Google's limit for marketing images.
pub const IMAGE_MAX_BYTES: usize = 5 * 1024 * 1024;
/// Google's limit for logos.
pub const LOGO_MAX_BYTES: usize = 150 * 1024;
/// Demand Gen's minimum logo side (PMax accepts 128).
pub const LOGO_MIN_SIDE: u32 = 144;
/// How far a file may be from its ratio and still count as that ratio.
const RATIO_TOLERANCE: f64 = 0.01;
const JPEG_QUALITY: u8 = 88;
const LOGO_SIDES: [u32; 7] = [1200, 800, 600, 400, 256, 200, 144];

fn decode(bytes: &[u8]) -> Result<(DynamicImage, ImageFormat), String> {
    let format = image::guess_format(bytes).map_err(|e| format!("unknown image format: {e}"))?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(format!("{format:?} is not PNG or JPEG"));
    }
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| format!("cannot decode image: {e}"))?;
    Ok((img, format))
}

/// File extension for PNG or JPEG bytes.
pub fn extension(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        ImageFormat::Png => Some("png"),
        ImageFormat::Jpeg => Some("jpg"),
        _ => None,
    }
}

fn ratio_close(w: u32, h: u32, ratio: f64) -> bool {
    let actual = f64::from(w) / f64::from(h.max(1));
    ((actual - ratio) / ratio).abs() <= RATIO_TOLERANCE
}

/// Crops around the center to the ratio, resizes to Google's recommended size and encodes JPEG.
pub fn fit_to_ratio(bytes: &[u8], ratio: AspectRatio) -> Result<Vec<u8>, String> {
    let (img, _) = decode(bytes)?;
    let (w, h) = img.dimensions();
    let target = ratio.value();
    let (cw, ch) = if f64::from(w) / f64::from(h) > target {
        ((f64::from(h) * target).round() as u32, h)
    } else {
        (w, (f64::from(w) / target).round() as u32)
    };
    let (cw, ch) = (cw.clamp(1, w), ch.clamp(1, h));
    let cropped = img.crop_imm((w - cw) / 2, (h - ch) / 2, cw, ch);
    let (tw, th) = ratio.size();
    let resized = cropped.resize_exact(tw, th, FilterType::Lanczos3).to_rgb8();
    let mut out = Vec::new();
    let encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(Cursor::new(&mut out), JPEG_QUALITY);
    resized
        .write_with_encoder(encoder)
        .map_err(|e| format!("cannot encode JPEG: {e}"))?;
    Ok(out)
}

/// Checks a marketing image against Google's limits for its ratio.
pub fn check_image(bytes: &[u8], ratio: AspectRatio) -> Result<(), String> {
    if bytes.len() > IMAGE_MAX_BYTES {
        return Err(format!("{} bytes, limit is {IMAGE_MAX_BYTES}", bytes.len()));
    }
    let (img, _) = decode(bytes)?;
    let (w, h) = img.dimensions();
    let (mw, mh) = ratio.min_size();
    if w < mw || h < mh {
        return Err(format!("{w}x{h} is below the minimum {mw}x{mh}"));
    }
    if !ratio_close(w, h, ratio.value()) {
        return Err(format!("{w}x{h} is not {ratio:?}"));
    }
    Ok(())
}

/// Checks a logo: square, at least 144 px, at most 150 KB, PNG or JPEG.
pub fn check_logo(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > LOGO_MAX_BYTES {
        return Err(format!("{} bytes, limit is {LOGO_MAX_BYTES}", bytes.len()));
    }
    let (img, _) = decode(bytes)?;
    let (w, h) = img.dimensions();
    if w < LOGO_MIN_SIDE || h < LOGO_MIN_SIDE {
        return Err(format!(
            "{w}x{h} is below the minimum {LOGO_MIN_SIDE}x{LOGO_MIN_SIDE}"
        ));
    }
    if !ratio_close(w, h, 1.0) {
        return Err(format!("{w}x{h} is not square"));
    }
    Ok(())
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| format!("cannot encode PNG: {e}"))?;
    Ok(out)
}

/// Turns a downloaded logo into one Google accepts: padded to a square with transparency,
/// then the largest side from 1200 down whose PNG fits in 150 KB.
pub fn prepare_logo(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let (img, _) = decode(bytes)?;
    let (w, h) = img.dimensions();
    if w.max(h) < LOGO_MIN_SIDE {
        return Err(format!(
            "{w}x{h} is below the minimum {LOGO_MIN_SIDE}x{LOGO_MIN_SIDE}"
        ));
    }
    let side = w.max(h);
    let mut square = RgbaImage::new(side, side);
    image::imageops::overlay(
        &mut square,
        &img.to_rgba8(),
        i64::from((side - w) / 2),
        i64::from((side - h) / 2),
    );
    for target in LOGO_SIDES.iter().copied().filter(|s| *s <= side) {
        let resized = image::imageops::resize(&square, target, target, FilterType::Lanczos3);
        let png = encode_png(&resized)?;
        if png.len() <= LOGO_MAX_BYTES {
            return Ok(png);
        }
    }
    Err("the logo does not fit in 150 KB at 144 px".into())
}

/// A flat picture of the ratio's size. For tests and the offline `solid` provider.
pub fn solid_png(ratio: AspectRatio, rgb: [u8; 3]) -> Vec<u8> {
    let (w, h) = ratio.size();
    let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb));
    let mut out = Vec::new();
    // Encoding an in-memory RGB buffer to PNG cannot fail; an empty result fails the image check later.
    let _ = img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([10, 120, 200, 255]));
        encode_png(&img).unwrap()
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        decode(bytes).unwrap().0.dimensions()
    }

    #[test]
    fn fit_crops_and_resizes_to_every_ratio() {
        let src = png(1536, 1024);
        for ratio in AspectRatio::ALL {
            let out = fit_to_ratio(&src, ratio).unwrap();
            assert_eq!(dims(&out), ratio.size(), "{ratio:?}");
            assert_eq!(extension(&out), Some("jpg"));
            check_image(&out, ratio).unwrap();
        }
    }

    #[test]
    fn check_rejects_wrong_ratio_small_size_and_unknown_bytes() {
        let square = fit_to_ratio(&png(800, 800), AspectRatio::Square).unwrap();
        assert!(check_image(&square, AspectRatio::Landscape).is_err());
        assert!(check_image(&png(200, 200), AspectRatio::Square).is_err());
        assert!(check_image(b"not an image", AspectRatio::Square).is_err());
    }

    #[test]
    fn logo_check_needs_square_size_and_weight() {
        check_logo(&png(200, 200)).unwrap();
        assert!(check_logo(&png(100, 100)).unwrap_err().contains("minimum"));
        assert!(check_logo(&png(400, 200)).unwrap_err().contains("square"));
    }

    #[test]
    fn prepare_logo_pads_to_a_square_that_passes_the_check() {
        let out = prepare_logo(&png(600, 200)).unwrap();
        let (w, h) = dims(&out);
        assert_eq!(w, h);
        assert_eq!(w, 600);
        check_logo(&out).unwrap();
    }

    #[test]
    fn prepare_logo_refuses_a_tiny_icon() {
        assert!(prepare_logo(&png(64, 64)).is_err());
    }

    #[test]
    fn solid_png_has_the_ratio_size() {
        let p = solid_png(AspectRatio::Portrait, [1, 2, 3]);
        assert_eq!(dims(&p), (960, 1200));
    }
}
