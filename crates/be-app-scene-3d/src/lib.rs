pub mod root;
mod camera;
mod renderer;

be_sdk_beui::beui_plugin!("../manifest.json", {
    scene => root::Scene3DRoot,
});

#[cfg(test)]
mod tests;
