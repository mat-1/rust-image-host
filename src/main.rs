#[macro_use]
extern crate rocket;
mod db;
mod util;

use image::imageops::FilterType::Lanczos3;
use image::io::Reader as ImageReader;
use image::GenericImageView;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use rocket_multipart_form_data::mime::Mime;
use std::collections::HashMap;
use std::path::PathBuf;

extern crate rocket_multipart_form_data;

use rocket::http::{ContentType, Header};
use rocket::Data;
use rocket::State;

use rocket_multipart_form_data::{
    mime, MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};

use dotenv::dotenv;

#[get("/")]
fn index() -> Template {
    let context: HashMap<String, ()> = HashMap::new();
    Template::render("index", &context)
}

/// Encode a jpeg from a vec of raw pixels into a vec of jpeg bytes
fn encode_jpeg(
    img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> Result<Vec<u8>, Box<dyn std::any::Any + std::marker::Send>> {
    // the mozjpeg library likes panicking, so we have to catch_unwind
    std::panic::catch_unwind(|| {
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_EXT_RGBA);
        comp.set_color_space(mozjpeg::ColorSpace::JCS_RGB);

        // compression epic gaming
        comp.set_optimize_scans(true);
        comp.set_quality(90.0);
        comp.set_size(img.width() as usize, img.height() as usize);
        comp.set_mem_dest();

        comp.start_compress();
        assert!(comp.write_scanlines(&img));
        comp.finish_compress();

        let jpeg_bytes = comp.data_to_vec().unwrap();

        Ok(jpeg_bytes)
    })?
}

/// Encode an image as a Jpeg from the given file path
fn image_path_to_jpeg(path: &PathBuf, content_type: &Option<Mime>) -> Result<Vec<u8>, String> {
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

#[post("/upload", data = "<data>")]
async fn upload_image_route(
    content_type: &ContentType,
    data: Data<'_>,
    images_collection: &State<db::Collections>,
) -> Result<Redirect, String> {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::file("image")
            .content_type_by_string(Some(mime::IMAGE_STAR))
            .unwrap(),
    ]);

    let multipart_form_data = MultipartFormData::parse(content_type, data, options)
        .await
        .unwrap();

    let image = multipart_form_data.files.get("image"); // Use the get method to preserve file fields from moving out of the MultipartFormData instance in order to delete them automatically when the MultipartFormData instance is being dropped

    if let Some(file_fields) = image {
        let file_field = &file_fields[0];

        let _content_type = &file_field.content_type;
        let _file_name = &file_field.file_name;
        let _path = &file_field.path;

        println!("content type: {:?}", _content_type);
        println!("file name: {:?}", _file_name);
        println!("path: {:?}", _path);
        let image_encoded_bytes = image_path_to_jpeg(&_path, &_content_type)?;

        let image_id = match db::generate_image_id(&images_collection.images).await {
            Ok(image_id) => image_id,
            Err(e) => return Err(e.to_string()),
        };
        let insert_result =
            db::insert_image(&images_collection.images, &image_id, image_encoded_bytes).await;
        if insert_result.is_err() {
            return Err(insert_result.err().unwrap().to_string());
        }

        dbg!(format!("uploading image {}", &image_id));

        Ok(Redirect::to(uri!(view_image_route(image_id))))
    } else {
        Err("no image selected :(".to_string())
    }
}

#[derive(Responder)]
#[response(status = 200, content_type = "image/jpeg")]
struct MyResponder {
    inner: Vec<u8>,
    header: ContentType,
    more: Header<'static>,
}

/// View the image from the database by quickly converting it to jpeg
#[get("/<id>")]
async fn view_image_route(
    id: String,
    images_collection: &State<db::Collections>,
) -> Result<MyResponder, String> {
    let image_doc_option = match db::get_image(&images_collection.images, id).await {
        Ok(image_doc) => image_doc,
        Err(e) => return Err(e.to_string()),
    };
    let image_doc = match image_doc_option {
        Some(image_doc) => image_doc,
        None => return Err("No image found".to_string()),
    };
    let image_data: &Vec<u8> = image_doc.get_binary_generic("data").unwrap();

    Ok(MyResponder {
        inner: image_data.clone(),
        header: ContentType::JPEG,
        more: Header::new("Content-Type", "image/jpeg"),
    })
}

#[launch]
async fn rocket() -> _ {
    println!("Starting server");

    dotenv().ok();

    let images_collection = db::connect().await.unwrap();

    println!("Connected to database");

    rocket::build()
        .manage(db::Collections {
            images: images_collection,
        })
        .mount("/", routes![index, upload_image_route, view_image_route])
        .attach(Template::fairing())
}
