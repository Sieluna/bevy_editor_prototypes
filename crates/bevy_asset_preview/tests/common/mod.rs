use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};

/// Helper function to create a test image with a specific color.
pub fn create_test_image(
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

/// Helper function to save an image to disk, handling format-specific requirements.
pub fn save_test_image(
    image: &Image,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let dynamic_image = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| format!("Failed to convert image: {:?}", e))?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => {
            // JPEG doesn't support transparency, convert to RGB
            let rgb_image = dynamic_image.into_rgb8();
            rgb_image.save(path)?;
        }
        _ => {
            // PNG and other formats support RGBA
            let rgba_image = dynamic_image.into_rgba8();
            rgba_image.save(path)?;
        }
    }
    Ok(())
}
