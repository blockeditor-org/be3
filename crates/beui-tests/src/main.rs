beui_host_window::main! {
    async fn main() -> Result<(), Box<dyn std::error::Error>> {
        let mut fixture = String::new();
        for argument in std::env::args().skip(1) {
            match argument.as_str() {
                "--delay-measurement" => beui_host::native::testing::delay_measurement(true),
                _ => fixture = argument,
            }
        }
        beui_host_window::run_program(
            beui_host_window::RunOptions::default(),
            beui_tests::FixtureProgram::new(fixture),
        )
        .await
    }
}
