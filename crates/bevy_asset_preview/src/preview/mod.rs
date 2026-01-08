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
/// Returns a new resized image, or None if the image is already small enough.
pub fn resize_image_for_preview(image: &Image) -> Option<Image> {
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

#[cfg(test)]
mod tests {
    use std::fs;

    use bevy::{
        asset::AssetPlugin,
        prelude::*,
        render::{
            render_asset::RenderAssetUsages,
            render_resource::{Extent3d, TextureDimension, TextureFormat},
        },
    };
    use tempfile::TempDir;

    use super::*;

    fn create_test_image(
        images: &mut Assets<Image>,
        width: u32,
        height: u32,
        color: [u8; 4],
    ) -> Handle<Image> {
        let pixel_data: Vec<u8> = (0..(width * height))
            .flat_map(|_| color.iter().copied())
            .collect();

        let image = Image::new_fill(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &pixel_data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );

        images.add(image)
    }

    #[test]
    fn test_image_compression() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let assets_dir = temp_dir.path().join("assets");
        fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin {
                file_path: assets_dir.display().to_string(),
                ..Default::default()
            })
            .init_asset::<Image>();

        let mut images = app.world_mut().resource_mut::<Assets<Image>>();

        // Test small image (should not be compressed)
        let small_handle = create_test_image(&mut images, 64, 64, [128, 128, 128, 255]);
        let small_image = images.get(&small_handle).unwrap();
        let compressed_small = resize_image_for_preview(small_image);
        assert!(
            compressed_small.is_none(),
            "Small image should not be compressed"
        );

        // Test large image (should be compressed)
        let large_handle = create_test_image(&mut images, 512, 512, [128, 128, 128, 255]);
        let large_image = images.get(&large_handle).unwrap();
        let compressed_large = resize_image_for_preview(large_image);
        assert!(
            compressed_large.is_some(),
            "Large image should be compressed"
        );
        let compressed = compressed_large.unwrap();
        assert!(
            compressed.width() <= 256 && compressed.height() <= 256,
            "Compressed image should be <= 256x256, got {}x{}",
            compressed.width(),
            compressed.height()
        );

        // Test wide image (maintain aspect ratio)
        let wide_handle = create_test_image(&mut images, 800, 200, [128, 128, 128, 255]);
        let wide_image = images.get(&wide_handle).unwrap();
        let compressed_wide = resize_image_for_preview(wide_image);
        assert!(compressed_wide.is_some(), "Wide image should be compressed");
        let compressed = compressed_wide.unwrap();
        assert_eq!(compressed.width(), 256, "Wide image width should be 256");
        assert!(
            compressed.height() < 256,
            "Wide image height should be < 256"
        );
        // Verify aspect ratio: 800/200 = 4:1, after compression should be 256:64
        let expected_height = (200.0 * 256.0 / 800.0) as u32;
        assert_eq!(
            compressed.height(),
            expected_height,
            "Wide image should maintain aspect ratio"
        );

        // Test tall image (maintain aspect ratio)
        let tall_handle = create_test_image(&mut images, 200, 800, [128, 128, 128, 255]);
        let tall_image = images.get(&tall_handle).unwrap();
        let compressed_tall = resize_image_for_preview(tall_image);
        assert!(compressed_tall.is_some(), "Tall image should be compressed");
        let compressed = compressed_tall.unwrap();
        assert_eq!(compressed.height(), 256, "Tall image height should be 256");
        assert!(compressed.width() < 256, "Tall image width should be < 256");
        // Verify aspect ratio: 200/800 = 1:4, after compression should be 64:256
        let expected_width = (200.0 * 256.0 / 800.0) as u32;
        assert_eq!(
            compressed.width(),
            expected_width,
            "Tall image should maintain aspect ratio"
        );
    }
}
