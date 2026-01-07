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
    pub timestamp: u64,
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
        timestamp: u64,
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
