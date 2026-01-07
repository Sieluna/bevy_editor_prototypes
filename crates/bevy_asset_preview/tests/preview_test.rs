use std::fs;
use std::path::PathBuf;

use bevy::{
    asset::{AssetPath, AssetPlugin},
    image::{CompressedImageFormats, ImageLoader},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_asset_preview::{
    AssetLoadCompleted, AssetLoadFailed, AssetLoader, AssetPreviewPlugin, PreviewAsset,
    PreviewCache, is_image_file,
};
use tempfile::TempDir;

/// Helper function to create a test image with a specific color.
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

/// Helper function to save an image to disk, handling format-specific requirements.
fn save_test_image(
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

/// Test image file detection function.
#[test]
fn test_is_image_file() {
    assert!(is_image_file(PathBuf::from("test.png").as_path()));
    assert!(is_image_file(PathBuf::from("test.jpg").as_path()));
    assert!(is_image_file(PathBuf::from("test.jpeg").as_path()));
    assert!(is_image_file(PathBuf::from("test.bmp").as_path()));
    assert!(is_image_file(PathBuf::from("test.gif").as_path()));
    assert!(is_image_file(PathBuf::from("test.webp").as_path()));
    assert!(is_image_file(PathBuf::from("test.tga").as_path()));
    assert!(is_image_file(PathBuf::from("test.TGA").as_path())); // Case insensitive

    assert!(!is_image_file(PathBuf::from("test.txt").as_path()));
    assert!(!is_image_file(PathBuf::from("test.rs").as_path()));
    assert!(!is_image_file(PathBuf::from("test").as_path())); // No extension
}

/// Test complete preview workflow with image loading and compression.
#[test]
fn test_complete_preview_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    unsafe {
        std::env::set_var(
            "BEVY_ASSET_ROOT",
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert path to string"),
        );
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    // Create a large test image (512x512) that should be compressed
    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let handle = create_test_image(&mut images, 512, 512, [128, 128, 128, 255]);
        let image = images.get(&handle).unwrap();

        let path = assets_dir.join("test_large.png");
        save_test_image(image, &path).expect("Failed to save test image");
    }

    // Spawn entity with PreviewAsset
    let entity = app
        .world_mut()
        .spawn(PreviewAsset(PathBuf::from("test_large.png")))
        .id();

    // Run systems until preview is ready
    let mut max_iterations = 1000;
    let mut preview_ready = false;
    while !preview_ready && max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        let world = app.world();
        // Check if PreviewAsset and PendingPreviewLoad are removed (preview is ready)
        if !world.entity(entity).contains::<PreviewAsset>()
            && !world
                .entity(entity)
                .contains::<bevy_asset_preview::PendingPreviewLoad>()
        {
            preview_ready = true;
        }
    }

    assert!(preview_ready, "Preview should be ready after loading");

    // Check that ImageNode was updated with preview
    let world = app.world();
    assert!(
        world.entity(entity).contains::<ImageNode>(),
        "ImageNode should be present after preview is ready"
    );

    // Check that preview was cached
    let cache = app.world().resource::<PreviewCache>();
    let asset_path: AssetPath<'static> = "test_large.png".into();
    assert!(
        cache.get_by_path(&asset_path).is_some(),
        "Large image preview should be cached"
    );

    // Verify the cached preview is compressed (256x256 max)
    let cache_entry = cache.get_by_path(&asset_path).unwrap();
    let images = app.world().resource::<Assets<Image>>();
    if let Some(preview_image) = images.get(&cache_entry.image_handle) {
        assert!(
            preview_image.width() <= 256 && preview_image.height() <= 256,
            "Cached preview should be compressed to max 256x256, got {}x{}",
            preview_image.width(),
            preview_image.height()
        );
    }
}

