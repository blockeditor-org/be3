use std::time::{SystemTime, UNIX_EPOCH};

use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod tasks;
mod ui;

use ui::WorktreeView;

pub struct VersionControlWorktreeApp;

impl block_editor_plugin::BeuiApp for VersionControlWorktreeApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <WorktreeView editor={editor} />
        }
    }
}

pub(crate) fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
