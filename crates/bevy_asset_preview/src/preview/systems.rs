use bevy::{
    asset::{AssetEvent, AssetPath, AssetServer},
    ecs::event::{BufferedEvent, EventReader, EventWriter},
    image::Image,
    platform::collections::HashMap,
    prelude::*,
};

use crate::preview::{PreviewCache, resize_image_for_preview};

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
/// This is a helper that can be used in systems or other contexts.
/// Returns the task ID for tracking.
pub fn request_image_preview<'a>(
    mut commands: Commands,
    mut task_manager: ResMut<PreviewTaskManager>,
    cache: Res<PreviewCache>,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut images_mut: ResMut<Assets<Image>>,
    mut preview_ready_events: EventWriter<PreviewReady>,
    path: impl Into<AssetPath<'a>>,
) -> u64 {
    let path: AssetPath<'static> = path.into().into_owned();

    // Check cache first
    if let Some(cache_entry) = cache.get_by_path(&path) {
        // Cache hit - send ready event immediately
        let task_id = task_manager.create_task_id();
        preview_ready_events.write(PreviewReady {
            task_id,
            path: path.clone(),
            image_handle: cache_entry.image_handle.clone(),
        });
        return task_id;
    }

    // Cache miss - create request
    let task_id = task_manager.create_task_id();
    let handle: Handle<Image> = asset_server.load(&path);

    // Check if image is already loaded
    if let Some(image) = images.get(&handle) {
        // Image is already loaded, compress if needed and cache
        let preview_image = if let Some(compressed) = resize_image_for_preview(image) {
            images_mut.add(compressed)
        } else {
            handle.clone()
        };

        // Cache will be handled in handle_image_preview_events when the event is processed

        preview_ready_events.write(PreviewReady {
            task_id,
            path: path.clone(),
            image_handle: preview_image,
        });
        return task_id;
    }

    // Image not loaded yet - create pending request
    let entity = commands
        .spawn(PendingPreviewRequest {
            task_id,
            path: path.clone(),
        })
        .id();
    task_manager.register_task(task_id, entity);
    task_id
}

/// System that handles image asset events for previews and caches ready previews.
pub fn handle_image_preview_events(
    mut commands: Commands,
    mut cache: ResMut<PreviewCache>,
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
    // Cache previews from ready events
    for event in ready_events.read() {
        // Cache the preview if not already cached
        if cache.get_by_path(&event.path).is_none() {
            let asset_id = event.image_handle.id();
            cache.insert(
                &event.path,
                asset_id,
                event.image_handle.clone(),
                time.elapsed().as_secs(),
            );
        }
    }
    for event in asset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                // Find requests waiting for this image
                for (entity, request) in requests.iter() {
                    let handle: Handle<Image> = asset_server.load(&request.path);
                    if handle.id() == *id {
                        if let Some(image) = images.get(&handle) {
                            // Compress if needed
                            let preview_image =
                                if let Some(compressed) = resize_image_for_preview(image) {
                                    images.add(compressed)
                                } else {
                                    handle.clone()
                                };

                            // Cache the preview
                            let preview_id = preview_image.id();
                            cache.insert(
                                &request.path,
                                preview_id,
                                preview_image.clone(),
                                time.elapsed().as_secs(),
                            );

                            // Send ready event
                            preview_ready_events.write(PreviewReady {
                                task_id: request.task_id,
                                path: request.path.clone(),
                                image_handle: preview_image,
                            });

                            // Cleanup
                            task_manager.remove_task(request.task_id);
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
            AssetEvent::Removed { id } => {
                // Find requests for removed image
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
