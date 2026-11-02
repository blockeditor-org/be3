pub mod app;
pub mod picker;
pub mod rename;

be_sdk_beui::beui_plugin!("../manifest.json", {
    workspace => app::WorkspaceUiApp,
    picker => picker::PickerApp,
    rename => rename::RenameApp,
});

#[cfg(test)]
mod tests;
