#[macro_use]
extern crate rocket;
use image::io::Reader as ImageReader;
use mongodb::Collection;
use rand::{distributions::Alphanumeric, Rng};
use rocket::fs::NamedFile;
use rocket::futures::StreamExt;
use rocket::response::status::Forbidden;
use rocket_dyn_templates::Template;
use rocket_multipart_form_data::mime::Mime;
use std::io::Cursor;
use std::path::PathBuf;
use std::{collections::HashMap, fs};

extern crate rocket_multipart_form_data;

use rocket::http::{ContentType, Header};
use rocket::Data;
use rocket::State;

use kagamijxl;
use rocket_multipart_form_data::{
    mime, MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};

use dotenv::dotenv;
use std::env;

use bson::spec::BinarySubtype;
use mongodb::bson::{doc, Document};
use mongodb::{options::ClientOptions, Client};

#[get("/")]
fn index() -> Template {
    let context: HashMap<String, ()> = HashMap::new();
    Template::render("index", &context)
}

/// Generate a random alphanumeric string of the given length
fn generate_random_string(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Check if the image with the given id exists
async fn db_check_image_exists(
    images_collection: &Collection<Document>,
    id: String,
) -> Result<bool, mongodb::error::Error> {
    let filter = doc! {"_id": id};
    let counted_documents = images_collection
        .count_documents(
            filter,
            Some(mongodb::options::CountOptions::builder().limit(1).build()),
        )
        .await?;
    Ok(counted_documents > 0)
}

/// Generate a random non-duplicate image id
async fn generate_image_id(
    images_collection: &Collection<Document>,
) -> Result<String, mongodb::error::Error> {
    let mut id = generate_random_string(5);
    // we read from environ the list of phrases that are not allowed in the image id, separated by commas
    let forbidden_phrases_string = env::var("FORBIDDEN_PHRASES").unwrap();
    let mut forbidden_phrases = forbidden_phrases_string.split(",");

    while db_check_image_exists(&images_collection, id.clone()).await?
        || forbidden_phrases.any(|p| id.contains(p))
    {
        id = generate_random_string(5);
    }
    Ok(id)
}

/// Convert a mime type to an `image::ImageFormat`
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

fn image_path_to_jpegxl(path: &PathBuf, content_type: &Option<Mime>) -> Result<Vec<u8>, String> {
    let mut read_image = match ImageReader::open(path) {
        Ok(read_image) => read_image,
        Err(e) => return Err(e.to_string()),
    };
    let mimetype_string = match content_type {
        Some(mimetype_string) => mimetype_string.to_string(),
        None => return Err("No mimetype".to_string()),
    };
    read_image.set_format(mimetype_to_format(&mimetype_string.as_str()));
    let decoded_image = match read_image.decode() {
        Ok(decoded_image) => decoded_image,
        Err(e) => return Err(e.to_string()),
    };
    let img = decoded_image.to_rgba8();

    let mut encoder = kagamijxl::Encoder::default();
    encoder.basic_info.xsize = img.width();
    encoder.basic_info.ysize = img.height();
    let result = encoder.encode(&img)?;
    Ok(result)
}

async fn db_insert_image(
    images_collection: &Collection<Document>,
    id: String,
    image_data: Vec<u8>,
) -> Result<mongodb::results::InsertOneResult, mongodb::error::Error> {
    images_collection
        .insert_one(
            doc! {
                "_id": id,
                "data": bson::Binary { subtype: BinarySubtype::Generic, bytes: image_data },
                "date": bson::DateTime::now(),
                "last_seen": bson::DateTime::now(),
            },
            None,
        )
        .await
}

async fn db_get_image(
    images_collection: &Collection<Document>,
    id: String,
) -> Result<Option<Document>, mongodb::error::Error> {
    let filter = doc! {"_id": id};
    images_collection.find_one(filter, None).await
}

#[post("/upload", data = "<data>")]
async fn upload_image_route(
    content_type: &ContentType,
    data: Data<'_>,
    images_collection: &State<Collections>,
) -> Result<&'static str, String> {
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
        let result = image_path_to_jpegxl(&_path, &_content_type)?;
        fs::write("image.jxl", &result).unwrap();

        let image_id = match generate_image_id(&images_collection.images).await {
            Ok(image_id) => image_id,
            Err(e) => return Err(e.to_string()),
        };
        let insert_result = db_insert_image(&images_collection.images, image_id, result).await;
        if insert_result.is_err() {
            return Err(insert_result.err().unwrap().to_string());
        }

        // You can now deal with the uploaded file.
        Ok("epic")
    } else {
        Ok("no image selected :(")
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
    images_collection: &State<Collections>,
) -> Result<MyResponder, String> {
    let image_doc_option = match db_get_image(&images_collection.images, id).await {
        Ok(image_doc) => image_doc,
        Err(e) => return Err(e.to_string()),
    };
    let image_doc = match image_doc_option {
        Some(image_doc) => image_doc,
        None => return Err("No image found".to_string()),
    };
    let image_data: &Vec<u8> = image_doc.get_binary_generic("data").unwrap();
    // quickly convert the image data back to jpeg
    let decode_result = kagamijxl::decode_memory(&image_data)?;

    // let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = match image::ImageBuffer::from_vec(
    //     decode_result.basic_info.xsize * 2,
    //     decode_result.basic_info.ysize / 2,
    //     decode_result.frames[0].data,
    // ) {
    //     Some(img) => img,
    //     None => return Err("Could not convert image to image buffer".to_string()),
    // };

    // convert the image buffer to jpeg
    let img = image::jpeg::JpegEncoder::new(&mut Vec::new());
    if img
        .encode(
            &decode_result.frames[0].data,
            decode_result.basic_info.xsize,
            decode_result.basic_info.ysize,
            image::ColorType::Rgba8,
        )
        .is_err()
    {
        return Err("Could not convert image to jpeg".to_string());
    }

    // let img2 = match ImageReader::new(Cursor::new(img)).decode() {
    //     Ok(img2) => img2,
    //     Err(e) => return Err(e.to_string()),
    // };

    // let mut bytes: Vec<u8> = Vec::new();
    // match img.write_to(&mut bytes, image::ImageOutputFormat::Jpeg(100)) {
    //     Ok(_) => (),
    //     Err(e) => return Err(e.to_string()),
    // };

    // // let r = rocket::response::Response::build()
    // //     .header(Header::new("Content-Type", "image/jpeg"))
    // //     .sized_body(image_data.len(), Cursor::new(img))
    // //     .ok();
    // // if r.is_err() {
    // //     return Err(r.err().unwrap().to_string());
    // // }
    // // Ok(r.unwrap())

    // Ok(MyResponder {
    //     inner: img.clone(),
    //     header: ContentType::JPEG,
    //     more: Header::new("Content-Type", "image/jpeg"),
    // })
}

struct Collections {
    images: Collection<Document>,
}

#[launch]
async fn rocket() -> _ {
    println!("Starting server");

    dotenv().ok();

    println!("Loaded env variables");

    let mongodb_uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let mongodb_db_name = env::var("MONGODB_DB_NAME").expect("MONGODB_DB_NAME must be set");
    println!("MONGODB_URI: {}", mongodb_uri);
    println!("MONGODB_DB_NAME: {}", mongodb_db_name);

    let client_options = ClientOptions::parse(mongodb_uri).await.unwrap();
    println!("Connecting to mongodb");
    let client = Client::with_options(client_options).unwrap();
    let db = client.database(&mongodb_db_name);
    let images_collection = db.collection::<Document>("images");

    println!("Connected to database");

    rocket::build()
        .manage(Collections {
            images: images_collection,
        })
        .mount("/", routes![index, upload_image_route, view_image_route])
        .attach(Template::fairing())
}
