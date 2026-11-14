use std::process::ExitCode;

fn main() -> ExitCode {
    let result = be_tool_paint_compare::parse(std::env::args_os().skip(1))
        .and_then(|options| be_tool_paint_compare::run(&options));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("paint-compare: {message}");
            ExitCode::FAILURE
        }
    }
}
