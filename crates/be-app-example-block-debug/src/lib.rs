pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    debug => app::BlockDebugApp,
});

#[cfg(test)]
mod tests;
