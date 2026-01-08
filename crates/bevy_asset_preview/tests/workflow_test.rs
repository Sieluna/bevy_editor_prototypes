use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;

use bevy::{
    asset::{AssetPath, AssetPlugin, io::file::FileAssetWriter},
    image::{CompressedImageFormats, ImageLoader},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_asset_preview::{
    ActiveSaveTask, AssetHotReloaded, AssetLoadCompleted, AssetLoadFailed, AssetLoader,
    PreviewCache, PreviewConfig, SaveCompleted, SaveTaskTracker, monitor_save_completion,
    save_image,
};
use tempfile::TempDir;
use gltf::json::{
    Accessor, Buffer, Mesh, Node, Root, Scene, Value,
    accessor::{ComponentType, GenericComponentType, Type},
    buffer::{Target, View},
    mesh::{Mode, Primitive, Semantic},
    serialize,
    validation::{Checked::Valid, USize64},
};

// ========== Helper functions ==========

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
            let rgb_image = dynamic_image.into_rgb8();
            rgb_image.save(path)?;
        }
        _ => {
            let rgba_image = dynamic_image.into_rgba8();
            rgba_image.save(path)?;
        }
    }
    Ok(())
}

/// Save a simple cube model as glb file
fn save_model(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // Simple cube vertices (position only)
    let positions: Vec<[f32; 3]> = vec![
        // Front face
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
        // Back face
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, -0.5, -0.5],
        // Top face
        [-0.5, 0.5, -0.5],
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
        [0.5, 0.5, -0.5],
        // Bottom face
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, -0.5, 0.5],
        [-0.5, -0.5, 0.5],
        // Right face
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, 0.5, 0.5],
        [0.5, -0.5, 0.5],
        // Left face
        [-0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [-0.5, 0.5, -0.5],
    ];

    // Indices for triangles
    let indices: Vec<u16> = vec![
        0, 1, 2, 0, 2, 3, // Front
        4, 5, 6, 4, 6, 7, // Back
        8, 9, 10, 8, 10, 11, // Top
        12, 13, 14, 12, 14, 15, // Bottom
        16, 17, 18, 16, 18, 19, // Right
        20, 21, 22, 20, 22, 23, // Left
    ];

    let mut root = Root::default();

    // Create buffer with positions and indices
    let positions_bytes: &[u8] = bytemuck::cast_slice(&positions);
    let indices_bytes: &[u8] = bytemuck::cast_slice(&indices);
    let mut buffer_data = Vec::from(positions_bytes);
    buffer_data.extend_from_slice(indices_bytes);

    // Pad buffer to multiple of 4
    while buffer_data.len() % 4 != 0 {
        buffer_data.push(0);
    }

    let buffer = root.push(Buffer {
        byte_length: USize64::from(buffer_data.len()),
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        uri: None, // glb doesn't use uri
    });

    // Buffer view for positions
    let positions_view = root.push(View {
        buffer,
        byte_length: USize64::from(positions_bytes.len()),
        byte_offset: Some(USize64(0)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(Target::ArrayBuffer)),
    });

    // Buffer view for indices
    let indices_view = root.push(View {
        buffer,
        byte_length: USize64::from(indices_bytes.len()),
        byte_offset: Some(USize64::from(positions_bytes.len())),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(Target::ElementArrayBuffer)),
    });

    // Accessor for positions
    let positions_accessor = root.push(Accessor {
        buffer_view: Some(positions_view),
        byte_offset: None,
        count: USize64::from(positions.len()),
        component_type: Valid(GenericComponentType(ComponentType::F32)),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(Type::Vec3),
        min: Some(Value::from(vec![-0.5, -0.5, -0.5])),
        max: Some(Value::from(vec![0.5, 0.5, 0.5])),
        name: None,
        normalized: false,
        sparse: None,
    });

    // Accessor for indices
    let indices_accessor = root.push(Accessor {
        buffer_view: Some(indices_view),
        byte_offset: None,
        count: USize64::from(indices.len()),
        component_type: Valid(GenericComponentType(ComponentType::U16)),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(Type::Scalar),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    // Create mesh primitive
    let primitive = Primitive {
        attributes: {
            let mut map = std::collections::BTreeMap::new();
            map.insert(Valid(Semantic::Positions), positions_accessor);
            map
        },
        extensions: Default::default(),
        extras: Default::default(),
        indices: Some(indices_accessor),
        material: None,
        mode: Valid(Mode::Triangles),
        targets: None,
    };

    let mesh = root.push(Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        primitives: vec![primitive],
        weights: None,
    });

    let node = root.push(Node {
        mesh: Some(mesh),
        ..Default::default()
    });

    root.scenes = vec![Scene {
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        nodes: vec![node],
    }];

    // Save as glb (binary glTF)
    let json_string = serialize::to_string(&root)?;
    let mut json_offset = json_string.len();
    while json_offset % 4 != 0 {
        json_offset += 1;
    }

    let glb = gltf::binary::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            length: (json_offset + buffer_data.len())
                .try_into()
                .map_err(|_| "File size exceeds binary glTF limit")?,
        },
        bin: Some(Cow::Owned(buffer_data)),
        json: Cow::Owned({
            let mut json_bytes = json_string.into_bytes();
            while json_bytes.len() % 4 != 0 {
                json_bytes.push(0x20); // pad with space
            }
            json_bytes
        }),
    };

    let writer = std::fs::File::create(path)?;
    glb.to_writer(writer)?;

    Ok(())
}

