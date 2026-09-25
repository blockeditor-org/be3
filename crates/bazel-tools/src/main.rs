use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!("usage: bazel-tools clippy|rust-project|resolve|apk ...");
        return ExitCode::FAILURE;
    };
    let result = match command.as_str() {
        "clippy" => bazel_tools::clippy::run(rest),
        "rust-project" => bazel_tools::rust_project::run(rest),
        "resolve" => bazel_tools::resolve::run(rest),
        "apk" => bazel_tools::apk::run(rest),
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("bazel-tools {command}: {message}");
            ExitCode::FAILURE
        }
    }
}
