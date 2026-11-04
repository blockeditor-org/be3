pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    terminal => root::TerminalRoot,
});

#[cfg(test)]
mod tests;
