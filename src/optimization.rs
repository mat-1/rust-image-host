//! We store a queue of images that we need to compress

use std::collections::VecDeque;

use crate::encoding::{from_image, FromImageOptions};
use bson::Document;
use core::time::Duration;
use futures::stream::TryStreamExt;
use mongodb::bson::doc;
use mongodb::Collection;
use tokio::sync::Mutex;


async fn optimize_image_and_update(image_doc: Document) {
    let image_id = image_doc.get_str("_id").unwrap();
    let image_bytes = image_doc.get_binary_generic("data").unwrap();
    let optimization_level = image_doc.get_i32("optim_level");

    let encoded_image_future = from_image(FromImageOptions {
        optimize_png: true,
        ..FromImageOptions::default()
    }).await;
    let encoded_thumbnail_future = from_image(FromImageOptions {
        optimize_png: true,
        max_size: 128,
        ..FromImageOptions::default()
    })

    let (encoded_image_result, encoded_thumbnail_result) =
        join!(encoded_image_future, encoded_thumbnail_future);
    let (encoded_image, encoded_thumbnail) = (encoded_image_result?, encoded_thumbnail_result?);

    await db::insert_image(&images_collection.images,
        &db::NewImage {
            id: &image_id,

            data: &encoded_image.data,
            content_type: &encoded_image.content_type,

            thumbnail_data: &encoded_thumbnail.data,
            thumbnail_content_type: &encoded_thumbnail.content_type,

            size: encoded_image.size,

            optim_level: 0,
        }
    )
}

/// Find images that should be optimized from the database and optimize them
async fn optimize_images_from_database(
    queue: &OptimizationQueue,
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
        if let Ok(image_id) = im.get_str("_id") {
            let mut queue_contents = queue.queue.lock().await;
            queue_contents.push_back(image_id.to_string());
        }
    }

    Ok(())
}

// // Optimize all the images that we have in the queue
// pub async fn optimize_images_from_queue(
//     queue: &OptimizationQueue,
//     images_collection: &Collection<Document>,
// ) {
//     await queue_unoptimized_images_from_database();

//     let mut queue_contents = queue.queue.lock().await;

//     if let Some(image_id) = queue_contents.pop_front() {
//         let image = await
//         // if the queue has stuff, optimize this image
//         let encoded_image_future = from_image(FromImageOptions {
//             optimize_png: true,
//             ..FromImageOptions::default()
//         }).await;
//         let encoded_thumbnail_future = from_image(FromImageOptions {
//             optimize_png: true,
//             max_size: 128,
//             ..FromImageOptions::default()
//         })

//         let (encoded_image_result, encoded_thumbnail_result) =
//             join!(encoded_image_future, encoded_thumbnail_future);
//         let (encoded_image, encoded_thumbnail) = (encoded_image_result?, encoded_thumbnail_result?);

//         await db::insert_image(&images_collection.images,
//             &db::NewImage {
//                 id: &image_id,

//                 data: &encoded_image.data,
//                 content_type: &encoded_image.content_type,

//                 thumbnail_data: &encoded_thumbnail.data,
//                 thumbnail_content_type: &encoded_thumbnail.content_type,

//                 size: encoded_image.size,

//                 optim_level: 0,
//             }
//         )

//     } else {
//         // if there's nothing in the queue, wait 10 seconds and try again
//         tokio::time::sleep(Duration::from_secs(10));
//     }

// }
