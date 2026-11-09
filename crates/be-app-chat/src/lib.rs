pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    chat => root::ChatRoot,
});

#[cfg(test)]
mod tests;
