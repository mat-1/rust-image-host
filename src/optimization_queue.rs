use std::collections::VecDeque;

use bson::Document;
use mongodb::Collection;

use mongodb::bson::doc;

use futures::stream::TryStreamExt;
use tokio::sync::Mutex;

pub struct OptimizationQueue {
    /// A queue of image ids that we need to optimize
    pub queue: Mutex<VecDeque<String>>,
}

impl OptimizationQueue {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }
}

/// Find images that should be optimized from the database and optimize them
pub async fn optimize_images(
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
    let mut queue_contents = queue.queue.lock().await;
    while let Some(im) = images_cursor.try_next().await.map_err(|e| e.to_string())? {
        if let Ok(image_id) = im.get_str("_id") {
            queue_contents.push_back(image_id.to_string());
        }
    }

    Ok(())
}
