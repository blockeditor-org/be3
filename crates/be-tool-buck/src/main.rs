use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!(
            "usage: buck-tools generate|compile-fail|rust-project|resolve|apk|archive|pdfium|fix|unpinned|bundle ..."
        );
        return ExitCode::FAILURE;
    };
    let result = match command.as_str() {
        "generate" => be_tool_buck::generate::run(rest),
        "compile-fail" => be_tool_buck::compile_fail::run(rest),
        "rust-project" => be_tool_buck::rust_project::run(rest),
        "resolve" => be_tool_buck::resolve::run(rest),
        "apk" => be_tool_buck::apk::run(rest),
        "archive" => be_tool_buck::archive::run(rest),
        "pdfium" => be_tool_buck::pdfium::run(rest),
        "fix" => be_tool_buck::fix::run(rest),
        "unpinned" => be_tool_buck::unpinned::run(rest),
        "bundle" => be_tool_buck::bundle::run(rest),
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
