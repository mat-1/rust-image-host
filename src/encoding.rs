//! Encode images into Webp

use crate::util;
use image::imageops::FilterType::Lanczos3;
use image::io::Reader as ImageReader;
use image::DynamicImage;
use image::GenericImageView;
use std::path::PathBuf;
use webp;
use oxipng;

pub struct EncodeResult<'a> {
    pub data: Vec<u8>,
    pub size: (u32, u32),
    pub content_type: &'a str,
}

/// Encode an image as a Webp from the given file path
pub fn image_path_to_encoded<'a>(
    path: &'a PathBuf,
    content_type: &'a String,
) -> Result<EncodeResult<'a>, String> {
    // read the bytes of the file into an ImageReader
    let mut read_image = match ImageReader::open(path) {
        Ok(read_image) => read_image,
        Err(e) => return Err(e.to_string()),
    };

    // set the format of the ImageReader to the format of the image
    read_image.set_format(util::mimetype_to_format(&content_type.as_str()));

    let decoded_image = match read_image.decode() {
        Ok(decoded_image) => decoded_image,
        Err(e) => return Err(e.to_string()),
    };

    from_image(decoded_image)
}

/// Convert a dynamic image into a Webp
fn to_webp(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let encoder = match webp::Encoder::from_image(&image) {
        Ok(i) => i,
        Err(e) => return Err(e.to_string()),
    };
    let image_bytes = (*encoder.encode(90.0)).to_vec();

    Ok(image_bytes)
}

/// Convert a dynamic image into an optimized image
fn from_image(image: DynamicImage) -> Result<EncodeResult<'static>, String> {
    let (width, height) = image.dimensions();

    // if the image is too big, resize it to be 512x512
    if width * height > 512 * 512 {
        image.resize(512, 512, Lanczos3);
        // decoded_image.thumbnail(512, 512);
    }
    
    let image_bytes = to_webp(&image)?;

    Ok(EncodeResult {
        data: image_bytes,
        size: (width, height),
        content_type: "image/webp",
    })
}
