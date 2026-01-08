mod cache;
mod image;
mod model;
mod renderer;
mod task;
// mod entity_preview; // Temporarily disabled

use bevy::{mesh::Mesh, pbr::StandardMaterial, prelude::*, scene::Scene};

// Re-export public types and functions
pub use cache::{PreviewCache, PreviewCacheEntry};
pub use image::{
    generate_previews_for_resolutions, handle_image_preview_events, request_image_preview,
    resize_image_for_preview,
};
pub use model::{
    PreviewScene3D, WaitingForScreenshot, capture_preview_screenshot, handle_preview_screenshots,
    process_3d_preview_requests, wait_for_asset_load,
};
pub use renderer::*;
pub use task::{PendingPreviewRequest, PreviewFailed, PreviewReady, PreviewTaskManager};
// pub use entity_preview::*; // Temporarily disabled

/// Preview mode for 3D assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreviewMode {
    /// Static image preview (for folder views).
    Image,
    /// Interactive 3D preview window (like Unity Inspector).
    Entity,
}

/// Model format for 3D model files.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFormat {
    /// GLTF format (.gltf, .glb)
    Gltf,
    /// FBX format (.fbx)
    FBX,
    /// OBJ format (.obj)
    Obj,
    /// Other format (specified by string)
    Other(String),
}

/// Type of preview request.
#[derive(Debug, Clone)]
pub enum PreviewRequestType {
    /// 2D image preview.
    Image2D,
    /// Generic model file preview (GLTF, OBJ, FBX, etc. loaded as Scene).
    ModelFile {
        /// Scene asset handle.
        handle: Handle<Scene>,
        /// Model format.
        format: ModelFormat,
    },
    /// Material preview (on a sphere).
    Material {
        /// Material asset handle.
        handle: Handle<StandardMaterial>,
    },
    /// Mesh preview.
    Mesh {
        /// Mesh asset handle.
        handle: Handle<Mesh>,
    },
}

/// Configuration for preview generation.
#[derive(Resource, Debug, Clone)]
pub struct PreviewConfig {
    /// Resolutions to generate previews for (in pixels). Default: [64, 256]
    pub resolutions: Vec<u32>,
    /// Render layer for preview scene isolation. Default: 1
    pub render_layer: usize,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            resolutions: vec![64, 256],
            render_layer: 1,
        }
    }
}
