pub mod root;

be_sdk_beui::beui_plugin!("../manifest.json", {
    debug => root::BlockDebugRoot,
});

#[cfg(test)]
mod tests;
