pub mod root;
mod wire;

be_sdk_beui::beui_app!("../manifest.json", {
    hunt => root::HuntRoot,
    hunt_host => root::HuntHost,
});

#[cfg(test)]
mod tests;