/// Simulate the complete frontend workflow: DirectoryContent -> spawn_file_node -> PreviewAsset -> preview loading
#[test]
fn test_frontend_workflow_simulation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    unsafe {
        std::env::set_var(
            "BEVY_ASSET_ROOT",
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert path to string"),
        );
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    // Step 1: Create test files (simulating files in a directory)
    let test_files = vec![
        ("icon.png", [255, 0, 0, 255], true),    // Image file
        ("sprite.png", [0, 255, 0, 255], true),  // Image file
        ("readme.txt", [0, 0, 0, 0], false),     // Non-image file
        ("texture.png", [0, 0, 255, 255], true), // Image file
    ];

    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        for (filename, color, is_image) in &test_files {
            if *is_image {
                let handle = create_test_image(&mut images, 128, 128, *color);
                let image = images.get(&handle).unwrap();

                let path = assets_dir.join(filename);
                save_test_image(image, &path).expect("Failed to save test image");
            } else {
                // Create a text file
                let path = assets_dir.join(filename);
                fs::write(&path, "Test file content").expect("Failed to write test file");
            }
        }
    }

    // Step 2: Simulate DirectoryContent (like refresh_ui system would do)
    // This simulates what happens when DirectoryContent changes and populate_directory_content is called
    let mut file_entities = Vec::new();
    let location_path = PathBuf::from(""); // Root directory

    for (filename, _, _) in &test_files {
        // Simulate spawn_file_node creating PreviewAsset
        let file_entity = app
            .world_mut()
            .spawn(PreviewAsset(location_path.join(filename)))
            .id();
        file_entities.push((
            file_entity,
            filename,
            is_image_file(&PathBuf::from(filename)),
        ));
    }

    // Step 3: Run preview_handler (simulates what happens in Update schedule)
    app.update();

    // Step 4: Verify initial state
    let world = app.world();
    for (entity, filename, is_image) in &file_entities {
        assert!(
            world.entity(*entity).contains::<ImageNode>(),
            "ImageNode should be added for file: {}",
            filename
        );

        if *is_image {
            // Image files should have PendingPreviewLoad (submitted to loader)
            assert!(
                world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>(),
                "Image file {} should have PendingPreviewLoad",
                filename
            );
        } else {
            // Non-image files should have PreviewAsset removed (placeholder used)
            assert!(
                !world.entity(*entity).contains::<PreviewAsset>(),
                "Non-image file {} should have PreviewAsset removed",
                filename
            );
        }
    }

    // Step 5: Wait for all image previews to load (simulate async loading)
    let image_entities: Vec<_> = file_entities
        .iter()
        .filter(|(_, _, is_image)| *is_image)
        .map(|(entity, filename, _)| (*entity, *filename))
        .collect();

    let mut max_iterations = 3000;
    while max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        // Check if all image previews are ready (no PreviewAsset and no PendingPreviewLoad)
        let world = app.world();
        let all_ready = image_entities.iter().all(|(entity, _)| {
            !world.entity(*entity).contains::<PreviewAsset>()
                && !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>()
        });

        if all_ready {
            break;
        }
    }

    // Verify all image previews are ready
    let world = app.world();
    for (entity, filename) in &image_entities {
        if world.entity(*entity).contains::<PreviewAsset>() {
            // Check if there are any load failed events
            let failed_events = app
                .world()
                .resource::<bevy::ecs::event::Events<AssetLoadFailed>>();
            let mut cursor = failed_events.get_cursor();
            let failures: Vec<_> = cursor.read(failed_events).collect();
            if !failures.is_empty() {
                panic!(
                    "Image {} failed to load. Failures: {:?}",
                    filename, failures
                );
            }
        }
        assert!(
            !world.entity(*entity).contains::<PreviewAsset>(),
            "PreviewAsset should be removed after loading for file: {}",
            filename
        );
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PendingPreviewLoad>(),
            "PendingPreviewLoad should be removed after loading for file: {}",
            filename
        );
    }

    // Step 6: Verify final state - all previews should be ready
    let world = app.world();
    for (entity, filename, is_image) in &file_entities {
        if *is_image {
            // Image files: PreviewAsset and PendingPreviewLoad should be removed, ImageNode should have preview
            assert!(
                !world.entity(*entity).contains::<PreviewAsset>(),
                "Image file {} should have PreviewAsset removed after loading",
                filename
            );
            assert!(
                !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>(),
                "Image file {} should have PendingPreviewLoad removed after loading",
                filename
            );
            assert!(
                world.entity(*entity).contains::<ImageNode>(),
                "Image file {} should still have ImageNode after loading",
                filename
            );
        }
    }

    // Step 7: Verify cache was populated
    let cache = app.world().resource::<PreviewCache>();
    for (_, filename, is_image) in &file_entities {
        if *is_image {
            let asset_path: AssetPath<'static> = filename.to_string().into();
            assert!(
                cache.get_by_path(&asset_path).is_some(),
                "Image file {} should be cached",
                filename
            );
        }
    }
}

