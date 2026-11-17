use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: program-archive pack DIR ARCHIVE
       program-archive run [--beside=FILE]... ARCHIVE PROGRAM [ARGUMENTS...]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [command, dir, archive] if command == "pack" => {
            be_host_archive::pack(dir.as_ref(), archive.as_ref())
        }
        [command, rest @ ..] if command == "run" => {
            let beside: Vec<PathBuf> = rest
                .iter()
                .map_while(|argument| argument.strip_prefix("--beside="))
                .map(PathBuf::from)
                .collect();
            match &rest[beside.len()..] {
                [archive, program, rest @ ..] => {
                    be_host_archive::run(archive.as_ref(), &beside, program, rest)
                }
                _ => Err(USAGE.to_owned()),
            }
        }
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("program-archive: {message}");
            ExitCode::FAILURE
        }
    }
}
