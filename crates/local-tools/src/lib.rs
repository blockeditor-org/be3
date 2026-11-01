mod browser;
use std::process::ExitCode;

mod apk_sign;
mod changed;
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
        "changed" => changed::main(rest),
        "icons" => icons::main(rest),
        "deploy" => deploy::main(rest),
        #[cfg(unix)]
        "fuzz" => fuzz::main(rest),
        _ => {
            eprintln!(
                "usage: local-tools ci|changed|deploy|fuzz|rasterize|icons|web-page|apk-sign ..."
            );
            ExitCode::FAILURE
        }
    }
}

fn git(arguments: &[&str]) -> Option<String> {
    git_bytes(arguments).and_then(|bytes| String::from_utf8(bytes).ok())
}

fn git_bytes(arguments: &[&str]) -> Option<Vec<u8>> {
    let output = std::process::Command::new("git")
        .args(arguments)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
