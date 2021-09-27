#[macro_use]
extern crate rocket;
use image::io::Reader as ImageReader;
use rocket_dyn_templates::Template;
use std::{collections::HashMap, fs};

extern crate rocket_multipart_form_data;

use rocket::http::ContentType;
use rocket::Data;
use rocket::State;

use kagamijxl;
use rocket_multipart_form_data::{
    mime, MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};

use dotenv::dotenv;
use std::env;

use mongodb::bson::{doc, Document};
use mongodb::{options::ClientOptions, Client};

#[get("/")]
fn index() -> Template {
    let context: HashMap<String, ()> = HashMap::new();
    Template::render("index", &context)
}

fn mimetype_to_format(mimetype: &str) -> image::ImageFormat {
    match mimetype {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/gif" => image::ImageFormat::Gif,
        "image/webp" => image::ImageFormat::WebP,
        "image/pnm" => image::ImageFormat::Pnm,
        "image/tiff" => image::ImageFormat::Tiff,
        "image/tga" => image::ImageFormat::Tga,
        "image/dds" => image::ImageFormat::Dds,
        "image/bmp" => image::ImageFormat::Bmp,
        "image/ico" => image::ImageFormat::Ico,
        "image/hdr" => image::ImageFormat::Hdr,
        "image/farbfeld" => image::ImageFormat::Farbfeld,
        "image/avif" => image::ImageFormat::Avif,
        // idk just go with jpeg it'll probably fail
        _ => image::ImageFormat::Jpeg,
    }
}

#[post("/upload", data = "<data>")]
async fn image_upload(content_type: &ContentType, data: Data<'_>) -> Result<&'static str, String> {
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
        let mut read_image = match ImageReader::open(_path) {
            Ok(read_image) => read_image,
            Err(e) => return Err(e.to_string()),
        };
        let mimetype_string = match _content_type {
            Some(mimetype_string) => mimetype_string.to_string(),
            None => return Err("No mimetype".to_string()),
        };
        read_image.set_format(mimetype_to_format(&mimetype_string.as_str()));
        let decoded_image = match read_image.decode() {
            Ok(decoded_image) => decoded_image,
            Err(e) => return Err(e.to_string()),
        };
        let img = decoded_image.to_rgba8();
        let result = kagamijxl::encode_memory(&img, img.width() as usize, img.height() as usize)?;
        fs::write("image.jxl", result).unwrap();

        // You can now deal with the uploaded file.
        Ok("epic")
    } else {
        Ok("no image selected :(")
    }
}

#[launch]
async fn rocket() -> _ {
    dotenv().ok();

    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let mongodb_db_name = env::var("MONGODB_DB_NAME").expect("MONGODB_DB_NAME must be set");

    let client_options = ClientOptions::parse(mongodb_uri).await.unwrap();
    let client = Client::with_options(client_options).unwrap();
    let db = client.database(&mongodb_db_name);
    let images_collection = db.collection::<Document>("images");

    rocket::build()
        .manage(db)
        .mount("/", routes![index, image_upload])
        .attach(Template::fairing())
}
