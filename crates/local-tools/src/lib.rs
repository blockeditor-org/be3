use std::process::ExitCode;

mod apk_sign;
mod deploy;
#[cfg(unix)]
mod fuzz;
mod previews;
mod rasterize;
mod verify;

pub fn run() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_default();
    let rest = arguments.collect();
    match command.as_str() {
        "apk-sign" => apk_sign::main(rest),
        "rasterize" => rasterize::main(rest),
        "verify" => verify::main(rest),
        "deploy" => deploy::main(rest),
        #[cfg(unix)]
        "fuzz" => fuzz::main(rest),
        _ => {
            eprintln!("usage: local-tools verify|deploy|fuzz|rasterize|apk-sign ...");
            ExitCode::FAILURE
        }
    }
}
