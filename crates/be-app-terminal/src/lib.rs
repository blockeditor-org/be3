pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    terminal => app::TerminalApp,
});

#[cfg(test)]
mod tests;
