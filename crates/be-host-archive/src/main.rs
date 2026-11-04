use std::process::ExitCode;

const USAGE: &str = "usage: program-archive pack DIR ARCHIVE
       program-archive run ARCHIVE PROGRAM [ARGUMENTS...]";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [command, dir, archive] if command == "pack" => {
            be_host_archive::pack(dir.as_ref(), archive.as_ref())
        }
        [command, archive, program, rest @ ..] if command == "run" => {
            be_host_archive::run(archive.as_ref(), program, rest)
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
