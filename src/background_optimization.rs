//! This is responsible for optimizing images in the background, like how right
//! after we upload an image we do some heavier work to compress the image

use std::io::Cursor;

use crate::encoding::{from_image, FromImageOptions};
use crate::{db, util};
use bson::Document;
use futures::join;
use futures::stream::TryStreamExt;
use image::io::Reader;
use mongodb::bson::doc;
use mongodb::Collection;

/// Optimize an image from the database and bump its compression level.
pub async fn optimize_image_and_update(
    images_collection: &Collection<Document>,
    image_doc: Document,
) -> Result<(), String> {
    let image_id = image_doc.get_str("_id").expect("Image id must be a string");
    let image_bytes = image_doc
        .get_binary_generic("data")
        .expect("data must be set");
    let content_type = image_doc
        .get_str("content_type")
        .expect("content_type must be set");
    let optimization_level = image_doc
        .get_i32("optim_level")
        .expect("optim_level must be set") as u8;

    // create a DynamicImage from the bytes and content type
    let mut read_image = Reader::new(Cursor::new(image_bytes));

    read_image.set_format(util::mimetype_to_format(content_type));

    let image = read_image.decode().map_err(|e| e.to_string())?;

    let encoded_image_future = match optimization_level {
        0 => from_image(
            image.clone(),
            FromImageOptions {
                optimize_png: true,
                ..FromImageOptions::default()
            },
        ),
        _ => return Err("This image is already too compressed!".to_string()),
    };

    let encoded_thumbnail_future = from_image(
        image,
        FromImageOptions {
            optimize_png: true,
            max_size: 128,
            ..FromImageOptions::default()
        },
    );

    let (encoded_image_result, encoded_thumbnail_result) =
        join!(encoded_image_future, encoded_thumbnail_future);
    let (encoded_image, encoded_thumbnail) = (encoded_image_result?, encoded_thumbnail_result?);

    db::insert_image(
        images_collection,
        &db::NewImage {
            id: &image_id.to_string(),

            data: &encoded_image.data,
            content_type: &encoded_image.content_type,

            thumbnail_data: &encoded_thumbnail.data,
            thumbnail_content_type: &encoded_thumbnail.content_type,

            size: encoded_image.size,

            optim_level: optimization_level + 1,
        },
    )
    .await
    .map_err(|e| "Inserting into database failed")?;

    Ok(())
}

/// Find images that should be optimized from the database and optimize them
pub async fn optimize_images_from_database(
    images_collection: &Collection<Document>,
) -> Result<(), String> {
    // images with an optimization level of 0
    let mut images_cursor = images_collection
        .find(
            doc! {
                "optim_level": 0
            },
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    while let Some(im) = images_cursor.try_next().await.map_err(|e| e.to_string())? {
        // if there's an error, just ignore it
        optimize_image_and_update(images_collection, im).await.ok();
    }

    Ok(())
}