fn wait_for_save_completion(
    app: &mut App,
    expected_count: usize,
    max_iterations: usize,
) -> Vec<SaveCompleted> {
    let mut save_completed_count = 0;
    let mut processed_task_ids = std::collections::HashSet::new();
    let mut completed_events = Vec::new();
    let mut iterations = 0;

    while save_completed_count < expected_count && iterations < max_iterations {
        app.update();
        iterations += 1;

        let world = app.world();
        let save_events = world.resource::<Events<SaveCompleted>>();
        let mut cursor = save_events.get_cursor();
        for event in cursor.read(save_events) {
            if processed_task_ids.insert(event.task_id) {
                match &event.result {
                    Ok(_) => {
                        save_completed_count += 1;
                        completed_events.push(event.clone());
                    }
                    Err(e) => panic!("Save task {} failed: {}", event.task_id, e),
                }
            }
        }
    }

    assert_eq!(
        save_completed_count, expected_count,
        "All save tasks should complete"
    );

    completed_events
}

fn wait_for_load_completion(
    app: &mut App,
    expected_count: usize,
    max_iterations: usize,
) -> (Vec<AssetLoadCompleted>, usize, usize) {
    let mut loaded_count = 0;
    let mut processed_task_ids = std::collections::HashSet::new();
    let mut completed_events = Vec::new();
    let mut max_active_tasks = 0;
    let mut initial_queue_len = 0;
    let mut queue_len_checked = false;
    let mut iterations = 0;

    while loaded_count < expected_count && iterations < max_iterations {
        app.update();
        iterations += 1;

        let world = app.world();
        let loader = world.resource::<AssetLoader>();
        let active_tasks = loader.active_tasks();
        let queue_len = loader.queue_len();

        if !queue_len_checked && queue_len > 0 {
            initial_queue_len = queue_len;
            queue_len_checked = true;
        }

        if active_tasks > max_active_tasks {
            max_active_tasks = active_tasks;
        }

        assert!(
            active_tasks <= 4,
            "Active tasks should not exceed max_concurrent (4), got {}",
            active_tasks
        );

        let load_events = world.resource::<bevy::ecs::event::Events<AssetLoadCompleted>>();
        let mut cursor = load_events.get_cursor();
        for event in cursor.read(load_events) {
            if processed_task_ids.insert(event.task_id) {
                loaded_count += 1;
                completed_events.push(event.clone());
            }
        }
    }

    (completed_events, max_active_tasks, initial_queue_len)
}

struct CleanupState {
    pending_cleaned: bool,
    active_task_cleaned: bool,
    task_path_cleaned: bool,
    iterations: usize,
}

