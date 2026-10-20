use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;

mod be;
mod buck;
mod buck2;
mod deploy;
mod download;
mod nsc;
mod pins;
mod platform;
mod quiet;
mod server;
mod setup;
mod source;
mod switch;
mod test;
mod verify;

pub use pins::Pins;
pub use quiet::Quiet;
pub use test::crate_of_function;

pub type Result<T> = std::result::Result<T, String>;

const USAGE: &str =
    "usage: bootstrap buck|test|verify|ci|be|deploy|setup|switch|install-buck2|install-nsc ...";

pub fn run() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_default();
    let rest: Vec<String> = arguments.collect();
    if let Some(code) = source::run_from_source(&command, &rest) {
        return code;
    }
    let result = match command.as_str() {
        "buck" => buck::main(rest),
        "test" => test::main(rest),
        "verify" => verify::verify(rest),
        "ci" => verify::ci(rest),
        "be" => be::main(rest),
        "deploy" => deploy::main(rest),
        "setup" => setup::main(rest),
        "switch" => switch::main(rest),
        "install-buck2" => buck2::install_command(rest),
        "install-nsc" => nsc::main(rest),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("{message}");
            }
            ExitCode::FAILURE
        }
    }
}

pub fn repository() -> &'static Path {
    static REPOSITORY: OnceLock<PathBuf> = OnceLock::new();
    REPOSITORY.get_or_init(|| {
        let is_root = |directory: &Path| directory.join("scripts/internal/pins").is_file();
        let from_executable = std::env::current_exe()
            .ok()
            .and_then(|executable| executable.ancestors().find(|directory| is_root(directory)).map(Path::to_path_buf));
        let from_directory = || {
            std::env::current_dir()
                .ok()
                .and_then(|directory| directory.ancestors().find(|directory| is_root(directory)).map(Path::to_path_buf))
        };
        from_executable.or_else(from_directory).unwrap_or_else(|| {
            eprintln!("Run this from inside the be3 checkout: no directory above it holds scripts/internal/pins.");
            std::process::exit(1)
        })
    })
}

fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn in_ci() -> bool {
    std::env::var("CI").is_ok_and(|value| !value.is_empty() && value != "false")
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
    if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(false);
    }
    let partial = partial_path(path);
    std::fs::write(&partial, bytes)
        .map_err(|error| format!("could not write {}: {error}", partial.display()))?;
    std::fs::rename(&partial, path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(true)
}

fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.partial", std::process::id()));
    path.with_file_name(name)
}


#[cfg(test)]
mod tests;
