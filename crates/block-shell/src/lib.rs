mod artifact;
mod block_menu;
mod deleted;
mod dialogs;
mod host_panel;
mod kept;
mod linked;
mod menu;
mod panel;
mod phone;
mod picker;
mod picker_view;
mod picking;
mod rename;
mod revisions;
mod saved;
pub use revisions::RevisionHistoryPanel;
mod share;
mod tab;
mod window;
mod workspace;

pub use dialogs::WorkspaceDialogs;
pub use kept::KeptWindow;
pub use picker_view::PickerView;
pub use picking::Picker;
pub use rename::RenameView;
pub use saved::saved_windows;
pub use window::{program_name, shown_windows, window_name, window_picture, window_tab};
pub use workspace::{
    BlockTab, FILES, Failure, FloatingWindow, PanelStatus, PanelWindow, REVISIONS, SETTINGS,
    SHELL_BARS, WindowDialog, WindowTab, Workspace,
};
