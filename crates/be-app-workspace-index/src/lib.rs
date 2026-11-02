pub mod app;

be_sdk_beui::beui_plugin!("../manifest.json", {
    folder => app::WorkspaceIndexApp,
    workspace => app::WorkspaceIndexApp,
});

#[cfg(test)]
mod tests;
