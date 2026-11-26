mod artifact;
mod block_menu;
mod deleted;
mod dialogs;
mod host_panel;
mod linked;
mod log_doubt;
mod menu;
mod panel;
mod program;
mod saved;
mod share;
mod tab;
mod workspace;

pub use dialogs::WorkspaceDialogs;
pub use menu::{BlockAction, Permissions, apply, permissions};
pub use panel::{Deleted, Refs, TabInfo, Watched, read_info};
pub use program::{
    ProgramRecord, ProgramTab, ProgramWindow, UnshownWindows, WindowDialog, dialog_tab,
    picture_image,
};
pub use saved::{ClosedViews, SavedLayout, SavedProgram, saved_programs};
pub use tab::TabItem;
pub use workspace::{BlockTab, Focus, PanelStatus, PanelWindow, SHELL_BARS, Workspace};
