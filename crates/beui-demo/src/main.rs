#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::native::run_with(
        beui::native::RunOptions::default().saving_preferences("beui-demo"),
        beui_demo::DemoApp::new(),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
