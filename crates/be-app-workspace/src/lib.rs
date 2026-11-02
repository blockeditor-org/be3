pub mod picker;
pub mod rename;
pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    workspace => root::WorkspaceUiRoot,
    picker => picker::PickerRoot,
    rename => rename::RenameRoot,
});

#[cfg(test)]
mod tests;
