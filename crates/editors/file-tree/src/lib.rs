pub mod app;

#[cfg(test)]
mod tests;

block_editor_beui::beui_plugin!("../manifest.json", {
    files => app::FileTreeApp,
});
