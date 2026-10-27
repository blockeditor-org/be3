#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = String::new();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--delay-measurement" => beui::native::testing::delay_measurement(true),
            _ => fixture = argument,
        }
    }
    beui::native::run_with(
        beui::native::RunOptions::default(),
        beui_shared_tests::FixtureApp::new(fixture),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
