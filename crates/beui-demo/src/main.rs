#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui demo", beui_demo::DemoApp::new())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
