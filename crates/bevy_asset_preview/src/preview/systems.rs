use core::time::Duration;

use bevy::{
    asset::{AssetEvent, AssetPath, AssetServer},
    ecs::event::{BufferedEvent, EventReader, EventWriter},
    image::Image,
    platform::collections::HashMap,
    prelude::*,
};

use crate::preview::{PreviewCache, PreviewConfig, resize_image_for_preview};

/// Generates preview images for the specified resolutions and caches them.
pub fn generate_previews_for_resolutions(
    images: &mut Assets<Image>,
    original_image: &Image,
    original_handle: Handle<Image>,
    path: &AssetPath<'static>,
    asset_id: AssetId<Image>,
    resolutions: &[u32],
    cache: &mut PreviewCache,
    timestamp: Duration,
) {
    for &resolution in resolutions {
        if cache.get_by_path(path, Some(resolution)).is_some() {
            continue;
        }

        let preview_handle = match resize_image_for_preview(original_image, resolution) {
            Some(compressed) => images.add(compressed),
            None => original_handle.clone(),
        };

        cache.insert(path, asset_id, resolution, preview_handle, timestamp);
    }
}

/// Event emitted when a preview is ready.
#[derive(Event, BufferedEvent, Debug, Clone)]
pub struct PreviewReady {
    /// Task ID of the preview request.
    pub task_id: u64,
    /// Asset path.
    pub path: AssetPath<'static>,
    /// Preview image handle.
    pub image_handle: Handle<Image>,
}

/// Event emitted when a preview generation fails.
#[derive(Event, BufferedEvent, Debug, Clone)]
pub struct PreviewFailed {
    /// Task ID of the preview request.
    pub task_id: u64,
    /// Asset path.
    pub path: AssetPath<'static>,
    /// Error message.
    pub error: String,
}

/// Component that tracks a pending preview request.
#[derive(Component, Debug)]
pub struct PendingPreviewRequest {
    /// Task ID.
    pub task_id: u64,
    /// Asset path.
    pub path: AssetPath<'static>,
}

/// Resource that manages preview generation tasks.
#[derive(Resource, Default)]
pub struct PreviewTaskManager {
    /// Next task ID.
    next_task_id: u64,
    /// Maps task ID to request entity.
    task_to_entity: HashMap<u64, Entity>,
}

impl PreviewTaskManager {
    /// Creates a new task manager.
    pub fn new() -> Self {
        Self {
            next_task_id: 0,
            task_to_entity: HashMap::new(),
        }
    }

    /// Creates a new task ID.
    pub fn create_task_id(&mut self) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }

    /// Registers a task entity.
    pub fn register_task(&mut self, task_id: u64, entity: Entity) {
        self.task_to_entity.insert(task_id, entity);
    }

    /// Gets entity for a task ID.
    pub fn get_entity(&self, task_id: u64) -> Option<Entity> {
        self.task_to_entity.get(&task_id).copied()
    }

    /// Removes task registration.
    pub fn remove_task(&mut self, task_id: u64) {
        self.task_to_entity.remove(&task_id);
    }
}

/// Requests a preview for an image asset path.
/// Returns the task ID for tracking.
/// Uses highest configured resolution if resolution is None.
pub fn request_image_preview<'a>(
    mut commands: Commands,
    mut task_manager: ResMut<PreviewTaskManager>,
    mut cache: ResMut<PreviewCache>,
    config: Res<PreviewConfig>,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut images_mut: ResMut<Assets<Image>>,
    mut preview_ready_events: EventWriter<PreviewReady>,
    time: Res<Time<Real>>,
    path: impl Into<AssetPath<'a>>,
    resolution: Option<u32>,
) -> u64 {
    let path: AssetPath<'static> = path.into().into_owned();

    if let Some(cache_entry) = cache.get_by_path(&path, resolution) {
        let task_id = task_manager.create_task_id();
        preview_ready_events.write(PreviewReady {
            task_id,
            path: path.clone(),
            image_handle: cache_entry.image_handle.clone(),
        });
        return task_id;
    }

    let task_id = task_manager.create_task_id();
    let handle: Handle<Image> = asset_server.load(&path);

    if let Some(image) = images.get(&handle) {
        let image_clone = image.clone();
        let asset_id = handle.id();

        generate_previews_for_resolutions(
            &mut images_mut,
            &image_clone,
            handle.clone(),
            &path,
            asset_id,
            &config.resolutions,
            &mut cache,
            time.elapsed(),
        );

        let preview_handle = cache
            .get_by_path(&path, resolution)
            .map(|entry| entry.image_handle.clone())
            .unwrap_or_else(|| handle.clone());

        preview_ready_events.write(PreviewReady {
            task_id,
            path: path.clone(),
            image_handle: preview_handle,
        });
        return task_id;
    }

    let entity = commands
        .spawn(PendingPreviewRequest {
            task_id,
            path: path.clone(),
        })
        .id();
    task_manager.register_task(task_id, entity);
    task_id
}

/// System that handles image asset events for previews.
pub fn handle_image_preview_events(
    mut commands: Commands,
    mut cache: ResMut<PreviewCache>,
    config: Res<PreviewConfig>,
    asset_server: Res<AssetServer>,
    mut preview_ready_events: EventWriter<PreviewReady>,
    mut preview_failed_events: EventWriter<PreviewFailed>,
    mut asset_events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    requests: Query<(Entity, &PendingPreviewRequest)>,
    mut task_manager: ResMut<PreviewTaskManager>,
    mut ready_events: EventReader<PreviewReady>,
    time: Res<Time<Real>>,
) {
    for event in ready_events.read() {
        let _ = cache.get_by_path(&event.path, None);
    }
    for event in asset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                for (entity, request) in requests.iter() {
                    let handle: Handle<Image> = asset_server.load(&request.path);
                    if handle.id() == *id {
                        if let Some(image) = images.get(&handle) {
                            let image_clone = image.clone();
                            let asset_id = handle.id();

                            generate_previews_for_resolutions(
                                &mut images,
                                &image_clone,
                                handle.clone(),
                                &request.path,
                                asset_id,
                                &config.resolutions,
                                &mut cache,
                                time.elapsed(),
                            );

                            let preview_image = cache
                                .get_by_path(&request.path, None)
                                .map(|entry| entry.image_handle.clone())
                                .unwrap_or_else(|| handle.clone());

                            preview_ready_events.write(PreviewReady {
                                task_id: request.task_id,
                                path: request.path.clone(),
                                image_handle: preview_image,
                            });

                            task_manager.remove_task(request.task_id);
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
            AssetEvent::Removed { id } => {
                cache.remove_by_id(*id, None);

                for (entity, request) in requests.iter() {
                    let handle: Handle<Image> = asset_server.load(&request.path);
                    if handle.id() == *id {
                        preview_failed_events.write(PreviewFailed {
                            task_id: request.task_id,
                            path: request.path.clone(),
                            error: "Image asset was removed".to_string(),
                        });
                        task_manager.remove_task(request.task_id);
                        commands.entity(entity).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}
