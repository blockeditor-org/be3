use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const FILE: &str = "build-info.json";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Metadata {
    pub id: String,
    pub program_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_server: Option<DefaultServer>,
    /// Where a deployed build's `be` is, per Linux architecture, which `./scripts/deploy`
    /// writes once it has uploaded them: a terminal host the host installs on a machine of an
    /// app's (guides/terminal.md) must be of the same build. Empty in other builds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminal_hosts: Vec<TerminalHostBinary>,
}

/// A deployed `be` for one architecture: `x86_64` or `arm64`, its URL and its SHA-256 in
/// lower-case hex.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct TerminalHostBinary {
    pub arch: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct DefaultServer {
    pub name: String,
    pub api_url: String,
    pub auth_url: String,
    pub gateway_required: bool,
}

impl Metadata {
    pub fn parse(json: &str) -> Result<Self, String> {
        let info: Self = serde_json::from_str(json)
            .map_err(|error| format!("invalid build metadata: {error}"))?;
        if info.id.trim().is_empty() || info.id == "unknown" || info.program_id.trim().is_empty() {
            return Err("the build metadata has no build or program identity".to_owned());
        }
        Ok(info)
    }

    /// The deployed `be` for a Linux machine of `arch`, if this build was deployed with one.
    pub fn terminal_host(&self, arch: &str) -> Option<&TerminalHostBinary> {
        self.terminal_hosts.iter().find(|binary| binary.arch == arch)
    }

    /// This metadata as the `build-info.json` of a `be` of the same build: what names the
    /// build, without the deployment's own `be`s.
    pub fn for_be(&self) -> String {
        let mut metadata = self.clone();
        metadata.terminal_hosts.clear();
        let mut json = serde_json::to_string(&metadata).unwrap_or_default();
        json.push('\n');
        json
    }
}

/// The Linux architectures a terminal host is built for, as the host names them, with the
/// target triple each `be` is built for.
pub const TERMINAL_HOST_ARCHES: [(&str, &str); 2] = [
    ("x86_64", "x86_64-unknown-linux-gnu"),
    ("arm64", "aarch64-unknown-linux-gnu"),
];

static CURRENT: OnceLock<Result<Metadata, String>> = OnceLock::new();

pub fn metadata() -> Result<&'static Metadata, String> {
    CURRENT
        .get_or_init(beside_executable)
        .as_ref()
        .map_err(Clone::clone)
}

pub fn current() -> Result<&'static str, String> {
    metadata().map(|info| info.id.as_str())
}

pub fn install(id: &str) {
    let _ = CURRENT.set(Ok(Metadata {
        id: id.to_owned(),
        program_id: "be-host-dev".to_owned(),
        default_server: None,
        terminal_hosts: Vec::new(),
    }));
}

pub fn install_metadata(json: &str) -> Result<(), String> {
    let info = Metadata::parse(json)?;
    let _ = CURRENT.set(Ok(info));
    Ok(())
}

pub fn install_from(path: &Path) -> Result<(), String> {
    let json = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    install_metadata(&json)
}

pub fn compatible(ours: &str, theirs: &str) -> bool {
    ours == theirs
}

/// The directories a program finds its bundle's files in, nearest first: those of `BE_BUNDLE`
/// (a path list, as `PATH` is) when it is set, else the one its executable is in. A test's host
/// uses the list to stage only the apps it names in one directory and every library in
/// another that all the tests share, beside a `be-host` they all share.
pub fn bundle_directories() -> Result<Vec<PathBuf>, String> {
    if let Some(list) = std::env::var_os("BE_BUNDLE").filter(|list| !list.is_empty()) {
        return Ok(std::env::split_paths(&list).collect());
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot find this program's own path: {error}"))?;
    let directory = executable
        .parent()
        .ok_or("the executable is in no directory")?;
    Ok(vec![directory.to_path_buf()])
}

/// The path of `file` in the first of `bundle_directories` that has it, or in the first when none
/// does, which is the one whose error says where it looked.
pub fn bundle_file(file: &str) -> Result<PathBuf, String> {
    find_in(&bundle_directories()?, file)
}

fn find_in(directories: &[PathBuf], file: &str) -> Result<PathBuf, String> {
    let first = directories.first().ok_or("BE_BUNDLE names no directory")?;
    Ok(directories
        .iter()
        .map(|directory| directory.join(file))
        .find(|path| path.exists())
        .unwrap_or_else(|| first.join(file)))
}

fn beside_executable() -> Result<Metadata, String> {
    let path = bundle_file(FILE)?;
    let json = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "cannot read {}: {error}; run this program from its bundle",
            path.display()
        )
    })?;
    Metadata::parse(&json)
}

#[cfg(test)]
mod tests;