/// Test batch preview loading with mixed priorities (simulating viewport scrolling)
#[test]
fn test_batch_preview_loading_with_priorities() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    unsafe {
        std::env::set_var(
            "BEVY_ASSET_ROOT",
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert path to string"),
        );
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    // Create many test images (simulating a folder with many files)
    let num_images: usize = 20;
    let mut test_files = Vec::new();

    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        for i in 0..num_images {
            let filename = format!("image_{:03}.png", i);
            let color: [u8; 4] = [
                ((i * 13) % 256) as u8,
                ((i * 17) % 256) as u8,
                ((i * 19) % 256) as u8,
                255,
            ];
            let handle = create_test_image(&mut images, 64, 64, color);
            let image = images.get(&handle).unwrap();

            let path = assets_dir.join(&filename);
            save_test_image(image, &path).expect("Failed to save test image");
            test_files.push(filename);
        }
    }

    // Simulate frontend: spawn all file nodes at once (like opening a folder)
    let mut file_entities = Vec::new();
    for filename in &test_files {
        let entity = app
            .world_mut()
            .spawn(PreviewAsset(PathBuf::from(filename)))
            .id();
        file_entities.push(entity);
    }

    // Run preview_handler - should submit all to loader
    app.update();

    // Verify all were submitted
    let loader = app.world().resource::<AssetLoader>();
    let initial_queue_size = loader.queue_len() + loader.active_tasks();
    assert!(
        initial_queue_size > 0,
        "Preview requests should be submitted to loader"
    );

    // Wait for all previews to load
    let mut max_iterations = 5000;
    let mut loaded_count = 0;
    let mut processed_task_ids = std::collections::HashSet::new();
    let mut load_order = Vec::new();

    while loaded_count < num_images && max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        let load_events = app
            .world()
            .resource::<bevy::ecs::event::Events<AssetLoadCompleted>>();
        let mut cursor = load_events.get_cursor();
        for event in cursor.read(load_events) {
            if processed_task_ids.insert(event.task_id) {
                loaded_count += 1;
                load_order.push((event.path.clone(), event.priority));
            }
        }
    }

    assert_eq!(
        loaded_count, num_images,
        "All images should be loaded. Loaded: {}, Expected: {}",
        loaded_count, num_images
    );

    // Verify all entities have previews ready
    let world = app.world();
    for entity in &file_entities {
        assert!(
            !world.entity(*entity).contains::<PreviewAsset>(),
            "PreviewAsset should be removed after loading"
        );
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PendingPreviewLoad>(),
            "PendingPreviewLoad should be removed after loading"
        );
        assert!(
            world.entity(*entity).contains::<ImageNode>(),
            "ImageNode should be present after loading"
        );
    }

    // Verify cache contains all previews
    let cache = app.world().resource::<PreviewCache>();
    assert_eq!(
        cache.len(),
        num_images,
        "Cache should contain all {} previews",
        num_images
    );
}

/// Test cache hit scenario (simulating re-opening a folder or scrolling back)
#[test]
fn test_cache_hit_scenario() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    unsafe {
        std::env::set_var(
            "BEVY_ASSET_ROOT",
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert path to string"),
        );
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    // Create test image
    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let handle = create_test_image(&mut images, 128, 128, [255, 128, 64, 255]);
        let image = images.get(&handle).unwrap();

        let path = assets_dir.join("cached_image.png");
        save_test_image(image, &path).expect("Failed to save test image");
    }

    // First request - should load and cache
    let _entity1 = app
        .world_mut()
        .spawn(PreviewAsset(PathBuf::from("cached_image.png")))
        .id();

    // Wait for first load
    let mut max_iterations = 1000;
    let mut loaded = false;
    while !loaded && max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        let load_events = app
            .world()
            .resource::<bevy::ecs::event::Events<AssetLoadCompleted>>();
        let mut cursor = load_events.get_cursor();
        for _event in cursor.read(load_events) {
            loaded = true;
        }
    }

    // Verify cache
    let cache = app.world().resource::<PreviewCache>();
    let asset_path: AssetPath<'static> = "cached_image.png".into();
    assert!(
        cache.get_by_path(&asset_path).is_some(),
        "Preview should be cached after first load"
    );

    // Simulate re-opening folder or scrolling back - second request
    let entity2 = app
        .world_mut()
        .spawn(PreviewAsset(PathBuf::from("cached_image.png")))
        .id();

    // Run preview_handler - should use cache immediately
    app.update();

    // Verify cache hit: entity2 should immediately have ImageNode, no PendingPreviewLoad
    let world = app.world();
    assert!(
        !world.entity(entity2).contains::<PreviewAsset>(),
        "PreviewAsset should be removed immediately on cache hit"
    );
    assert!(
        !world
            .entity(entity2)
            .contains::<bevy_asset_preview::PendingPreviewLoad>(),
        "PendingPreviewLoad should not be added on cache hit"
    );
    assert!(
        world.entity(entity2).contains::<ImageNode>(),
        "ImageNode should be added immediately on cache hit"
    );

    // Verify loader queue didn't grow (cache hit means no new load request)
    // Note: queue might have other tasks, but we verify cache was used by checking entity state
}

