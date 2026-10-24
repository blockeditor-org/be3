mod browser;
use std::process::ExitCode;

mod apk_sign;
mod ci;
mod deploy;
#[cfg(unix)]
mod fuzz;
mod icons;
mod previews;
mod rasterize;
mod web_page;

pub fn run() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_default();
    let rest = arguments.collect();
    match command.as_str() {
        "apk-sign" => apk_sign::main(rest),
        "web-page" => web_page::main(rest),
        "rasterize" => rasterize::main(rest),
        "ci" => ci::main(rest),
        "icons" => icons::main(rest),
        "deploy" => deploy::main(rest),
        #[cfg(unix)]
        "fuzz" => fuzz::main(rest),
        _ => {
            eprintln!("usage: local-tools ci|deploy|fuzz|rasterize|icons|web-page|apk-sign ...");
            ExitCode::FAILURE
        }
    }
}
