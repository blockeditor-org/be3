mod phone;
pub mod picker;
pub mod rename;
mod revisions;
pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    workspace => root::WorkspaceUiRoot,
    picker => picker::PickerRoot,
    rename => rename::RenameRoot,
});

#[cfg(test)]
mod tests;
