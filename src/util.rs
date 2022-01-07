//! Useful things that aren't entirely specific to this project.

use image::ImageFormat;
use mongodb::bson::Bson;
use rand::Rng;
use std::fmt;

/// Generate a random string of the given length using the given charset.
pub fn generate_random_string(length: usize, charset: &[u8]) -> String {
    let mut rng = rand::thread_rng();

    let random_string: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect();
    random_string
}

/// A randomly generated id of an image in the database
#[derive(Clone, fmt::Debug)]
pub struct ImageId(pub String);

impl TryFrom<Bson> for ImageId {
    type Error = &'static str;

    fn try_from(item: Bson) -> Result<Self, Self::Error> {
        match item {
            Bson::String(v) => Ok(Self(v)),
            _ => Err("Only bson strings can be converted into ImageId"),
        }
    }
}

impl From<ImageId> for Bson {
    fn from(item: ImageId) -> Self {
        Self::String(item.0)
    }
}

impl fmt::Display for ImageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generate a random string meant to be used as an id.
pub fn generate_random_id(length: usize) -> ImageId {
    ImageId(generate_random_string(
        length,
        b"bcdfghjklmnpqrstvwxyzBCDFGHJKLMNPQRSTVWXYZ0123456789-_",
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn generate_random_id_works() {
        assert_eq!(generate_random_id(5).0.len(), 5);
    }
}

/// Convert a string mime type to an `ImageFormat`, default to Jpeg if not found.
pub fn mimetype_to_format(mimetype: &str) -> ImageFormat {
    match mimetype {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        "image/pnm" => ImageFormat::Pnm,
        "image/tiff" => ImageFormat::Tiff,
        "image/tga" => ImageFormat::Tga,
        "image/dds" => ImageFormat::Dds,
        "image/bmp" => ImageFormat::Bmp,
        "image/ico" => ImageFormat::Ico,
        "image/hdr" => ImageFormat::Hdr,
        "image/farbfeld" => ImageFormat::Farbfeld,
        "image/avif" => ImageFormat::Avif,
        // idk just go with jpeg it'll probably fail
        _ => ImageFormat::Jpeg,
    }
}
