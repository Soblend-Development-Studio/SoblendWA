use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

use crate::error::{Error, Result};

pub(crate) const THUMBNAIL_WIDTH: u32 = 32;
const PROFILE_PICTURE_SIZE: u32 = 640;

pub(super) fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&image.to_rgb8())
        .map_err(|err| Error::Media(err.to_string()))?;
    Ok(out.into_inner())
}

fn decode(data: &[u8]) -> Result<DynamicImage> {
    image::load_from_memory(data).map_err(|err| Error::Media(format!("unsupported image: {err}")))
}

pub fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(data)).with_guessed_format().ok()?.into_dimensions().ok()
}

pub fn thumbnail(data: &[u8], width: u32) -> Result<Vec<u8>> {
    let image = decode(data)?;
    let (w, h) = image.dimensions();
    let target_w = width.min(w).max(1);
    let target_h = ((u64::from(h) * u64::from(target_w)) / u64::from(w.max(1))).max(1) as u32;
    // thumbnail_exact samples instead of filtering every source pixel; for a phone photo going down
    // to 32 px wide it's about twice as fast as resize_exact and looks the same at that size
    encode_jpeg(&image.thumbnail_exact(target_w, target_h), 50)
}

pub fn profile_picture(data: &[u8]) -> Result<Vec<u8>> {
    let image = decode(data)?;
    let (w, h) = image.dimensions();
    let side = w.min(h);
    let square = image.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let size = side.min(PROFILE_PICTURE_SIZE);
    encode_jpeg(&square.resize_exact(size, size, FilterType::Lanczos3), 80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn photo_cost() {
        let photo = DynamicImage::ImageRgb8(image::RgbImage::from_fn(1600, 1200, |x, y| {
            image::Rgb([(x * 7 % 251) as u8, (y * 13 % 241) as u8, ((x ^ y) % 239) as u8])
        }));
        let mut jpeg = std::io::Cursor::new(Vec::new());
        photo.write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
        let jpeg = jpeg.into_inner();
        let time = |label: &str, work: &dyn Fn()| {
            let started = std::time::Instant::now();
            for _ in 0..5 {
                work();
            }
            println!("BENCH {label}: {:?}", started.elapsed() / 5);
        };
        println!("BENCH foto de 1600x1200, {} KB", jpeg.len() / 1024);
        time("dimensiones", &|| {
            std::hint::black_box(image_dimensions(&jpeg));
        });
        time("miniatura", &|| {
            let _ = std::hint::black_box(thumbnail(&jpeg, THUMBNAIL_WIDTH));
        });
        let key = [7u8; 32];
        let iv = [9u8; 16];
        time("cifrar + mac + hash", &|| {
            let body = crate::crypto::aes_cbc_encrypt(&key, &iv, &jpeg).unwrap();
            std::hint::black_box(crate::crypto::hmac_sha256(&key, &[&iv, &body]));
            std::hint::black_box(crate::crypto::sha256(&body));
            std::hint::black_box(crate::crypto::sha256(&jpeg));
        });
    }

    #[test]
    fn thumbnails() {
        let source = DynamicImage::new_rgb8(300, 120);
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png = png.into_inner();
        assert_eq!(image_dimensions(&png), Some((300, 120)));
        let thumb = thumbnail(&png, 32).unwrap();
        assert_eq!(image_dimensions(&thumb), Some((32, 12)));
        let avatar = profile_picture(&png).unwrap();
        assert_eq!(image_dimensions(&avatar), Some((120, 120)));
    }
}