fn test_error_handling_for_nonexistent_file(app: &mut App) {
    let non_existent_entity = app
        .world_mut()
        .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from(
            "non_existent.png",
        )))
        .id();
    app.update();

    let world = app.world();
    let pending_component = world
        .entity(non_existent_entity)
        .get::<bevy_asset_preview::PendingPreviewLoad>()
        .expect("Non-existent file should have PendingPreviewLoad");
    let non_existent_task_id = pending_component.task_id;

    let loader = world.resource::<bevy_asset_preview::AssetLoader>();
    assert!(
        loader.get_task_path(non_existent_task_id).is_some(),
        "Task should be in queue after submission"
    );

    let cleanup_state = wait_for_failure_cleanup(app, non_existent_entity, non_existent_task_id);
    verify_failure_cleanup_complete(
        app,
        non_existent_entity,
        non_existent_task_id,
        &cleanup_state,
    );
}

fn wait_for_failure_cleanup(app: &mut App, entity: Entity, task_id: u64) -> CleanupState {
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 2000;

    let mut pending_cleaned = false;
    let mut active_task_cleaned = false;
    let mut task_path_cleaned = false;

    let mut active_task_query = app
        .world_mut()
        .query::<&bevy_asset_preview::ActiveLoadTask>();

    while iterations < MAX_ITERATIONS {
        app.update();
        iterations += 1;
        let world = app.world();

        pending_cleaned = !world
            .entity(entity)
            .contains::<bevy_asset_preview::PendingPreviewLoad>();

        let has_active_task_now = active_task_query
            .iter(world)
            .any(|active_task| active_task.task_id == task_id);
        active_task_cleaned = !has_active_task_now;

        let loader = world.resource::<bevy_asset_preview::AssetLoader>();
        task_path_cleaned = loader.get_task_path(task_id).is_none();

        if pending_cleaned && active_task_cleaned && task_path_cleaned {
            break;
        }
    }

    CleanupState {
        pending_cleaned,
        active_task_cleaned,
        task_path_cleaned,
        iterations,
    }
}

fn verify_failure_cleanup_complete(
    app: &mut App,
    entity: Entity,
    task_id: u64,
    state: &CleanupState,
) {
    let mut active_task_query = app
        .world_mut()
        .query::<&bevy_asset_preview::ActiveLoadTask>();
    let world = app.world();

    assert!(
        !world
            .entity(entity)
            .contains::<bevy_asset_preview::PendingPreviewLoad>(),
        "PendingPreviewLoad MUST be cleaned up (iterations: {})",
        state.iterations
    );

    let has_active_task = active_task_query
        .iter(world)
        .any(|active_task| active_task.task_id == task_id);
    assert!(
        !has_active_task,
        "ActiveLoadTask MUST be cleaned up (iterations: {})",
        state.iterations
    );

    let loader = world.resource::<bevy_asset_preview::AssetLoader>();
    assert!(
        loader.get_task_path(task_id).is_none(),
        "Task path MUST be removed from loader (iterations: {})",
        state.iterations
    );

    assert!(
        !world
            .entity(entity)
            .contains::<bevy_asset_preview::PreviewAsset>(),
        "PreviewAsset must be cleaned up after failure"
    );

    assert!(
        world.entity(entity).contains::<ImageNode>(),
        "ImageNode (placeholder) should remain after failure cleanup"
    );
}

fn test_boundary_conditions(app: &mut App) {
    let empty_path_entity = app
        .world_mut()
        .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from("")))
        .id();
    app.update();

    let world = app.world();
    assert!(
        world.entity(empty_path_entity).contains::<ImageNode>(),
        "Empty path should have ImageNode (placeholder)"
    );
    assert!(
        !world
            .entity(empty_path_entity)
            .contains::<bevy_asset_preview::PendingPreviewLoad>(),
        "Empty path should not have PendingPreviewLoad"
    );

    let special_char_entity = app
        .world_mut()
        .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from(
            "test_file_123.png",
        )))
        .id();
    app.update();

    let world = app.world();
    assert!(
        world.entity(special_char_entity).contains::<ImageNode>(),
        "Special char path should have ImageNode"
    );
}

