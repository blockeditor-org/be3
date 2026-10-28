pub mod app;
pub mod picker;

block_editor_beui::beui_plugin!("../manifest.json", {
    workspace => app::WorkspaceUiApp,
    picker => picker::PickerApp,
});

#[cfg(test)]
mod tests;
