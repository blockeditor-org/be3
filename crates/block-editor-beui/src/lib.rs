pub use beui;
pub use block_editor_plugin;
pub use block_editor_plugin::*;

pub mod beui_frame;
mod block_link;
mod child;
mod chrome;
pub mod database;
mod datetime;
mod editor;
mod file_chooser;
pub mod headless;
mod instance;
mod related_content;
pub mod version_control;

use be_block::presence::PresenceColor;

pub use block_link::{BlockDisplay, BlockLink, watch_block_label};
pub use child::{ChildBlock, ChildHandle as ChildBlockHandle};
pub use chrome::{SIDEBAR_WIDTH, Side, Sidebar, Toolbar};
pub use datetime::DateTimeRow;
pub use editor::{Artifacts, ChildState, ChildTarget, Creation, Drag, Editor, fit_content};
pub use file_chooser::{FileChooser, content_file_creation};
pub use instance::BeuiPlugin;
pub use related_content::RelatedContent;
pub use version_control::{VersionHistory, short_id};

pub trait BeuiApp: 'static {
    fn view(editor: Editor) -> beui::NodeId;
    fn preview_view(_editor: Editor) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn creation_view(_creation: Creation) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        creation.create_block()
    }
    fn connect_artifact(_artifacts: &Artifacts) {}
    fn describe_artifact(_data: &[u8]) -> Result<ArtifactDescription, String> {
        Err("this editor does not generate artifacts".into())
    }
    fn artifact_settings_view(_artifacts: Artifacts) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
    fn intrinsic_size() -> Option<beui::Vec2> {
        None
    }
    fn aspect_ratio() -> Option<f32> {
        None
    }
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
    ($app:ty, $manifest:expr) => {
        $crate::block_editor_plugin::plugin!($crate::BeuiPlugin<$app>, $manifest);
    };
    ($manifest:expr, { $($content:ty => $app:ty),+ $(,)? }) => {
        $crate::block_editor_plugin::plugin!($manifest, { $($content => $crate::BeuiPlugin<$app>),+ });
    };
}
