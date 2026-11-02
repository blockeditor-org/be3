pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    terminal => root::TerminalRoot,
});

#[cfg(test)]
mod tests;
