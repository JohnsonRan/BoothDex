//! Decoding the upstream image and encoding the response: AVIF from libaom or rav1e, PNG and JPEG.

use std::io::Cursor;

use anyhow::Result;
#[cfg(feature = "libavif")]
use anyhow::bail;
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

/// AVIF quality by size, on libavif's 0..=100 scale. Both values are smaller and closer to the source than the
/// WebP served before (80 for the sized variants, 88 for the original), measured through this service on 24 Booth
/// images with SSIMULACRA2, medians:
/// - 60 for the sized variants: 10% fewer bytes and 2.8 points higher at 300 px (21 of 24 images better), 11%
///   and 3.1 at 620 px. 55 would be 23% smaller but half the thumbnails would score below their WebP.
/// - 75 for the original, the image the item dialog shows: 14% fewer bytes and 2.5 points higher (20 of 24).
///   70 would be 23% smaller for a level score.
#[cfg(feature = "libavif")]
pub(crate) fn avif_quality(size: &str) -> u8 {
    match size {
        "orig" => 75,
        _ => 60,
    }
}

/// AVIF quality by size for a build without libavif, on ravif's own 1..=100 scale, which is neither libaom's nor
/// the one libavif-style tools use: its 75 measured softer than the WebP this replaced. On 12 Booth images,
/// scored with SSIMULACRA2 against the WebP 80 (sized) and 88 (original) served before, rav1e's 83 is 2% smaller
/// at 300 px for a level score, and its 85 scores 3.5 points higher at 620 px for 16% more bytes and is 22%
/// smaller at the original size for about the same score.
#[cfg(not(feature = "libavif"))]
pub(crate) fn avif_quality(size: &str) -> u8 {
    match size {
        "620x620" | "orig" => 85,
        _ => 83,
    }
}

/// Both encoders: 1 (slowest) ..= 10. On 2 CPUs, libaom at 6 takes about 40 ms for a 300 px thumbnail and 180 ms
/// for an original; rav1e at 6 about 50 ms and 420 ms (asm, 2 threads), its 8 a quarter less for 3% larger files,
/// its 10 over a third larger.
const AVIF_SPEED: u8 = 6;
/// libaom's threads for one image; several images are encoded at once (MAX_TRANSCODES below)
#[cfg(feature = "libavif")]
const AOM_THREADS: std::ffi::c_int = 2;
/// Images libaom encodes at once, at most one per core. It holds about 60 MB for an original, and the container
/// is limited to 1 GB with a 256 MB memory cache in it: a burst of 24 originals peaked at 260 MB with 2 at once,
/// about 400 MB with 4 and 630 MB with 8 (the test's memory cache was 64 MB).
#[cfg(feature = "libavif")]
pub(crate) const MAX_TRANSCODES: usize = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Format {
    Avif,
    Png,
    Jpeg,
}

