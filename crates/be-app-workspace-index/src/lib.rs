pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    folder | workspace => root::WorkspaceIndexRoot,
});

#[cfg(test)]
mod tests;
