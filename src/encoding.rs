//! Encode images into Webp

use crate::util;
use futures::future::join_all;
use futures::future::BoxFuture;
use image::imageops::FilterType::Lanczos3;
use image::io::Reader as ImageReader;
use image::DynamicImage;
use image::GenericImageView;
use std::path::Path;

pub struct EncodeResult {
    pub data: Vec<u8>,
    pub size: (u32, u32),
    pub content_type: String,
}

/// Encode an image as a Webp from the given file path
pub async fn image_path_to_encoded<'a>(
    path: &'a Path,
    content_type: &'a str,
    opts: FromImageOptions,
) -> Result<EncodeResult, String> {
    // read the bytes of the file into an ImageReader
    let mut read_image = match ImageReader::open(path) {
        Ok(read_image) => read_image,
        Err(e) => return Err(e.to_string()),
    };

    // set the format of the ImageReader to the format of the image
    read_image.set_format(util::mimetype_to_format(content_type));

    let decoded_image = match read_image.decode() {
        Ok(decoded_image) => decoded_image,
        Err(e) => return Err(e.to_string()),
    };

    from_image(decoded_image, opts).await
}

struct CompressedImageResult {
    data: Vec<u8>,
    content_type: String,
}

/// Convert a dynamic image into a Webp
fn to_webp(im: &DynamicImage) -> Result<CompressedImageResult, String> {
    let encoder = match webp::Encoder::from_image(im) {
        Ok(i) => i,
        Err(e) => return Err(e.to_string()),
    };
    let image_bytes = (*encoder.encode(90.0)).to_vec();

    Ok(CompressedImageResult {
        data: image_bytes,
        content_type: "image/webp".to_string(),
    })
}

/// Convert a dynamic image to png
fn to_png(im: &DynamicImage) -> Result<CompressedImageResult, String> {
    let mut bytes: Vec<u8> = Vec::new();
    match im.write_to(&mut bytes, image::ImageOutputFormat::Png) {
        Ok(_) => (),
        Err(e) => return Err(e.to_string()),
    };
    let image_bytes = match oxipng::optimize_from_memory(&bytes[..], &oxipng::Options::default()) {
        Ok(r) => r,
        Err(e) => return Err(e.to_string()),
    };

    Ok(CompressedImageResult {
        data: image_bytes,
        content_type: "image/png".to_string(),
    })
}

pub struct FromImageOptions {
    /// The max width and height of the image
    pub max_size: u32,
    /// Whether it should also try compressing the image with PNG, this will be slower and often unnecessary
    pub optimize_png: bool,
}

impl Default for FromImageOptions {
    fn default() -> FromImageOptions {
        FromImageOptions {
            max_size: 1024,
            optimize_png: false,
        }
    }
}

/// Take in the current size of the image along with a new desired max height
/// and return the new size. If both the width and height are smaller than
/// the max height, their old values are returned
fn clamp_im_size(width: u32, height: u32, max_size: u32) -> (u32, u32) {
    // they're both within the size, we don't need to do anything
    if width < max_size && height < max_size {
        return (width, height);
    }

    if width > height {
        let aspect_ratio = width as f32 / max_size as f32;
        (max_size, (height as f32 / aspect_ratio) as u32)
    } else {
        let aspect_ratio = height as f32 / max_size as f32;
        ((width as f32 / aspect_ratio) as u32, max_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamp_im_size_already_smaller() {
        let (w, h) = clamp_im_size(32, 64, 64);
        assert_eq!((w, h), (32, 64));
    }
    #[test]
    fn clamp_im_height_bigger() {
        let (w, h) = clamp_im_size(64, 256, 16);
        assert_eq!((w, h), (4, 16));
    }
    #[test]
    fn clamp_im_width_bigger() {
        let (w, h) = clamp_im_size(256, 64, 16);
        assert_eq!((w, h), (16, 4));
    }
    #[test]
    fn clamp_im_uneven() {
        let (w, h) = clamp_im_size(112, 398, 256);
        assert_eq!((w, h), (72, 256));
    }
}

/// Convert a dynamic image into an optimized image
async fn from_image(im: DynamicImage, opts: FromImageOptions) -> Result<EncodeResult, String> {
    let (original_width, original_height) = im.dimensions();

    // if the image is too big, resize it to be 512x512
    let size = if original_width > opts.max_size || original_height > opts.max_size {
        let new_size = clamp_im_size(original_width, original_height, opts.max_size);
        im.resize(512, 512, Lanczos3);
        new_size
    } else {
        (original_width, original_height)
    };

    // we have to clone `im` because it will get moved
    let png_im = im.clone();

    let mut futures: Vec<BoxFuture<'static, Result<CompressedImageResult, String>>> =
        vec![Box::pin(util::run_thread(move || to_webp(&im)))];

    if opts.optimize_png {
        futures.push(Box::pin(util::run_thread(move || to_png(&png_im))));
    }

    // unbox the futures and join them
    let future_results = join_all(futures).await;

    // if any of the futures failed, return the error
    if future_results.iter().any(|r| r.is_err()) {
        return Err(future_results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .next()
            .unwrap()
            .to_string());
    }

    // find which one is smallest and set image_bytes and content_type
    let compressed_image_result = future_results
        .iter()
        .map(|r| r.as_ref().unwrap())
        .min_by_key(|r| r.data.len())
        .unwrap();

    Ok(EncodeResult {
        data: compressed_image_result.data.to_vec(),
        size,
        content_type: compressed_image_result.content_type.to_string(),
    })
}
