#[macro_use]
extern crate rocket;
mod background_optimization;
mod db;
mod encoding;
mod util;

use background_optimization::{optimize_image_and_update, optimize_images_from_database};
use dotenv::dotenv;
use log::info;
use rocket::{
    http::{ContentType, Header},
    response::Redirect,
    Data, State,
};
use rocket_multipart_form_data::{
    mime, MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};
use std::path::PathBuf;
use tokio::{join, task};
use util::ImageId;

#[derive(Responder)]
#[response(status = 200)]
struct HtmlResponder {
    inner: &'static str,
    // header: ContentType,
    more: Header<'static>,
}

#[get("/")]
fn index() -> HtmlResponder {
    HtmlResponder {
        inner: include_str!("../site/index.html"),
        more: Header::new("Content-Type", "text/html; charset=utf-8"),
    }
}

/// Upload an image to the database from the Pathbuf and metadata.
async fn upload_image(
    path: PathBuf,
    content_type_string: String,
    images_collection: &mongodb::Collection<mongodb::bson::Document>,
) -> Result<ImageId, String> {
    let encoded_image_future = encoding::image_path_to_encoded(
        Box::new(path.clone()),
        &content_type_string,
        encoding::FromImageOptions::default(),
    );
    // we generate a low quality thumbnail alongside the image
    let encoded_thumbnail_future = encoding::image_path_to_encoded(
        Box::new(path),
        &content_type_string,
        encoding::FromImageOptions {
            max_size: 128,
            ..encoding::FromImageOptions::default()
        },
    );
    let image_id_future = db::generate_image_id(&images_collection);

    info!("Finished making futures image, doing encoding!");

    // encode the full image and thumbnail at the same time
    // also figure out the image id while we're doing this
    let (encoded_image_result, encoded_thumbnail_result, image_id_result) = join!(
        encoded_image_future,
        encoded_thumbnail_future,
        image_id_future
    );

    info!("Finished join");

    let (encoded_image, encoded_thumbnail) = (encoded_image_result?, encoded_thumbnail_result?);

    let image_id = match image_id_result {
        Ok(image_id) => image_id,
        Err(e) => return Err(e.to_string()),
    };

    info!("Inserting image into database");

    let insert_result = db::insert_image(
        &images_collection,
        &db::NewImage {
            id: &image_id,

            data: &encoded_image.data,
            content_type: &encoded_image.content_type,

            thumbnail_data: &encoded_thumbnail.data,
            thumbnail_content_type: &encoded_thumbnail.content_type,

            size: encoded_image.size,

            optim_level: 0,
        },
    )
    .await;
    // db::insert_image(&images_collection.images, &image_id, &encoded_image.data).await;
    if insert_result.is_err() {
        return Err(insert_result.err().unwrap().to_string());
    }

    info!("uploaded image {}", &image_id);

    let owned_images_collection = images_collection.clone();
    // optimize the image more heavily in the background so we can serve it faster
    task::spawn(async move {
        // if it fails optimizing, we don't care
        optimize_image_and_update(&owned_images_collection, insert_result.unwrap().unwrap())
            .await
            .ok();
        info!("optimized!")
    });

    Ok(image_id)
}

#[post("/", data = "<data>")]
async fn upload_image_route(
    content_type: &ContentType,
    data: Data<'_>,
    collections: &State<db::Collections>,
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
        let path = file_field.path.clone();

        println!("content type: {:?}", _content_type);
        println!("file name: {:?}", _file_name);
        println!("path: {:?}", path);

        let content_type_string = match _content_type {
            Some(t) => t.to_string(),
            None => return Err("No mimetype".to_string()),
        };

        let image_id: ImageId =
            upload_image(path, content_type_string, &collections.images).await?;

        Ok(Redirect::to(uri!(view_image_route(image_id.to_string()))))
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

// this is here for compatibility with the old version of the site
#[get("/image/<id>")]
async fn redirect_image_route(id: String) -> Redirect {
    Redirect::to(uri!(view_image_route(id)))
}

#[launch]
async fn rocket() -> _ {
    info!("Starting server");

    dotenv().ok();

    let images_collection = db::connect().await.unwrap();

    println!("Connected to database");

    let collections = db::Collections {
        images: images_collection,
    };

    let owned_images_collection = collections.images.clone();
    tokio::spawn(async move {
        optimize_images_from_database(&owned_images_collection)
            .await
            .expect("Failed optimizing images");
    });

    rocket::build().manage(collections).mount(
        "/",
        routes![
            index,
            upload_image_route,
            view_image_route,
            redirect_image_route
        ],
    )
}
