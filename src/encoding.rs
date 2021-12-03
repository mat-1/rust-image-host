//! Encode images into Webp

/// Encode an image as a Jpeg from the given file path
fn image_path_to_encoded(path: &PathBuf, content_type: &Option<Mime>) -> Result<Vec<u8>, String> {
    // read the bytes of the file into an ImageReader
    let mut read_image = match ImageReader::open(path) {
        Ok(read_image) => read_image,
        Err(e) => return Err(e.to_string()),
    };

    let mimetype_string = match content_type {
        Some(mimetype_string) => mimetype_string.to_string(),
        None => return Err("No mimetype".to_string()),
    };

    // set the format of the ImageReader to the format of the image
    read_image.set_format(util::mimetype_to_format(&mimetype_string.as_str()));

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

    let img = decoded_image.to_rgba8();

    let jpeg_bytes_result = encode_jpeg(img);
    match jpeg_bytes_result {
        Ok(jpeg_bytes) => Ok(jpeg_bytes),
        Err(_) => Err("Jpeg encoding failed".to_string()),
    }
}
