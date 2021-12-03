//! Handles all the database operations.

use crate::util;

use bson::spec::BinarySubtype;
use log::info;
use std::env;

use mongodb::bson::{doc, Document};
use mongodb::{
    options::{ClientOptions, ResolverConfig},
    Client, Collection,
};

pub struct Collections {
    pub images: Collection<Document>,
}

/// Check if the image with the given id exists
pub async fn check_image_exists(
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
) -> Result<String, mongodb::error::Error> {
    let mut id = util::generate_random_id(5);
    while check_image_exists(&images_collection, id.clone()).await? {
        id = util::generate_random_id(5);
    }
    Ok(id)
}

pub async fn insert_image(
    images_collection: &Collection<Document>,
    id: &String,
    image_data: Vec<u8>,
) -> Result<mongodb::results::InsertOneResult, mongodb::error::Error> {
    println!("inserting doc");
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

pub async fn get_image(
    images_collection: &Collection<Document>,
    id: String,
) -> Result<Option<Document>, mongodb::error::Error> {
    let filter = doc! {"_id": id};
    images_collection.find_one(filter, None).await
}