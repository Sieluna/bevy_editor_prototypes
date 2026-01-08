use std::io::Cursor;

use bevy::{
    asset::{AssetPath, io::ErasedAssetWriter},
    ecs::event::{BufferedEvent, Event},
    image::{Image, ImageFormat},
    platform::collections::HashMap,
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, futures_lite},
};

/// Active save task tracking component.
#[derive(Component)]
pub struct ActiveSaveTask {
    pub task_id: u64,
    pub path: AssetPath<'static>,
    pub target_path: AssetPath<'static>,
    pub task: Task<Result<(), String>>,
}

/// Event emitted when a save task completes.
#[derive(Event, BufferedEvent, Debug, Clone)]
pub struct SaveCompleted {
    pub task_id: u64,
    pub path: AssetPath<'static>,
    pub result: Result<(), String>,
}

/// Resource for save task tracking.
#[derive(Resource, Default)]
pub struct SaveTaskTracker {
    next_task_id: u64,
    pending_saves: HashMap<u64, AssetPath<'static>>,
}

impl SaveTaskTracker {
    /// Creates a new save task ID.
    pub fn create_task_id(&mut self) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }

    /// Registers a pending save.
    pub fn register_pending(&mut self, task_id: u64, path: AssetPath<'static>) {
        self.pending_saves.insert(task_id, path);
    }

    /// Marks a save as completed.
    pub fn mark_completed(&mut self, task_id: u64) {
        self.pending_saves.remove(&task_id);
    }
}

/// Saves an image asset to the specified path asynchronously using AssetWriter abstraction.
pub fn save_image<'a>(
    image: Handle<Image>,
    target_path: impl Into<AssetPath<'a>>,
    images: &Assets<Image>,
    writer: impl ErasedAssetWriter,
) -> Task<Result<(), String>> {
    let target_path: AssetPath<'static> = target_path.into().into_owned();

    let Some(image_data) = images.get(&image) else {
        let error = format!("Image not found: {:?}", image);
        return IoTaskPool::get().spawn(async move { Err(error) });
    };

    // Convert to dynamic image
    let dynamic_image = match image_data.clone().try_into_dynamic() {
        Ok(img) => img,
        Err(e) => {
            let error = format!("Failed to convert image: {:?}", e);
            return IoTaskPool::get().spawn(async move { Err(error) });
        }
    };

    // Convert to RGBA8 format
    let rgba_image = dynamic_image.into_rgba8();

    let task_pool = IoTaskPool::get();
    let target_path_clone = target_path.clone();
    // Ensure the file extension is .webp for WebP format
    let mut target_path_for_writer = target_path.path().to_path_buf();
    if let Some(ext) = target_path_for_writer.extension() {
        if ext.to_str() != Some("webp") {
            target_path_for_writer.set_extension("webp");
        }
    } else {
        target_path_for_writer.set_extension("webp");
    }

    task_pool.spawn(async move {
        // Create directory first
        if let Some(parent) = target_path_for_writer.parent() {
            if let Err(e) = writer.create_directory(parent).await {
                let error = format!("Failed to create directory {:?}: {:?}", parent, e);
                error!("{}", error);
                return Err(error);
            }
        }

        // Encode PNG directly to memory
        let mut cursor = Cursor::new(Vec::new());
        match rgba_image.write_to(
            &mut cursor,
            ImageFormat::WebP.as_image_crate_format().unwrap(), // unwrap is safe because we enable bevy webp feature
        ) {
            Ok(_) => {
                let webp_bytes = cursor.into_inner();
                // Write via AssetWriter (atomic operation)
                match writer
                    .write_bytes(&target_path_for_writer, &webp_bytes)
                    .await
                {
                    Ok(_) => {
                        info!("Image saved successfully to {:?}", target_path_clone);
                        Ok(())
                    }
                    Err(e) => {
                        let error =
                            format!("Failed to save image to {:?}: {:?}", target_path_clone, e);
                        error!("{}", error);
                        Err(error)
                    }
                }
            }
            Err(e) => {
                let error = format!("Failed to encode image to WebP: {:?}", e);
                error!("{}", error);
                Err(error)
            }
        }
    })
}

/// System that monitors save completion and cleans up tasks.
pub fn monitor_save_completion(
    mut save_completed_events: EventWriter<SaveCompleted>,
    mut tracker: ResMut<SaveTaskTracker>,
    mut commands: Commands,
    mut save_task_query: Query<(Entity, &mut ActiveSaveTask)>,
) {
    for (entity, mut active_task) in save_task_query.iter_mut() {
        // Poll the async task
        if let Some(result) = block_on(futures_lite::future::poll_once(&mut active_task.task)) {
            // Task completed, send event
            save_completed_events.write(SaveCompleted {
                task_id: active_task.task_id,
                path: active_task.path.clone(),
                result,
            });

            tracker.mark_completed(active_task.task_id);
            commands.entity(entity).despawn();
        }
    }
}

/// System that handles save completion events.
pub fn handle_save_completed(mut save_completed_events: EventReader<SaveCompleted>) {
    for event in save_completed_events.read() {
        match &event.result {
            Ok(_) => {
                debug!(
                    "Save task {} completed successfully for {:?}",
                    event.task_id, event.path
                );
            }
            Err(e) => {
                warn!(
                    "Save task {} failed for {:?}: {}",
                    event.task_id, event.path, e
                );
            }
        }
    }
}