#[test]
fn test_complete_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let assets_dir = temp_dir.path().join("assets");
    let cache_dir = temp_dir.path().join("cache");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");
    fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");

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

    // Register cache directory as asset source
    let cache_dir_path = temp_dir.path().join("cache").join("asset_preview");
    app.register_asset_source(
        "thumbnail_cache",
        bevy::asset::io::AssetSourceBuilder::platform_default(
            cache_dir_path
                .to_str()
                .expect("Cache dir path should be valid"),
            None,
        ),
    );

    // Initialize complete plugin system
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .add_plugins(bevy_asset_preview::AssetPreviewPlugin)
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE))
        .add_event::<AssetLoadCompleted>()
        .add_event::<AssetLoadFailed>()
        .add_event::<AssetHotReloaded>()
        .add_event::<SaveCompleted>()
        .init_resource::<SaveTaskTracker>()
        .add_systems(Update, monitor_save_completion);

    // Initialize Time resource by running one update
    app.update();

    // ========== Phase 1: Create files and save previews to cache ==========
    let file_definitions = vec![
        ("icon.png", true, 64, 64, [255, 0, 0, 255]), // Small image, no compression
        ("texture.png", true, 512, 512, [0, 255, 0, 255]), // Large image, needs compression
        ("sprite.png", true, 800, 200, [0, 0, 255, 255]), // Wide image, needs compression
        ("readme.md", false, 0, 0, [0, 0, 0, 0]),     // Non-image file
        ("script.rs", false, 0, 0, [0, 0, 0, 0]),     // Non-image file
    ];

    let mut image_files = Vec::new();
    {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        for (filename, is_image, width, height, color) in &file_definitions {
            if *is_image {
                let handle = create_test_image(&mut images, *width, *height, *color);
                let image = images.get(&handle).unwrap();
                let path = assets_dir.join(filename);
                save_test_image(image, &path).expect("Failed to save test image");
                image_files.push((filename.to_string(), handle));
            } else {
                let path = assets_dir.join(filename);
                fs::write(&path, format!("Content of {}", filename))
                    .expect("Failed to write test file");
            }
        }
    }

    let save_tasks = app
        .world_mut()
        .resource_scope(|world, mut tracker: Mut<SaveTaskTracker>| {
            let images = world.get_resource::<Assets<Image>>().unwrap();
            let mut tasks = Vec::new();

            for (filename, handle) in &image_files {
                let writer = FileAssetWriter::new("", true);
                let target_path =
                    AssetPath::from_path_buf(PathBuf::from("cache/asset_preview").join(filename))
                        .into_owned();
                let task = save_image(handle.clone(), target_path.clone(), images, writer);
                let task_id = tracker.create_task_id();
                let path_asset: AssetPath<'static> =
                    AssetPath::from_path_buf(PathBuf::from(filename)).into_owned();
                tracker.register_pending(task_id, path_asset.clone());
                tasks.push((task_id, path_asset, target_path, task));
            }

            tasks
        });

    let mut commands = app.world_mut().commands();
    for (task_id, path, target_path, task) in save_tasks {
        commands.spawn(ActiveSaveTask {
            task_id,
            path,
            target_path,
            task,
        });
    }

    wait_for_save_completion(&mut app, image_files.len(), 1000);

    std::thread::sleep(std::time::Duration::from_millis(100));
    for (filename, _) in &image_files {
        let mut cached_path = cache_dir_path.join(filename);
        cached_path.set_extension("webp");
        assert!(
            cached_path.exists(),
            "Cached preview file should exist: {:?}",
            cached_path
        );
    }

    // Phase 2: Preview request processing and initial state validation
    let mut file_entities = Vec::new();
    for (filename, is_image, _, _, _) in &file_definitions {
        let entity = app
            .world_mut()
            .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from(filename)))
            .id();
        file_entities.push((entity, filename.to_string(), *is_image));
    }

    app.update();

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
                !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PreviewAsset>(),
                "Non-image file {} should have PreviewAsset removed",
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

    // Phase 3: Wait for load completion and validate
    let image_entities: Vec<_> = file_entities
        .iter()
        .filter(|(_, _, is_image)| *is_image)
        .map(|(entity, filename, _)| (*entity, filename.clone()))
        .collect();

    let (all_completed_events, max_active_tasks_observed, initial_queue_len) =
        wait_for_load_completion(&mut app, image_entities.len(), 3000);

    assert!(
        max_active_tasks_observed > 0,
        "Should have observed active tasks during loading"
    );
    assert!(
        max_active_tasks_observed <= 4,
        "Max active tasks should not exceed max_concurrent, got {}",
        max_active_tasks_observed
    );

    if image_entities.len() > 4 {
        assert!(
            initial_queue_len > 0,
            "Should have tasks in queue when loading more than max_concurrent tasks, got {}",
            initial_queue_len
        );
    }

    assert!(
        all_completed_events.len() >= image_files.len(),
        "Should have at least {} load completed events, got {}",
        image_files.len(),
        all_completed_events.len()
    );

    let world = app.world();
    let cache = world.resource::<PreviewCache>();
    let config = world.resource::<PreviewConfig>();
    let images = world.resource::<Assets<Image>>();

    assert!(!cache.is_empty(), "Cache should not be empty");

    // Each image should have previews for all configured resolutions
    let expected_cache_entries = image_files.len() * config.resolutions.len();
    assert_eq!(
        cache.len(),
        expected_cache_entries,
        "Cache should contain exactly {} entries ({} images × {} resolutions)",
        expected_cache_entries,
        image_files.len(),
        config.resolutions.len()
    );

    for (entity, filename, is_image) in &file_entities {
        if *is_image {
            assert!(
                !world
                    .entity(*entity)
                    .contains::<bevy_asset_preview::PreviewAsset>(),
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

            let asset_path: AssetPath<'static> = AssetPath::from(filename.as_str()).into_owned();

            // Check that all configured resolutions are cached
            for &resolution in &config.resolutions {
                let cache_entry_by_path = cache.get_by_path(&asset_path, Some(resolution));
                assert!(
                    cache_entry_by_path.is_some(),
                    "Image file {} should have {}px resolution cached",
                    filename,
                    resolution
                );

                let entry = cache_entry_by_path.unwrap();
                assert_eq!(
                    entry.resolution, resolution,
                    "Cached entry should have correct resolution {} for {}",
                    resolution, filename
                );

                let cache_entry_by_id = cache.get_by_id(entry.asset_id, Some(resolution));
                assert!(
                    cache_entry_by_id.is_some(),
                    "Cache entry should be accessible by ID for {} at {}px",
                    filename,
                    resolution
                );
                assert_eq!(
                    entry.image_handle.id(),
                    cache_entry_by_id.unwrap().image_handle.id(),
                    "Cache entries by path and ID should match for {} at {}px",
                    filename,
                    resolution
                );
            }

            // Check that highest resolution query works
            let highest_entry = cache.get_by_path(&asset_path, None);
            assert!(
                highest_entry.is_some(),
                "Image file {} should have highest resolution cached",
                filename
            );
            let highest_entry = highest_entry.unwrap();
            let highest_resolution = highest_entry.resolution;
            let expected_highest = *config.resolutions.iter().max().unwrap();
            assert_eq!(
                highest_resolution, expected_highest,
                "Highest resolution should be {} for {}",
                expected_highest, filename
            );

            // Validate highest resolution entry properties
            assert!(
                !highest_entry.timestamp.is_zero(),
                "Cache entry for {} should have valid timestamp, got: {:?}",
                filename,
                highest_entry.timestamp
            );
            assert!(
                highest_entry.image_handle.is_strong(),
                "Cache entry for {} should have strong handle",
                filename
            );

            // Check compression for large images (using highest resolution)
            if filename == "texture.png" || filename == "sprite.png" {
                if let Some(preview_image) = images.get(&highest_entry.image_handle) {
                    let max_dimension = expected_highest;
                    assert!(
                        preview_image.width() <= max_dimension
                            && preview_image.height() <= max_dimension,
                        "Large image {} should be compressed, got {}x{}",
                        filename,
                        preview_image.width(),
                        preview_image.height()
                    );
                }
            }

            // Check aspect ratio for wide image (using highest resolution)
            if filename == "sprite.png" {
                if let Some(preview_image) = images.get(&highest_entry.image_handle) {
                    // Original: 800x200 = 4:1, compressed should maintain aspect ratio
                    let expected_height = (200.0 * expected_highest as f32 / 800.0) as u32;
                    assert_eq!(
                        preview_image.width(),
                        expected_highest,
                        "Wide image width should be {}",
                        expected_highest
                    );
                    assert_eq!(
                        preview_image.height(),
                        expected_height,
                        "Wide image should maintain aspect ratio, expected height: {}, got: {}",
                        expected_height,
                        preview_image.height()
                    );
                }
            }
        }
    }

    let loader = app.world().resource::<AssetLoader>();
    assert_eq!(
        loader.queue_len(),
        0,
        "Loader queue should be empty after all tasks complete"
    );
    assert_eq!(
        loader.active_tasks(),
        0,
        "Should have 0 active tasks after cleanup"
    );

    let mut query = app.world_mut().query::<&ActiveSaveTask>();
    let world = app.world();
    let save_task_count = query.iter(world).count();
    assert_eq!(
        save_task_count, 0,
        "All save task entities should be cleaned up, found {} remaining",
        save_task_count
    );

    // Phase 4: Cache hit test
    let mut second_batch_entities = Vec::new();
    for (filename, is_image, _, _, _) in &file_definitions {
        if *is_image {
            let entity = app
                .world_mut()
                .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from(filename)))
                .id();
            second_batch_entities.push((entity, filename.to_string()));
        }
    }

    let concurrent_entities: Vec<_> = (0..3)
        .map(|_| {
            app.world_mut()
                .spawn(bevy_asset_preview::PreviewAsset(PathBuf::from("icon.png")))
                .id()
        })
        .collect();

    app.update();

    let world = app.world();
    for (entity, filename) in &second_batch_entities {
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PreviewAsset>(),
            "PreviewAsset should be removed immediately on cache hit for {}",
            filename
        );
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PendingPreviewLoad>(),
            "PendingPreviewLoad should not be added on cache hit for {}",
            filename
        );
        assert!(
            world.entity(*entity).contains::<ImageNode>(),
            "ImageNode should be added immediately on cache hit for {}",
            filename
        );
    }

    for entity in &concurrent_entities {
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PreviewAsset>(),
            "Concurrent requests should use cache immediately"
        );
        assert!(
            !world
                .entity(*entity)
                .contains::<bevy_asset_preview::PendingPreviewLoad>(),
            "Concurrent requests should not have PendingPreviewLoad"
        );
        assert!(
            world.entity(*entity).contains::<ImageNode>(),
            "Concurrent requests should have ImageNode"
        );
    }

    // Phase 5: Error handling and boundary conditions
    test_error_handling_for_nonexistent_file(&mut app);
    test_boundary_conditions(&mut app);

    // Phase 6: Final integrity validation
    let mut active_task_query = app
        .world_mut()
        .query::<&bevy_asset_preview::ActiveLoadTask>();
    let world = app.world();
    let cache = world.resource::<PreviewCache>();
    let config = world.resource::<bevy_asset_preview::PreviewConfig>();
    let loader = world.resource::<AssetLoader>();

    assert!(!cache.is_empty(), "Cache should not be empty");
    // Each image should have previews for all configured resolutions
    let expected_cache_entries = image_files.len() * config.resolutions.len();
    assert_eq!(
        cache.len(),
        expected_cache_entries,
        "Cache should contain all {} image previews ({} images × {} resolutions)",
        expected_cache_entries,
        image_files.len(),
        config.resolutions.len()
    );

    let non_existent_active_tasks = active_task_query
        .iter(world)
        .filter(|active_task| active_task.path.to_string().contains("non_existent"))
        .count();

    assert_eq!(
        non_existent_active_tasks, 0,
        "All non-existent file tasks must be cleaned up (found {} remaining)",
        non_existent_active_tasks
    );

    let actual_active_tasks = loader.active_tasks();
    let actual_queue_len = loader.queue_len();

    assert_eq!(
        actual_active_tasks, 0,
        "All active tasks must be cleaned up after failure handling (found {} remaining)",
        actual_active_tasks
    );

    assert!(
        actual_queue_len <= 1,
        "Queue should have at most 1 task, found {}",
        actual_queue_len
    );

    let mut image_entities_with_preview = 0;
    for (entity, filename, is_image) in &file_entities {
        if *is_image {
            assert!(
                world.entity(*entity).contains::<ImageNode>(),
                "Image file {} should have ImageNode",
                filename
            );
            image_entities_with_preview += 1;
        }
    }
    assert_eq!(
        image_entities_with_preview,
        image_files.len(),
        "All image entities should have ImageNode previews"
    );

    println!(
        "Test completed: created {} files, {} image previews, cached {} entries, {} load completed events",
        file_definitions.len(),
        image_files.len(),
        cache.len(),
        all_completed_events.len()
    );
}
