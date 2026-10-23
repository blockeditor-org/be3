pub mod app;

block_editor_beui::beui_plugin!("../manifest.json", {
    terminal => app::TerminalApp,
});

#[cfg(test)]
mod tests;
