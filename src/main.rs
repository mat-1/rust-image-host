#[macro_use]
extern crate rocket;
use image::io::Reader as ImageReader;
use rocket_dyn_templates::Template;
use std::collections::HashMap;

extern crate rocket_multipart_form_data;

use rocket::http::ContentType;
use rocket::Data;

use jpegxl_rs::image::*;
use rocket_multipart_form_data::{
    mime, MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};

#[get("/")]
fn index() -> Template {
    let context: HashMap<String, ()> = HashMap::new();
    Template::render("index", &context)
}

#[post("/upload", data = "<data>")]
async fn image_upload(
    content_type: &ContentType,
    data: Data<'_>,
) -> Result<&'static str, io::Error> {
    let mut options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::file("image")
            .content_type_by_string(Some(mime::IMAGE_STAR))
            .unwrap(),
    ]);

    let mut multipart_form_data = MultipartFormData::parse(content_type, data, options)
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
        let img = ImageReader::open(_path)?.decode()?.to_rgba16();
        let mut encoder = encoder_builder().build()?;
        let buffer: EncoderResult<f32> = encoder.encode(&img, img.width(), img.height())?;

        // You can now deal with the uploaded file.
        "epic"
    } else {
        "no image selected :("
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, image_upload])
        .attach(Template::fairing())
}

// fn main() {
//     println!("Hello, world!");
// }
