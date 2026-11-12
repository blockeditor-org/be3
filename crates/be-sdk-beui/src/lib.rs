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

pub use beui_frame::phone_layout;
pub use block_link::{BlockDisplay, BlockLink, label_glyph, watch_block_label};
pub use child::{ChildBlock, ChildDockTab, ChildHandle as ChildBlockHandle, Subregion};
pub use chrome::{NARROW_WIDTH, SIDE_PANEL_WIDTH, Toolbar, narrow_chrome};
pub use content::{ContentProjection, Lacking, OwnedContentProjection, Payload};
pub use docked::SavedDockLayout;
pub use editor::{
    Artifacts, Creation, Editor, KindsAbove, OrganizationLayers, OwnedEditor, SetKind, node_anchor,
};
pub use file_chooser::{ContentFileCreation, FileChooser};
pub use fitting::Fitting;
pub use graph::{BlockList, Blocks};
pub use instance::BeuiApp;
pub use layered_settings::{
    ChangesToast, LayeredSettings, LayeredSettingsRow, SettingHandle, SettingsChanges,
};
pub use loading::{FailedPane, Loaded, LoadingPane};
pub use picking::PickAnchor;
pub use related_content::RelatedContent;
pub use rotation::RotationPicker;
pub use version_control::{VersionHistory, VersionSteps};

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

#[macro_export]
macro_rules! beui_app {
    ($manifest:expr, { $($name:ident => $root:ty),+ $(,)? }) => {
        $crate::be_sdk::app!($manifest, { $($name => $crate::BeuiApp<$root>),+ });
    };
}
