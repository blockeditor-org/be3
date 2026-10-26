use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!("usage: buck-tools generate|compile-fail|rust-project|resolve|apk|archive ...");
        return ExitCode::FAILURE;
    };
    let result = match command.as_str() {
        "generate" => buck_tools::generate::run(rest),
        "compile-fail" => buck_tools::compile_fail::run(rest),
        "rust-project" => buck_tools::rust_project::run(rest),
        "resolve" => buck_tools::resolve::run(rest),
        "apk" => buck_tools::apk::run(rest),
        "archive" => buck_tools::archive::run(rest),
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("buck-tools {command}: {message}");
            ExitCode::FAILURE
        }
    }
}
