#[macro_use]
extern crate rocket;
mod db;
mod encoding;
mod util;

use log::info;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use std::collections::HashMap;

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

        let content_type_string = match _content_type {
            Some(t) => t.to_string(),
            None => return Err("No mimetype".to_string()),
        };

        let encoded_image = encoding::image_path_to_encoded(&_path, &content_type_string).await?;

        let image_id = match db::generate_image_id(&images_collection.images).await {
            Ok(image_id) => image_id,
            Err(e) => return Err(e.to_string()),
        };
        let insert_result = db::insert_image(
            &images_collection.images,
            &db::NewImage {
                id: &image_id,
                data: &encoded_image.data,
                content_type: &encoded_image.content_type,
            },
        )
        .await;
        // db::insert_image(&images_collection.images, &image_id, &encoded_image.data).await;
        if insert_result.is_err() {
            return Err(insert_result.err().unwrap().to_string());
        }

        info!("uploading image {}", &image_id);

        Ok(Redirect::to(uri!(view_image_route(image_id))))
    } else {
        Err("no image selected :(".to_string())
    }
}

#[derive(Responder)]
#[response(status = 200)]
struct MyResponder {
    inner: Vec<u8>,
    // header: ContentType,
    more: Header<'static>,
}

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

    let image_data: Vec<u8> = image_doc.get_binary_generic("data").unwrap().clone();
    let content_type: String = image_doc.get_str("content_type").unwrap().to_string();

    Ok(MyResponder {
        inner: image_data,
        more: Header::new("Content-Type", content_type),
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
