use std::process::ExitCode;

mod deploy;
#[cfg(unix)]
mod fuzz;
mod verify;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_default();
    let rest = arguments.collect();
    match command.as_str() {
        "verify" => verify::main(rest),
        "deploy" => deploy::main(rest),
        #[cfg(unix)]
        "fuzz" => fuzz::main(rest),
        _ => {
            eprintln!("usage: local-tools verify|deploy|fuzz ...");
            ExitCode::FAILURE
        }
    }
}
