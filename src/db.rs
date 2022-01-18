//! Handles all the database operations.

use crate::util;

use bson::spec::BinarySubtype;
use log::info;
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, FindOneAndUpdateOptions, ResolverConfig, ReturnDocument},
    results::UpdateResult,
    Client, Collection,
};
use std::env;
use util::ImageId;

pub struct Collections {
    pub images: Collection<Document>,
}

pub struct NewImage<'a> {
    pub id: &'a ImageId,
    pub size: (u32, u32),

    /// How optimized the image is.
    /// 0 means the image was *just* uploaded with minimal optimization.
    pub optim_level: u8,

    pub data: &'a Vec<u8>,
    pub content_type: &'a str,

    pub thumbnail_data: &'a Vec<u8>,
    pub thumbnail_content_type: &'a str,
}

/// Check if the image with the given id exists
pub async fn check_image_exists(
    images_collection: &Collection<Document>,
    id: ImageId,
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

/// Connect to the MongoDB database
pub async fn connect() -> Result<mongodb::Collection<bson::Document>, String> {
    // read the mongodb_uri env variable
    let mongodb_uri = match env::var("MONGODB_URI") {
        Ok(val) => val,
        Err(_) => return Err("MONGODB_URI must be set".to_string()),
    };
    // read the mongodb_db_name env variable
    let mongodb_db_name = match env::var("MONGODB_DB_NAME") {
        Ok(val) => val,
        Err(_) => return Err("MONGODB_DB_NAME must be set".to_string()),
    };

    info!("Parsing mongodb uri: {}", mongodb_uri);
    // create the client options, we specify cloudflare because otherwise it takes forever to resolve a dns thing on windows
    // https://github.com/mongodb/mongo-rust-driver#windows-dns-note
    let client_options =
        match ClientOptions::parse_with_resolver_config(mongodb_uri, ResolverConfig::cloudflare())
            .await
        {
            Ok(val) => val,
            Err(err) => return Err(err.to_string()),
        };

    let client = match Client::with_options(client_options) {
        Ok(val) => val,
        Err(err) => return Err(err.to_string()),
    };
    let db = client.database(&mongodb_db_name);
    let images_collection = db.collection::<Document>("images");

    info!("Pinging database");
    match client
        .database("admin")
        .run_command(doc! {"ping": 1}, None)
        .await
    {
        Ok(val) => val,
        Err(err) => return Err(err.to_string()),
    };

    Ok(images_collection)
}

/// Generate a random non-duplicate image id
pub async fn generate_image_id(
    images_collection: &Collection<Document>,
) -> Result<ImageId, mongodb::error::Error> {
    info!("generating image id");
    let mut id = util::generate_random_id(5);
    while check_image_exists(images_collection, id.clone()).await? {
        id = util::generate_random_id(5);
    }
    info!("generated image id");
    Ok(id)
}

/// Insert or update the content of an image
pub async fn insert_image(
    images_collection: &Collection<Document>,
    image: &NewImage<'_>,
) -> Result<Option<bson::Document>, mongodb::error::Error> {
    info!("inserting doc");
    images_collection
        .find_one_and_update(
            doc! {
                "_id": image.id,
            },
            doc! {
                "$setOnInsert": {
                    "date": bson::DateTime::now(),                    
                    "last_seen": bson::DateTime::now(),
                },
                "$set": {
                    "data": bson::Binary { subtype: BinarySubtype::Generic, bytes: image.data.to_vec() },
                    "content_type": image.content_type,

                    "width": image.size.0,
                    "height": image.size.0,

                    "thumbnail_data": bson::Binary { subtype: BinarySubtype::Generic, bytes: image.thumbnail_data.to_vec() },
                    "thumbnail_content_type": image.thumbnail_content_type,

                    "optim_level": image.optim_level as i32
                }
            },
            FindOneAndUpdateOptions ::builder().upsert(true).return_document(ReturnDocument ::After).build()
        )
        .await
}

/// Bump the "last_seen" value on an image to now
pub async fn update_last_seen(
    images_collection: &Collection<Document>,
    image_id: &ImageId,
) -> Result<UpdateResult, mongodb::error::Error> {
    images_collection
        .update_one(
            doc! {
                "_id": image_id.to_string(),
            },
            doc! {
                "$set": {
                    "last_seen": bson::DateTime::now(),
                }
            },
            None,
        )
        .await
}

pub async fn get_image(
    images_collection: &Collection<Document>,
    id: &str,
) -> Result<Option<Document>, mongodb::error::Error> {
    let filter = doc! {"_id": id};
    images_collection.find_one(filter, None).await
}
