pub use beui;
pub use beui::{FileFilter, PickedFile};
#[cfg(test)]
extern crate beui_platform_plugin as _;
pub use block_editor_plugin;
pub use block_editor_plugin::*;

pub mod beui_frame;
mod block_link;
mod child;
mod chrome;
mod dialog;
mod dock;
mod docked;
mod editor;
mod file_chooser;
mod fitting;
pub mod headless;
mod instance;
mod layered_settings;
mod loading;
mod module;
mod picking;
mod preview;
mod related_content;
mod rotation;
pub mod sections;
pub mod version_control;

use be_block::presence::PresenceColor;

pub use beui_frame::phone_layout;
pub use block_link::{BlockDisplay, BlockLink, label_glyph, watch_block_label};
pub use child::{ChildBlock, ChildDockTab, ChildHandle as ChildBlockHandle, Subregion};
pub use chrome::{
    NARROW_WIDTH, SIDEBAR_WIDTH, Side, Sidebar, Toolbar, narrow_chrome, sheet_control, sheet_open,
};
pub use dock::BottomDock;
pub use editor::{
    Artifacts, ChildState, ChildTarget, Creation, Drag, Editor, KindsAbove, OrganizationLayers,
    OwnedEditor, SetKind, SubregionContent, fit_content, node_anchor,
};
pub use file_chooser::{ContentFileCreation, FileChooser};
pub use fitting::{Fit, Fitting};
pub use instance::BeuiPlugin;
pub use layered_settings::{
    ChangesToast, LayeredSettings, LayeredSettingsRow, SettingHandle, SettingsChanges,
};
pub use loading::{FailedPane, Loaded, LoadingPane};
pub use picking::PickAnchor;
pub use related_content::RelatedContent;
pub use rotation::RotationPicker;
pub use version_control::{VersionHistory, VersionSteps, short_id};

#[beui::reactive::component]
pub trait BeuiApp: 'static {
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
    fn aspect_ratio() -> Option<f32> {
        None
    }
}

pub fn context() -> beui::native::Context {
    beui_platform_plugin_api::runtime().context()
}

pub fn presence_color(color: PresenceColor) -> beui::Color32 {
    match color {
        PresenceColor::Red => beui::Color32::from_rgb(224, 82, 82),
        PresenceColor::Orange => beui::Color32::from_rgb(230, 140, 50),
        PresenceColor::Yellow => beui::Color32::from_rgb(214, 179, 41),
        PresenceColor::Green => beui::Color32::from_rgb(84, 171, 90),
        PresenceColor::Teal => beui::Color32::from_rgb(46, 173, 168),
        PresenceColor::Blue => beui::Color32::from_rgb(74, 134, 227),
        PresenceColor::Purple => beui::Color32::from_rgb(150, 100, 214),
        PresenceColor::Pink => beui::Color32::from_rgb(224, 104, 168),
    }
}

#[macro_export]
macro_rules! beui_plugin {
    ($manifest:expr, { $($name:ident => $app:ty),+ $(,)? }) => {
        $crate::block_editor_plugin::plugin!($manifest, { $($name => $crate::BeuiPlugin<$app>),+ });
    };
}
