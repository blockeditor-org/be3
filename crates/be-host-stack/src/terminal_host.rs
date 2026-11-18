//! Where to get `be` of this host's own build for a terminal host an app installs on a Linux
//! machine of its own (`terminal-host-binary`, guides/terminal.md): a deployed build names
//! the deployment's copy and its SHA-256 (`build-info.json`'s `terminal_hosts`, which
//! `./scripts/deploy` writes), and a development build reads one built on this machine,
//! `//crates/be-tool-cli:terminal-host`: `be` with the apps' manifests and type modules
//! beside it, which it reads blocks through as a `be` staged here does.

use std::path::Path;

use be_lib_build_id::{Metadata, TERMINAL_HOST_ARCHES};
use be_sdk_api::{TerminalHostBe, TerminalHostDownload, TerminalHostFile, TerminalHostSource};

/// Answers `terminal-host-binary(arch)` on a thread of its own, since a development build's
/// `be` is tens of megabytes read from disk.
pub fn binary(arch: String, deliver: impl FnOnce(Result<TerminalHostBe, String>) + Send + 'static) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        deliver(
            be_lib_build_id::metadata()
                .and_then(|metadata| answer(metadata, &arch, built().as_deref())),
        );
    });
    #[cfg(target_arch = "wasm32")]
    deliver(be_lib_build_id::metadata().and_then(|metadata| answer(metadata, &arch, None)));
}

/// The `build-info.json` a `be` of this host's build carries, which names the build.
pub fn build_info() -> Result<String, String> {
    be_lib_build_id::metadata().map(Metadata::for_be)
}

/// Where a development build looks for the `be` built for `arch`: `be3/terminal-host/ARCH` in
/// the user's cache directory.
#[cfg(not(target_arch = "wasm32"))]
fn built() -> Option<std::path::PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|cache| !cache.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
        })
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from))?;
    Some(cache.join("be3").join("terminal-host"))
}

/// The answer for `arch` from this build's metadata, or from the `be` built under `built`
/// (`built/ARCH/be` and its `build-info.json`) when it names none.
pub fn answer(
    metadata: &Metadata,
    arch: &str,
    built: Option<&Path>,
) -> Result<TerminalHostBe, String> {
    if !TERMINAL_HOST_ARCHES.iter().any(|(known, _)| *known == arch) {
        return Err(format!(
            "be is built for x86_64 and arm64 Linux machines, not {arch}"
        ));
    }
    let build_info = metadata.for_be();
    if let Some(deployed) = metadata.terminal_host(arch) {
        return Ok(TerminalHostBe {
            source: TerminalHostSource::Download(TerminalHostDownload {
                url: deployed.url.clone(),
                sha256: deployed.sha256.clone(),
            }),
            build_info,
        });
    }
    let development = metadata.program_id == "be-host-dev";
    let Some(built) = built.filter(|_| development) else {
        return Err(format!(
            "this build of be3 names no be for {arch} Linux machines, so it cannot connect a \
             terminal there"
        ));
    };
    let directory = built.join(arch);
    let build = format!(
        "build it with `./scripts/buck build //crates/be-tool-cli:terminal-host --target-platforms \
         root//buck/platforms:linux_{arch} --out {}`",
        directory.display()
    );
    if !directory.join("be").is_file() {
        return Err(format!(
            "no be for {arch} is built for this development build of be3: {build}"
        ));
    }
    let theirs = std::fs::read_to_string(directory.join(be_lib_build_id::FILE))
        .ok()
        .and_then(|json| Metadata::parse(&json).ok());
    match theirs {
        Some(theirs) if theirs.id == metadata.id => Ok(TerminalHostBe {
            source: TerminalHostSource::Files(built_files(&directory, &build)?),
            build_info,
        }),
        _ => Err(format!(
            "the be for {arch} in {} is of another build than this one: {build}",
            directory.display()
        )),
    }
}

/// The files of a `:terminal-host` build in `directory`: `be`, `apps.json` and each app's
/// manifest and type module, which the app puts beside `be`. `build-info.json` goes up as the
/// answer's own, and the stage's list of files is not needed there.
fn built_files(directory: &Path, build: &str) -> Result<Vec<TerminalHostFile>, String> {
    if !directory.join("apps.json").is_file() {
        return Err(format!(
            "the be in {} has no apps beside it to read blocks through: {build}",
            directory.display()
        ));
    }
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("could not read {}: {error}", directory.display()))?
            .path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == be_lib_build_id::FILE || name == "program-files.txt" || !path.is_file() {
            continue;
        }
        if !TerminalHostFile::plain_name(name) {
            return Err(format!(
                "{} is no plain file name to put beside be; delete it: {build}",
                path.display()
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        files.push(TerminalHostFile {
            name: name.to_owned(),
            bytes,
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}
