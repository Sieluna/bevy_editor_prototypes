use std::path::{Path, PathBuf};

use bevy::{
    asset::{AssetPath, AssetServer},
    image::Image,
    prelude::*,
};

use crate::{
    asset::{AssetLoader, LoadPriority},
    preview::{PreviewCache, resize_image_for_preview},
};

#[derive(Component, Deref)]
pub struct PreviewAsset(pub PathBuf);

/// Component to track pending preview load requests
#[derive(Component, Debug)]
pub struct PendingPreviewLoad {
    pub task_id: u64,
    pub asset_path: AssetPath<'static>,
}

const FILE_PLACEHOLDER: &'static str = "embedded://bevy_asset_browser/assets/file_icon.png";

/// Checks if a file path represents an image file based on its extension.
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "bmp"
                    | "gif"
                    | "ico"
                    | "pnm"
                    | "pam"
                    | "pbm"
                    | "pgm"
                    | "ppm"
                    | "tga"
                    | "webp"
                    | "tiff"
                    | "tif"
                    | "dds"
                    | "exr"
                    | "hdr"
                    | "ktx2"
                    | "basis"
                    | "qoi"
            )
        })
        .unwrap_or(false)
}

/// System that handles PreviewAsset components and initiates preview loading.
pub fn preview_handler(
    mut commands: Commands,
    mut requests_query: Query<
        (Entity, &PreviewAsset),
        (Without<PendingPreviewLoad>, Without<ImageNode>),
    >,
    asset_server: Res<AssetServer>,
    mut loader: ResMut<AssetLoader>,
    cache: Res<PreviewCache>,
) {
    for (entity, preview_asset) in &mut requests_query {
        let path = &preview_asset.0;

        // Check if it's an image file
        if !is_image_file(path) {
            // Not an image, use placeholder
            let placeholder = asset_server.load(FILE_PLACEHOLDER);
            commands.entity(entity).insert(ImageNode::new(placeholder));
            commands.entity(entity).remove::<PreviewAsset>();
            continue;
        }

        // Convert PathBuf to AssetPath
        let asset_path: AssetPath<'static> = path.clone().into();

        // Check cache first
        if let Some(cache_entry) = cache.get_by_path(&asset_path) {
            // Cache hit - use cached preview immediately
            commands
                .entity(entity)
                .insert(ImageNode::new(cache_entry.image_handle.clone()));
            commands.entity(entity).remove::<PreviewAsset>();
            continue;
        }

        // Cache miss - submit to AssetLoader with Preload priority
        // (can be upgraded to CurrentAccess based on viewport visibility later)
        let task_id = loader.submit(&asset_path, LoadPriority::Preload);

        // Mark as pending
        commands.entity(entity).insert(PendingPreviewLoad {
            task_id,
            asset_path: asset_path.clone(),
        });

        // Insert placeholder temporarily
        let placeholder = asset_server.load(FILE_PLACEHOLDER);
        commands.entity(entity).insert(ImageNode::new(placeholder));
    }
}

/// System that handles completed asset loads and updates previews.
pub fn handle_preview_load_completed(
    mut commands: Commands,
    mut cache: ResMut<PreviewCache>,
    mut images: ResMut<Assets<Image>>,
    mut load_completed_events: EventReader<crate::asset::AssetLoadCompleted>,
    pending_query: Query<(Entity, &PendingPreviewLoad)>,
    mut image_node_query: Query<&mut ImageNode>,
    time: Res<Time<Real>>,
) {
    for event in load_completed_events.read() {
        // Find entities waiting for this task
        for (entity, pending) in pending_query.iter() {
            if pending.task_id == event.task_id {
                // Check if image is loaded
                if let Some(image) = images.get(&event.handle) {
                    // Compress if needed
                    let preview_image = if let Some(compressed) = resize_image_for_preview(image) {
                        images.add(compressed)
                    } else {
                        event.handle.clone()
                    };

                    // Cache the preview
                    let preview_id = preview_image.id();
                    cache.insert(
                        &pending.asset_path,
                        preview_id,
                        preview_image.clone(),
                        time.elapsed().as_secs(),
                    );

                    // Update ImageNode
                    if let Ok(mut image_node) = image_node_query.get_mut(entity) {
                        image_node.image = preview_image;
                    }

                    // Cleanup
                    commands.entity(entity).remove::<PreviewAsset>();
                    commands.entity(entity).remove::<PendingPreviewLoad>();
                }
                break;
            }
        }
    }
}
