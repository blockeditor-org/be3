pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    debug => app::BlockDebugApp,
});

#[cfg(test)]
mod tests;
