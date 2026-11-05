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
    encode_jpeg(&image.resize_exact(target_w, target_h, FilterType::Triangle), 50)
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
    fn makes_thumbnails_and_square_profile_pictures() {
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
