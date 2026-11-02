pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    folder => root::WorkspaceIndexRoot,
    workspace => root::WorkspaceIndexRoot,
});

#[cfg(test)]
mod tests;
