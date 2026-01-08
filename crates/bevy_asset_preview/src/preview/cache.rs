use core::time::Duration;

use bevy::{
    asset::{AssetId, AssetPath},
    image::Image,
    platform::collections::HashMap,
    prelude::{Handle, Resource},
};

/// Cache entry for a preview image.
#[derive(Clone, Debug)]
pub struct PreviewCacheEntry {
    /// The preview image handle.
    pub image_handle: Handle<Image>,
    /// The asset ID that this preview is for.
    pub asset_id: AssetId<Image>,
    /// Timestamp when the preview was generated (for cache invalidation).
    pub timestamp: Duration,
}

/// Cache for preview images to avoid re-rendering unchanged assets.
#[derive(Resource, Default)]
pub struct PreviewCache {
    /// Maps asset path to cache entry.
    path_cache: HashMap<AssetPath<'static>, PreviewCacheEntry>,
    /// Maps asset ID to cache entry.
    id_cache: HashMap<AssetId<Image>, PreviewCacheEntry>,
}

impl PreviewCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self {
            path_cache: HashMap::new(),
            id_cache: HashMap::new(),
        }
    }

    /// Gets a cached preview by asset path.
    pub fn get_by_path<'a>(&self, path: &AssetPath<'a>) -> Option<&PreviewCacheEntry> {
        // Convert to owned path for lookup
        let owned_path: AssetPath<'static> = path.clone().into_owned();
        self.path_cache.get(&owned_path)
    }

    /// Gets a cached preview by asset ID.
    pub fn get_by_id(&self, asset_id: AssetId<Image>) -> Option<&PreviewCacheEntry> {
        self.id_cache.get(&asset_id)
    }

    /// Inserts a preview into the cache.
    pub fn insert<'a>(
        &mut self,
        path: impl Into<AssetPath<'a>>,
        asset_id: AssetId<Image>,
        image_handle: Handle<Image>,
        timestamp: Duration,
    ) {
        let path: AssetPath<'static> = path.into().into_owned();
        let entry = PreviewCacheEntry {
            image_handle,
            asset_id,
            timestamp,
        };
        self.path_cache.insert(path.clone(), entry.clone());
        self.id_cache.insert(asset_id, entry);
    }

    /// Removes a preview from the cache by path.
    pub fn remove_by_path<'a>(&mut self, path: &AssetPath<'a>) -> Option<PreviewCacheEntry> {
        // Convert to owned path for lookup
        let owned_path: AssetPath<'static> = path.clone().into_owned();
        if let Some(entry) = self.path_cache.remove(&owned_path) {
            self.id_cache.remove(&entry.asset_id);
            Some(entry)
        } else {
            None
        }
    }

    /// Removes a preview from the cache by asset ID.
    pub fn remove_by_id(&mut self, asset_id: AssetId<Image>) -> Option<PreviewCacheEntry> {
        if let Some(entry) = self.id_cache.remove(&asset_id) {
            // Find and remove from path cache
            self.path_cache.retain(|_, e| e.asset_id != asset_id);
            Some(entry)
        } else {
            None
        }
    }

    /// Clears all cached previews.
    pub fn clear(&mut self) {
        self.path_cache.clear();
        self.id_cache.clear();
    }

    /// Returns the number of cached previews.
    pub fn len(&self) -> usize {
        self.path_cache.len()
    }

    /// Checks if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.path_cache.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use bevy::{
        asset::AssetPlugin,
        image::Image,
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
    fn test_preview_cache_operations() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let assets_dir = temp_dir.path().join("assets");
        fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin {
                file_path: assets_dir.display().to_string(),
                ..Default::default()
            })
            .init_resource::<PreviewCache>()
            .init_asset::<Image>();

        // Create test images
        let (handle1, handle2, asset_id1, asset_id2) =
            app.world_mut()
                .resource_scope(|_world, mut images: Mut<Assets<Image>>| {
                    let h1 = create_test_image(&mut images, 64, 64, [255, 0, 0, 255]);
                    let h2 = create_test_image(&mut images, 64, 64, [0, 255, 0, 255]);
                    let id1 = h1.id();
                    let id2 = h2.id();
                    (h1, h2, id1, id2)
                });

        let path1: AssetPath<'static> = "test1.png".into();
        let path2: AssetPath<'static> = "test2.png".into();

        let mut cache = app.world_mut().resource_mut::<PreviewCache>();

        // Test insertion and query by path
        cache.insert(&path1, asset_id1, handle1.clone(), Duration::from_secs(100));
        cache.insert(&path2, asset_id2, handle2.clone(), Duration::from_secs(200));

        assert_eq!(cache.len(), 2, "Cache should contain 2 entries");
        assert!(!cache.is_empty(), "Cache should not be empty");

        let entry1 = cache.get_by_path(&path1).unwrap();
        assert_eq!(entry1.asset_id, asset_id1, "Entry should match asset ID");
        assert_eq!(
            entry1.timestamp,
            Duration::from_secs(100),
            "Entry should match timestamp"
        );

        // Test query by ID
        let entry1_by_id = cache.get_by_id(asset_id1).unwrap();
        assert_eq!(
            entry1_by_id.image_handle.id(),
            handle1.id(),
            "Entry by ID should match"
        );

        // Test consistency between path and ID queries
        let entry2_by_path = cache.get_by_path(&path2).unwrap();
        let entry2_by_id = cache.get_by_id(asset_id2).unwrap();
        assert_eq!(
            entry2_by_path.image_handle.id(),
            entry2_by_id.image_handle.id(),
            "Path and ID queries should return same entry"
        );

        // Test removal by path
        let removed = cache.remove_by_path(&path1).unwrap();
        assert_eq!(removed.asset_id, asset_id1, "Removed entry should match");
        assert_eq!(cache.len(), 1, "Cache should have 1 entry after removal");
        assert!(
            cache.get_by_path(&path1).is_none(),
            "Path should be removed"
        );
        assert!(cache.get_by_id(asset_id1).is_none(), "ID should be removed");

        // Test removal by ID
        let removed2 = cache.remove_by_id(asset_id2).unwrap();
        assert_eq!(removed2.asset_id, asset_id2, "Removed entry should match");
        assert_eq!(cache.len(), 0, "Cache should be empty");
        assert!(cache.is_empty(), "Cache should be empty");

        // Test duplicate insertion (should overwrite)
        let handle1_clone = handle1.clone();
        let handle1_clone2 = handle1.clone();
        cache.insert(&path1, asset_id1, handle1_clone, Duration::from_secs(100));
        cache.insert(&path1, asset_id1, handle1_clone2, Duration::from_secs(300)); // Overwrite
        assert_eq!(
            cache.len(),
            1,
            "Cache should have 1 entry after duplicate insert"
        );
        assert_eq!(
            cache.get_by_path(&path1).unwrap().timestamp,
            Duration::from_secs(300),
            "Entry should be updated"
        );

        // Test clearing
        cache.clear();
        assert_eq!(cache.len(), 0, "Cache should be empty after clear");
        assert!(cache.is_empty(), "Cache should be empty after clear");
    }
}
