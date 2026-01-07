use std::collections::HashMap;
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
    LoadPriority, SaveCompleted, SaveTaskTracker, handle_asset_events, monitor_save_completion,
    process_load_queue, save_image,
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

pub(crate) fn get_base_path() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("BEVY_ASSET_ROOT") {
        PathBuf::from(manifest_dir)
    } else if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(manifest_dir)
    } else {
        std::env::current_exe()
            .map(|path| path.parent().map(ToOwned::to_owned).unwrap())
            .unwrap()
    }
}

/// Integration test: Complete file system workflow with batch operations.
#[test]
fn test_complete_filesystem_workflow() {
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

    println!("BEVY_ASSET_ROOT: {}", get_base_path().display());

    let mut app = App::new();

    let cache_dir = temp_dir.path().join("cache").join("asset_preview");
    app.register_asset_source(
        "thumbnail_cache",
        bevy::asset::io::AssetSourceBuilder::platform_default(
            cache_dir.to_str().expect("Cache dir path should be valid"),
            None,
        ),
    );

    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin {
            file_path: assets_dir.display().to_string(),
            ..Default::default()
        })
        .init_asset::<Image>()
        .register_asset_loader(ImageLoader::new(CompressedImageFormats::NONE))
        .add_event::<AssetLoadCompleted>()
        .add_event::<AssetLoadFailed>()
        .add_event::<AssetHotReloaded>()
        .add_event::<SaveCompleted>()
        .init_resource::<AssetLoader>()
        .init_resource::<SaveTaskTracker>()
        .add_systems(
            Update,
            (
                process_load_queue,
                handle_asset_events,
                monitor_save_completion,
            ),
        );

    let test_images: Vec<(Handle<Image>, PathBuf, [u8; 4])> = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        vec![
            (
                create_test_image(&mut images, 64, 64, [255, 0, 0, 255]),
                PathBuf::from("test_red.png"),
                [255, 0, 0, 255],
            ),
            (
                create_test_image(&mut images, 64, 64, [0, 255, 0, 255]),
                PathBuf::from("test_green.png"),
                [0, 255, 0, 255],
            ),
            (
                create_test_image(&mut images, 64, 64, [0, 0, 255, 255]),
                PathBuf::from("test_blue.png"),
                [0, 0, 255, 255],
            ),
            (
                create_test_image(&mut images, 64, 64, [255, 255, 0, 255]),
                PathBuf::from("test_yellow.png"),
                [255, 255, 0, 255],
            ),
            (
                create_test_image(&mut images, 64, 64, [255, 0, 255, 255]),
                PathBuf::from("test_magenta.png"),
                [255, 0, 255, 255],
            ),
        ]
    };

    {
        let save_tasks =
            app.world_mut()
                .resource_scope(|world, mut tracker: Mut<SaveTaskTracker>| {
                    let images = world.get_resource::<Assets<Image>>().unwrap();

                    let mut tasks = Vec::new();

                    for (handle, path, _) in &test_images {
                        let writer = FileAssetWriter::new("", true);
                        let target_path = AssetPath::from_path_buf(
                            PathBuf::from("cache/asset_preview").join(path),
                        )
                        .into_owned();
                        let task = save_image(handle.clone(), target_path.clone(), images, writer);
                        let task_id = tracker.create_task_id();
                        let path_asset: AssetPath<'static> =
                            AssetPath::from_path_buf(path.clone()).into_owned();
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
    }

    let mut save_completed_count = 0;
    let mut save_failed_count = 0;
    let mut max_iterations = 1000;
    let mut processed_task_ids = std::collections::HashSet::new();

    while save_completed_count < test_images.len() && max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        let save_events = app
            .world()
            .resource::<bevy::ecs::event::Events<SaveCompleted>>();
        let mut cursor = save_events.get_cursor();
        for event in cursor.read(save_events) {
            if processed_task_ids.insert(event.task_id) {
                match &event.result {
                    Ok(_) => {
                        save_completed_count += 1;
                    }
                    Err(e) => {
                        save_failed_count += 1;
                        eprintln!(
                            "Save task {} failed for {:?}: {}",
                            event.task_id, event.path, e
                        );
                    }
                }
            }
        }
    }

    if save_failed_count > 0 {
        panic!(
            "{} save tasks failed. Completed: {}, Failed: {}",
            save_failed_count, save_completed_count, save_failed_count
        );
    }

    assert_eq!(
        save_completed_count,
        test_images.len(),
        "All images should be saved successfully. Completed: {}, Expected: {}",
        save_completed_count,
        test_images.len()
    );

    std::thread::sleep(std::time::Duration::from_millis(100));

    for (_, path, _) in &test_images {
        let mut saved_path = temp_dir
            .path()
            .join("cache")
            .join("asset_preview")
            .join(path);

        saved_path.set_extension("webp");

        let saved_path = saved_path
            .canonicalize()
            .unwrap_or_else(|_| saved_path.clone());

        assert!(
            saved_path.exists(),
            "Saved file should exist: {:?}",
            saved_path
        );
    }

    for (_, path, _) in &test_images {
        let mut src = temp_dir
            .path()
            .join("cache")
            .join("asset_preview")
            .join(path);
        src.set_extension("webp");

        let mut dst = assets_dir.join(path);
        dst.set_extension("webp");
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).expect("Failed to create assets subdirectory");
        }
        fs::copy(&src, &dst).expect("Failed to copy file for loading test");
    }

    for (_, path, _) in &test_images {
        let mut asset_file = assets_dir.join(path);
        asset_file.set_extension("webp");
        assert!(
            asset_file.exists(),
            "Asset file should exist before loading: {:?}",
            asset_file
        );
    }

    let asset_paths: Vec<(bevy::asset::AssetPath<'static>, LoadPriority)> = vec![
        ("test_red.webp".into(), LoadPriority::Preload),
        ("test_green.webp".into(), LoadPriority::HotReload),
        ("test_blue.webp".into(), LoadPriority::CurrentAccess),
        ("test_yellow.webp".into(), LoadPriority::CurrentAccess),
        ("test_magenta.webp".into(), LoadPriority::HotReload),
    ];

    let mut loader = app.world_mut().resource_mut::<AssetLoader>();
    let mut load_task_ids = HashMap::new();

    for (path, priority) in &asset_paths {
        let task_id = loader.submit(path.clone(), *priority);
        load_task_ids.insert(task_id, (path.clone(), *priority));
    }

    assert_eq!(loader.queue_len(), asset_paths.len());

    let mut load_completed_count = 0;
    let mut load_completed_paths = Vec::new();
    max_iterations = 1000;
    let mut processed_load_task_ids = std::collections::HashSet::new();

    while load_completed_count < asset_paths.len() && max_iterations > 0 {
        app.update();
        max_iterations -= 1;

        let load_events = app
            .world()
            .resource::<bevy::ecs::event::Events<AssetLoadCompleted>>();
        let mut cursor = load_events.get_cursor();
        for event in cursor.read(load_events) {
            if processed_load_task_ids.insert(event.task_id) {
                load_completed_count += 1;
                load_completed_paths.push((event.path.clone(), event.priority));

                if let Some((expected_path, expected_priority)) = load_task_ids.get(&event.task_id)
                {
                    assert_eq!(
                        &event.path, expected_path,
                        "Loaded path should match submitted path"
                    );
                    assert_eq!(
                        event.priority, *expected_priority,
                        "Loaded priority should match submitted priority"
                    );
                }
            }
        }

        let failed_events = app
            .world()
            .resource::<bevy::ecs::event::Events<AssetLoadFailed>>();
        let mut failed_cursor = failed_events.get_cursor();
        for event in failed_cursor.read(failed_events) {
            eprintln!(
                "Load failed: task_id={}, path={:?}, error={}",
                event.task_id, event.path, event.error
            );
        }

        if max_iterations % 100 == 0 {
            let loader = app.world().resource::<AssetLoader>();
            let active_tasks = loader.active_tasks();
            let queue_len = loader.queue_len();
            eprintln!(
                "Load progress: completed={}, active={}, queue={}, iterations={}",
                load_completed_count, active_tasks, queue_len, max_iterations
            );
        }
    }

    assert_eq!(
        load_completed_count,
        asset_paths.len(),
        "All images should be loaded successfully"
    );

    let current_access_indices: Vec<usize> = load_completed_paths
        .iter()
        .enumerate()
        .filter_map(|(i, (_, p))| {
            if *p == LoadPriority::CurrentAccess {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    let hot_reload_indices: Vec<usize> = load_completed_paths
        .iter()
        .enumerate()
        .filter_map(|(i, (_, p))| {
            if *p == LoadPriority::HotReload {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    let preload_indices: Vec<usize> = load_completed_paths
        .iter()
        .enumerate()
        .filter_map(|(i, (_, p))| {
            if *p == LoadPriority::Preload {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    if let (Some(&max_current), Some(&min_preload)) = (
        current_access_indices.iter().max(),
        preload_indices.iter().min(),
    ) {
        assert!(
            max_current < min_preload,
            "CurrentAccess tasks should complete before Preload tasks"
        );
    }

    if let (Some(&max_hot_reload), Some(&min_preload)) = (
        hot_reload_indices.iter().max(),
        preload_indices.iter().min(),
    ) {
        assert!(
            max_hot_reload < min_preload,
            "HotReload tasks should complete before Preload tasks"
        );
    }
}

/// Functional test: Test priority queue ordering.
#[test]
fn test_asset_loader_priority_queue() {
    let mut app = App::new();
    app.init_resource::<AssetLoader>();

    app.world_mut()
        .resource_mut::<AssetLoader>()
        .submit("preload1.png", LoadPriority::Preload);
    app.world_mut()
        .resource_mut::<AssetLoader>()
        .submit("current1.png", LoadPriority::CurrentAccess);
    app.world_mut()
        .resource_mut::<AssetLoader>()
        .submit("hotreload1.png", LoadPriority::HotReload);
    app.world_mut()
        .resource_mut::<AssetLoader>()
        .submit("preload2.png", LoadPriority::Preload);
    app.world_mut()
        .resource_mut::<AssetLoader>()
        .submit("current2.png", LoadPriority::CurrentAccess);

    let loader = app.world().resource::<AssetLoader>();
    assert_eq!(loader.queue_len(), 5);

    // Verify priority ordering: CurrentAccess > HotReload > Preload
    let mut loader_mut = app.world_mut().resource_mut::<AssetLoader>();

    let task1 = loader_mut.pop_next().unwrap();
    assert_eq!(task1.priority, LoadPriority::CurrentAccess);
    assert_eq!(task1.path, "current1.png".into());

    let task2 = loader_mut.pop_next().unwrap();
    assert_eq!(task2.priority, LoadPriority::CurrentAccess);
    assert_eq!(task2.path, "current2.png".into());

    let task3 = loader_mut.pop_next().unwrap();
    assert_eq!(task3.priority, LoadPriority::HotReload);
    assert_eq!(task3.path, "hotreload1.png".into());

    let task4 = loader_mut.pop_next().unwrap();
    assert_eq!(task4.priority, LoadPriority::Preload);
    assert_eq!(task4.path, "preload1.png".into());

    let task5 = loader_mut.pop_next().unwrap();
    assert_eq!(task5.priority, LoadPriority::Preload);
    assert_eq!(task5.path, "preload2.png".into());

    assert_eq!(loader_mut.queue_len(), 0);
}

/// Functional test: Test concurrent control.
#[test]
fn test_asset_loader_concurrent_control() {
    let mut app = App::new();
    app.init_resource::<AssetLoader>();

    let mut loader = app.world_mut().resource_mut::<AssetLoader>();

    // Default max concurrent is 4
    assert_eq!(loader.active_tasks(), 0);
    assert!(loader.can_start_task());

    // Start tasks up to max concurrent limit
    loader.start_task();
    loader.start_task();
    loader.start_task();
    assert_eq!(loader.active_tasks(), 3);
    assert!(loader.can_start_task());

    // Reach max concurrent limit
    loader.start_task();
    assert_eq!(loader.active_tasks(), 4);
    assert!(!loader.can_start_task());

    // Finish one task, should allow starting new tasks again
    loader.finish_task();
    assert_eq!(loader.active_tasks(), 3);
    assert!(loader.can_start_task());
}