/// Test mixed file types in directory (images and non-images)
#[test]
fn test_mixed_file_types_in_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    unsafe {
        std::env::set_var(
            "BEVY_ASSET_ROOT",
            temp_dir
                .path()
                .to_str()
                .expect("Failed to convert path to string"),
        );
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE));

    // Create mixed files
    let mixed_files = vec![
        ("sprite.png", true),
        ("script.rs", false),
        ("texture.png", true),
        ("readme.md", false),
        ("icon.png", true),
        ("config.toml", false),
    ];

    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        for (filename, is_image) in &mixed_files {
            if *is_image {
                let handle = create_test_image(&mut images, 64, 64, [128, 128, 128, 255]);
                let image = images.get(&handle).unwrap();

                let path = assets_dir.join(filename);
                save_test_image(image, &path).expect("Failed to save test image");
            } else {
                let path = assets_dir.join(filename);
                fs::write(&path, format!("Content of {}", filename))
                    .expect("Failed to write test file");
            }
        }
    }

    // Simulate DirectoryContent with mixed files
    let mut file_entities = Vec::new();
    for (filename, is_image) in &mixed_files {
        let entity = app
            .world_mut()
            .spawn(PreviewAsset(PathBuf::from(filename)))
            .id();
        file_entities.push((entity, filename, *is_image));
    }

    // Run preview_handler
    app.update();

    // Verify: images should have PendingPreviewLoad, non-images should have placeholder
    let world = app.world();
    for (entity, filename, is_image) in &file_entities {
        assert!(
            world.entity(*entity).contains::<ImageNode>(),
            "All files should have ImageNode: {}",
            filename
        );

        if *is_image {
            assert!(
                world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>(),
                "Image file {} should have PendingPreviewLoad",
                filename
            );
        } else {
            assert!(
                !world.entity(*entity).contains::<PreviewAsset>(),
                "Non-image file {} should have PreviewAsset removed (placeholder used)",
                filename
            );
            assert!(
                !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>(),
                "Non-image file {} should not have PendingPreviewLoad",
                filename
            );
        }
    }

    // Wait for image previews to load
    let image_entities: Vec<_> = file_entities
        .iter()
        .filter(|(_, _, is_image)| *is_image)
        .map(|(entity, filename, _)| (*entity, *filename))
        .collect();

    let mut max_iterations = 3000;
    while max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        // Check if all image previews are ready
        let world = app.world();
        let all_ready = image_entities.iter().all(|(entity, _)| {
            !world.entity(*entity).contains::<PreviewAsset>()
                && !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>()
        });

        if all_ready {
            break;
        }
    }

    // Check for any load failures
    let failed_events = app
        .world()
        .resource::<bevy::ecs::event::Events<AssetLoadFailed>>();
    let mut cursor = failed_events.get_cursor();
    let failures: Vec<_> = cursor.read(failed_events).collect();
    if !failures.is_empty() {
        panic!("Some images failed to load: {:?}", failures);
    }

    // Final verification
    let world = app.world();
    for (entity, filename, is_image) in &file_entities {
        if *is_image {
            assert!(
                !world.entity(*entity).contains::<PreviewAsset>(),
                "Image file {} should have PreviewAsset removed after loading",
                filename
            );
            assert!(
                !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PendingPreviewLoad>(),
                "Image file {} should have PendingPreviewLoad removed after loading",
                filename
            );
        }
    }
}
