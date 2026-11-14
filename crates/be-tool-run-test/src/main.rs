use std::process::ExitCode;

fn main() -> ExitCode {
    be_tool_run_test::forward_signals();
    be_tool_run_test::adopt_orphans();
    let result = be_tool_run_test::parse(std::env::args_os().skip(1))
        .and_then(|options| be_tool_run_test::run(&options));
    match result {
        Ok(status) => ExitCode::from(u8::try_from(status).unwrap_or(1)),
        Err(message) => {
            eprintln!("run-test: {message}");
            ExitCode::FAILURE
        }
    }
}
