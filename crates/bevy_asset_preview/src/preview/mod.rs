mod cache;
mod systems;

use bevy::{asset::RenderAssetUsages, image::Image};
pub use cache::{PreviewCache, PreviewCacheEntry};
pub use systems::{
    PendingPreviewRequest, PreviewFailed, PreviewReady, PreviewTaskManager,
    handle_image_preview_events, request_image_preview,
};

/// Maximum preview size for 2D images (256x256).
const MAX_PREVIEW_SIZE: u32 = 256;

/// Resizes an image to preview size if it's larger than the maximum.
/// Returns a new compressed image, or None if the image is already small enough.
pub fn compress_image_for_preview(image: &Image) -> Option<Image> {
    let width = image.width();
    let height = image.height();

    // If image is already small enough, return None (use original)
    if width <= MAX_PREVIEW_SIZE && height <= MAX_PREVIEW_SIZE {
        return None;
    }

    // Calculate new size maintaining aspect ratio
    let (new_width, new_height) = if width > height {
        (
            MAX_PREVIEW_SIZE,
            (height as f32 * MAX_PREVIEW_SIZE as f32 / width as f32) as u32,
        )
    } else {
        (
            (width as f32 * MAX_PREVIEW_SIZE as f32 / height as f32) as u32,
            MAX_PREVIEW_SIZE,
        )
    };

    // Convert to dynamic image for resizing
    let dynamic_image = match image.clone().try_into_dynamic() {
        Ok(img) => img,
        Err(_) => return None,
    };

    // Resize using high-quality filter
    let resized =
        dynamic_image.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);

    // Convert back to Image
    Some(Image::from_dynamic(
        resized,
        true, // is_srgb
        RenderAssetUsages::RENDER_WORLD,
    ))
}