impl Format {
    pub(crate) fn parse(s: Option<&str>) -> Option<Self> {
        match s {
            None | Some("avif") => Some(Self::Avif),
            Some("png") => Some(Self::Png),
            Some("jpeg" | "jpg") => Some(Self::Jpeg),
            _ => None,
        }
    }
    pub(crate) fn mime(self) -> &'static str {
        match self {
            Self::Avif => "image/avif",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}

/// 8-bit AVIF with full-resolution chroma (4:4:4), from libaom through the system's libavif (src/aom.c).
#[cfg(feature = "libavif")]
fn encode_avif(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    use std::ffi::{CStr, c_char, c_int};
    unsafe extern "C" {
        fn boothdex_avif_encode(pixels: *const u8, width: u32, height: u32, has_alpha: c_int, quality: c_int, speed: c_int, threads: c_int, out: *mut *mut u8, out_len: *mut usize) -> c_int;
        fn boothdex_avif_free(data: *mut u8);
        fn boothdex_avif_error(code: c_int) -> *const c_char;
    }
    let has_alpha = img.color().has_alpha();
    let pixels = if has_alpha { img.to_rgba8().into_raw() } else { img.to_rgb8().into_raw() };
    let (mut data, mut len) = (std::ptr::null_mut(), 0usize);
    // SAFETY: `pixels` holds width * height packed RGB or RGBA pixels and is only read; on 0 the callee has set
    // `data` and `len` to a buffer it allocated, copied here and then released with its own allocator.
    unsafe {
        let code = boothdex_avif_encode(pixels.as_ptr(), img.width(), img.height(), has_alpha.into(), quality.into(), AVIF_SPEED.into(), AOM_THREADS, &mut data, &mut len);
        if code != 0 {
            bail!("libavif: {}", CStr::from_ptr(boothdex_avif_error(code)).to_string_lossy());
        }
        let out = std::slice::from_raw_parts(data, len).to_vec();
        boothdex_avif_free(data);
        Ok(out)
    }
}

/// 8-bit AVIF from rav1e; ravif never subsamples chroma, so the output is always 4:4:4.
#[cfg(not(feature = "libavif"))]
fn encode_avif(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let img = if img.color().has_alpha() { DynamicImage::ImageRgba8(img.to_rgba8()) } else { DynamicImage::ImageRgb8(img.to_rgb8()) };
    let mut out = Vec::new();
    img.write_with_encoder(image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut out, AVIF_SPEED, quality))?;
    Ok(out)
}

pub(crate) fn encode(img: &DynamicImage, format: Format, quality: u8) -> Result<Vec<u8>> {
    Ok(match format {
        Format::Avif => encode_avif(img, quality)?,
        Format::Png => {
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, ImageFormat::Png)?;
            out.into_inner()
        }
        Format::Jpeg => {
            let mut out = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&DynamicImage::ImageRgb8(img.to_rgb8()))?;
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcodes_to_each_format() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(32, 16, |x, y| image::Rgb([x as u8 * 8, y as u8 * 16, 128])));
        let mut png = Cursor::new(Vec::new());
        img.write_to(&mut png, ImageFormat::Png).unwrap();
        let decoded = decode(png.get_ref()).unwrap();
        let avif = encode(&decoded, Format::Avif, avif_quality("orig")).unwrap();
        assert_eq!(&avif[4..12], b"ftypavif");
        // av1C, third byte: monochrome, chroma_subsampling_x, chroma_subsampling_y, then the sample position.
        // All clear is colour at full chroma resolution (4:4:4).
        let av1c = avif.windows(4).position(|w| w == b"av1C").unwrap();
        assert_eq!(avif[av1c + 6] & 0b0001_1100, 0);
        assert_eq!(&encode(&decoded, Format::Jpeg, 90).unwrap()[0..2], &[0xFF, 0xD8]);
        assert_eq!(&encode(&decoded, Format::Png, 0).unwrap()[1..4], b"PNG");
        assert!(decode(b"not an image").is_err());
        assert_eq!(Format::parse(Some("gif")), None);
    }

    #[test]
    fn avif_is_the_default_and_webp_is_gone() {
        assert_eq!(Format::parse(None), Some(Format::Avif));
        assert_eq!(Format::parse(Some("avif")), Some(Format::Avif));
        assert_eq!(Format::parse(Some("webp")), None);
        assert_eq!(Format::Avif.mime(), "image/avif");
    }

    #[test]
    fn list_sizes_get_the_lighter_quality() {
        assert!(avif_quality("300x300_a2_g5") < avif_quality("orig"));
        assert_eq!(avif_quality("72x72_a2_g5"), avif_quality("300x300_a2_g5"));
        assert!(avif_quality("300x300_a2_g5") <= avif_quality("620x620") && avif_quality("620x620") <= avif_quality("orig"));
    }

    #[test]
    fn transparency_survives_avif() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(16, 16, |x, _| image::Rgba([200, 40, 40, if x < 8 { 0 } else { 255 }])));
        let avif = encode(&img, Format::Avif, avif_quality("300x300_a2_g5")).unwrap();
        // the colour item and its alpha auxiliary item: two AV1 configurations
        assert_eq!(avif.windows(4).filter(|w| *w == b"av1C").count(), 2);
    }
}
