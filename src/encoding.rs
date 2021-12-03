//! Encode images into Webp

use crate::util;
use image::imageops::FilterType::Lanczos3;
use image::io::Reader as ImageReader;
use image::DynamicImage;
use image::GenericImageView;
use std::path::PathBuf;
use webp;

pub const CONTENT_TYPE: &str = "image/webp";

/// Encode an image as a Webp from the given file path
pub fn image_path_to_encoded(path: &PathBuf, content_type: &String) -> Result<Vec<u8>, String> {
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

    let (width, height) = decoded_image.dimensions();

    // if the image is too big, resize it to be 512x512
    if width * height > 512 * 512 {
        decoded_image.resize(512, 512, Lanczos3);
        // decoded_image.thumbnail(512, 512);
    }

    let image_bytes = from_image(&decoded_image);

    image_bytes
}

/// Convert a dynamic image into Webp bytes
fn from_image(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let encoder = match webp::Encoder::from_image(&image) {
        Ok(i) => i,
        Err(e) => return Err(e.to_string()),
    };
    Ok((*encoder.encode(90.0)).to_vec())
}
