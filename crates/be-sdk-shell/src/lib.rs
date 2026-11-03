mod artifact;
mod block_menu;
mod deleted;
mod dialogs;
mod host_panel;
mod linked;
mod menu;
mod panel;
mod parts;
mod program;
mod saved;
mod share;
mod tab;
mod workspace;

pub use dialogs::WorkspaceDialogs;
pub use menu::{BlockAction, Permissions, apply, permissions};
pub use panel::{Deleted, Refs, TabInfo, Watched, read_info};
pub use parts::TabParts;
pub use program::{
    ProgramRecord, ProgramTab, ProgramWindow, UnshownWindows, WindowDialog, dialog_tab,
    picture_image,
};
pub use saved::{SavedProgram, saved_programs};
pub use tab::TabItem;
pub use workspace::{BlockTab, Failure, Focus, PanelStatus, PanelWindow, SHELL_BARS, Workspace};
