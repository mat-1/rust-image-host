//! Useful things that aren't entirely specific to this project.

use image::ImageFormat;
use rand::Rng;

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

/// Generate a random string meant to be used as an id.
pub fn generate_random_id(length: usize) -> String {
    generate_random_string(
        length,
        b"bcdfghjklmnpqrstvwxyzBCDFGHJKLMNPQRSTVWXYZ0123456789-_",
    )
}

#[test]
fn generate_random_id_works() {
    assert_eq!(generate_random_id(5).len(), 5);
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

/// Create a thread for a given function, await it, and return the result.
pub async fn run_thread<F: 'static, T: 'static>(f: F) -> T
where
    F: FnOnce() -> T + std::marker::Send,
    T: std::marker::Send,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let result = f();
        if tx.send(result).is_err() {
            panic!("failed to send result");
        }
    });
    rx.await.unwrap()
}
