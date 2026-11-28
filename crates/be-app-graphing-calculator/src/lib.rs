pub mod root;

be_sdk_beui::beui_app!("../manifest.json", {
    graphing_calculator => root::GraphRoot,
});
