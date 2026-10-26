use fix_rust_source::{fix_docs, fix_repository};
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => fix_repository(Path::new("."), false),
        ["--check"] => fix_repository(Path::new("."), true),
        ["docs", code] => fix_docs(Path::new("."), Path::new(code)),
        _ => Err(format!(
            "usage: fix-rust-source [--check] | fix-rust-source docs CODE (got {arguments:?})"
        )
        .into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
