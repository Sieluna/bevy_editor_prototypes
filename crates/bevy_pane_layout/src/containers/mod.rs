#![allow(unused)] // FIXME: This module is part of https://github.com/cart/bevy/pull/34 

//! Meta-module containing all feathers containers (passive widgets that hold other widgets).
mod flex_spacer;
mod pane;
mod subpane;

pub use flex_spacer::flex_spacer;
pub use pane::{pane, pane_body, pane_header, pane_header_divider};
pub use subpane::{subpane, subpane_body, subpane_header};

/// Size constants
pub mod size {
    use bevy::ui::Val;

    /// Height for pane headers
    pub const HEADER_HEIGHT: Val = Val::Px(30.0);

    /// Common size for toolbar buttons.
    pub const TOOL_HEIGHT: Val = Val::Px(18.0);
}

pub mod tokens {
    use bevy::feathers::theme::ThemeToken;

    // Pane

    /// Pane header background
    pub const PANE_HEADER_BG: ThemeToken = ThemeToken::new_static("feathers.pane.header.bg");
    /// Pane header border
    pub const PANE_HEADER_BORDER: ThemeToken =
        ThemeToken::new_static("feathers.pane.header.border");
    /// Pane header text color
    pub const PANE_HEADER_TEXT: ThemeToken = ThemeToken::new_static("feathers.pane.header.text");
    /// Pane header divider color
    pub const PANE_HEADER_DIVIDER: ThemeToken =
        ThemeToken::new_static("feathers.pane.header.divider");

    // Subpane

    /// Subpane background
    pub const SUBPANE_HEADER_BG: ThemeToken = ThemeToken::new_static("feathers.subpane.header.bg");
    /// Subpane header border
    pub const SUBPANE_HEADER_BORDER: ThemeToken =
        ThemeToken::new_static("feathers.subpane.header.border");
    /// Subpane header text color
    pub const SUBPANE_HEADER_TEXT: ThemeToken =
        ThemeToken::new_static("feathers.subpane.header.text");
    /// Subpane body background
    pub const SUBPANE_BODY_BG: ThemeToken = ThemeToken::new_static("feathers.subpane.body.bg");
    /// Subpane body border
    pub const SUBPANE_BODY_BORDER: ThemeToken =
        ThemeToken::new_static("feathers.subpane.body.border");
}
