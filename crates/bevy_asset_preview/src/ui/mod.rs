use std::path::PathBuf;

use bevy::{asset::AssetServer, prelude::*};

#[derive(Component, Deref)]
pub struct PreviewAsset(pub PathBuf);

const FILE_PLACEHOLDER: &'static str = "embedded://bevy_asset_browser/assets/file_icon.png";

pub fn preview_handler(
    mut commands: Commands,
    mut requests_query: Query<(Entity, &PreviewAsset)>,
    asset_server: Res<AssetServer>,
) {
    for (entity, preview) in &mut requests_query {
        let preview = asset_server.load(FILE_PLACEHOLDER);

        // TODO: sprite atlas.
        commands.entity(entity).insert(ImageNode::new(preview));
    }
}
