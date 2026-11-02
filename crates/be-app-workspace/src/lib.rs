pub mod root;
pub mod picker;
pub mod rename;

be_sdk_beui::beui_plugin!("../manifest.json", {
    workspace => root::WorkspaceUiRoot,
    picker => picker::PickerRoot,
    rename => rename::RenameRoot,
});

#[cfg(test)]
mod tests;
