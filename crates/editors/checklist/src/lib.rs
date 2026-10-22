pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    checklist => app::ChecklistApp,
});

#[cfg(test)]
mod tests;
