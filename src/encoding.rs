//! Encode images into the formats that we use

use crate::util;
use futures::future::join_all;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::DynamicImage;
use image::GenericImageView;
use std::io::Cursor;
use std::{fmt::Debug, path::PathBuf};
use tokio::task;
use tokio::task::JoinHandle;

pub struct EncodeResult {
    pub data: Vec<u8>,
    pub size: (u32, u32),
    pub content_type: String,
}

/// Encode an image as a Webp from the given file path
pub async fn image_path_to_encoded(
    path: Box<PathBuf>,
    content_type: &'_ str,
    opts: FromImageOptions,
) -> Result<EncodeResult, String> {
    info!("reading file");
    // read the bytes of the file into an ImageReader

    let read_image = task::spawn_blocking(move || ImageReader::open(*path))
        .await
        .unwrap();

    let mut read_image = match read_image {
        Ok(read_image) => read_image,
        Err(e) => return Err(e.to_string()),
    };
    info!("decoding");

    // set the format of the ImageReader to the format of the image
    read_image.set_format(util::mimetype_to_format(content_type));

    let decoded_image: DynamicImage = task::spawn_blocking(move || read_image.decode())
        .await
        .unwrap()
        .map_err(|_| "Error decoding image".to_string())?;

    info!("decoded file");

    from_image(decoded_image, opts).await
}

struct CompressedImageResult {
    data: Vec<u8>,
    content_type: String,
}

/// Convert a dynamic image into a Webp
fn to_webp(im: &DynamicImage) -> Result<CompressedImageResult, String> {
    info!("encoding webp");
    let encoder = match webp::Encoder::from_image(im) {
        Ok(i) => i,
        Err(e) => return Err(format!("Error making encoder for webp: {}", e.to_string())),
    };
    let image_bytes = (*encoder.encode(90.0)).to_vec();
    info!("encoded webp");

    Ok(CompressedImageResult {
        data: image_bytes,
        content_type: "image/webp".to_string(),
    })
}

/// Convert a dynamic image to png
fn to_png(im: &DynamicImage) -> Result<CompressedImageResult, String> {
    let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    match im.write_to(&mut bytes, image::ImageOutputFormat::Png) {
        Ok(_) => (),
        Err(e) => return Err(format!("Error writing png: {}", e.to_string())),
    };
    let image_bytes =
        match oxipng::optimize_from_memory(&bytes.into_inner()[..], &oxipng::Options::default()) {
            Ok(r) => r,
            Err(e) => return Err(format!("Error optimizing png: {}", e.to_string())),
        };

    Ok(CompressedImageResult {
        data: image_bytes,
        content_type: "image/png".to_string(),
    })
}

#[non_exhaustive]
#[derive(Debug)]
pub struct FromImageOptions {
    /// The max width and height of the image
    pub max_size: Option<u32>,
    /// Whether it should also try compressing the image with PNG in parallel, this will be slower and often unnecessary
    pub optimize_png: bool,
}

impl Default for FromImageOptions {
    fn default() -> FromImageOptions {
        FromImageOptions {
            max_size: None,
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
pub async fn from_image<'a>(
    original_im: DynamicImage,
    opts: FromImageOptions,
) -> Result<EncodeResult, String> {
    info!("from_image {:?}", opts);
    let (original_width, original_height) = original_im.dimensions();
    info!("dimensions: {} {}", original_width, original_height);

    // if the image is too big, resize it to be 512x512
    let (size, im) = if let Some(max_size) = opts.max_size {
        if original_width > max_size || original_height > max_size {
            let new_size = clamp_im_size(original_width, original_height, max_size);

            // we use nearest resizing because it's fast, in the future i should use fast_image_resize so it's even faster, maybe
            // task::spawn_blocking(move || im.resize_exact(512, 512, FilterType::Nearest))
            //     .await
            //     .unwrap();
            let new_im = task::spawn_blocking(move || {
                original_im.resize_exact(new_size.0, new_size.1, FilterType::Lanczos3)
            })
            .await
            .unwrap();

            (new_size, new_im)
        } else {
            ((original_width, original_height), original_im)
        }
    } else {
        ((original_width, original_height), original_im)
    };

    info!("did resize step");

    // we have to clone `im` because it will get moved
    // it's probably possible to not have to clone but i don't think it matters
    info!("cloning");
    let webp_im = im.clone();
    let png_im = im.clone();
    info!("cloned, now creating futures (this should be instant)");

    let mut futures: Vec<JoinHandle<Result<CompressedImageResult, String>>> =
        vec![task::spawn_blocking(move || to_webp(&webp_im))];

    if opts.optimize_png {
        futures.push(task::spawn_blocking(move || to_png(&png_im)));
    }
    info!("created futures; joining");
    // unbox the futures and join them
    let future_results = join_all(futures).await;
    info!("Did compression");

    // unwrap the first set of results
    let future_results: Vec<_> = future_results.iter().map(|r| r.as_ref().unwrap()).collect();

    // find which one is smallest and set image_bytes and content_type
    let compressed_image_result = future_results
        .iter()
        .filter(|r| r.as_ref().err().is_none())
        .map(|r| r.as_ref().unwrap())
        .min_by_key(|r| r.data.len())
        .unwrap();
    info!("finished from_image {:?}", opts);

    Ok(EncodeResult {
        data: compressed_image_result.data.to_vec(),
        size,
        content_type: compressed_image_result.content_type.to_string(),
    })
}
