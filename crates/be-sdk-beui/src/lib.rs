pub use beui;
pub use beui::{FileFilter, PickedFile};
#[cfg(test)]
extern crate be_sdk_beui_platform as _;
pub use be_sdk;
pub use be_sdk::*;
pub use beui::reactive;

pub mod beui_frame;
mod block_link;
mod child;
mod chrome;
mod content;
mod dialog;
mod docked;
mod editor;
mod file_chooser;
mod fitting;
mod graph;
pub mod headless;
mod instance;
mod layered_settings;
mod loading;
mod module;
mod picking;
mod popover;
mod preview;
mod related_content;
mod rotation;
pub mod sections;
pub mod version_control;
mod zoom;

pub use beui_frame::phone_layout;
pub use block_link::{BlockDisplay, BlockLink, label_glyph, watch_block_label};
pub use child::{ChildBlock, ChildBlockHandle, ChildDockTab, Subregion};
pub use chrome::{NARROW_WIDTH, SIDE_PANEL_WIDTH, Toolbar, narrow_chrome};
pub use content::{ContentProjection, DragHold, Lacking, OwnedContentProjection, Payload};
pub use docked::SavedDockLayout;
pub use editor::{
    Artifacts, Creation, Editor, KindsAbove, OrganizationLayers, OwnedEditor, SetKind,
    ViewStateHandle, node_anchor,
};
pub use file_chooser::{ContentFileCreation, FileChooser};
pub use fitting::Fitting;
pub use graph::{BlockList, Blocks};
pub use instance::BeuiApp;
pub use layered_settings::{
    ChangesToast, LayeredSettings, LayeredSettingsRow, SettingHandle, SettingsChanges,
};
pub use loading::{FailedPane, Loaded, LoadingPane, answer};
pub use picking::PickAnchor;
pub use related_content::RelatedContent;
pub use rotation::RotationPicker;
pub use version_control::{VersionHistory, VersionSteps};
pub use zoom::{ZoomActions, ZoomBar};

#[beui::reactive::component]
pub trait BeuiRoot: 'static {
    #[beui::reactive::component]
    fn View(editor: Editor) -> beui::styled::Root;
    #[beui::reactive::component]
    fn Preview(editor: Editor) -> beui::NodeId {
        let _ = editor;
        beui::reactive::view! {
            <beui::styled::Frame />
        }
    }
    #[beui::reactive::component]
    fn Display(editor: Editor, display: u32) -> beui::NodeId {
        let _ = (editor, display);
        beui::reactive::view! {
            <beui::styled::Frame />
        }
    }
    #[beui::reactive::component]
    fn Creation(creation: Creation) -> beui::NodeId {
        let _ = creation;
        beui::reactive::view! {
            <beui::styled::Frame />
        }
    }
    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        creation.create_block()
    }
    /// Runs a host component (the manifest's `hosts`, guides/apps/host_services.md, "Host
    /// components"), once, as it opens on its block, in a scope that lives as long as it does:
    /// it has no view, so what it does it starts here, with effects on the editor's content.
    fn host(editor: Editor) {
        let _ = editor;
    }
    fn connect_artifact(_artifacts: &Artifacts) {}
    fn describe_artifact(_data: &[u8]) -> Result<String, String> {
        Err("this editor does not generate artifacts".into())
    }
    #[beui::reactive::component]
    fn ArtifactSettings(artifacts: Artifacts) -> beui::NodeId {
        let _ = artifacts;
        beui::reactive::view! {
            <beui::styled::Frame />
        }
    }
    fn intrinsic_size() -> Option<beui::Vec2> {
        None
    }
}

pub fn context() -> beui::native::Context {
    be_sdk_beui_platform_api::runtime().context()
}

/// A root an app's manifest declares editors of: `beui_app!` implements it for each root it
/// names, so whatever runs the root (be-sdk-test's `BeuiTest`) reads what the manifest says of
/// it, such as the camera limits the host gives it.
pub trait AppEditor {
    /// The app's `manifest.json`.
    const MANIFEST: &'static str;
    /// The keys under the manifest's `editors` this root is the editor of.
    const EDITORS: &'static [&'static str];
}

/// The app's entry point: `name => Root` for each editor of the manifest, or
/// `first | second => Root` for one root several editors share.
#[macro_export]
macro_rules! beui_app {
    ($manifest:expr, { $($($name:ident)|+ => $root:ty),+ $(,)? }) => {
        $crate::__beui_app_entries!($manifest; []; $($($name)|+ => $root),+);
        $(
            impl $crate::AppEditor for $root {
                const MANIFEST: &'static str = APP_MANIFEST;
                const EDITORS: &'static [&'static str] = &[$(stringify!($name)),+];
            }
        )+
    };
}

/// `beui_app!`'s entries, one per editor name.
#[doc(hidden)]
#[macro_export]
macro_rules! __beui_app_entries {
    ($manifest:expr; [$($done:tt)*];) => {
        $crate::be_sdk::app!($manifest, { $($done)* });
    };
    ($manifest:expr; [$($done:tt)*]; $($name:ident)|+ => $root:ty $(, $($rest:tt)*)?) => {
        $crate::__beui_app_entries!(
            $manifest;
            [$($done)* $($name => $crate::BeuiApp<$root>,)+];
            $($($rest)*)?
        );
    };
}
