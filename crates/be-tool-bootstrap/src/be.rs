use std::process::{Command, ExitCode};

use crate::download::{fetch_https, is_executable, make_executable};
use crate::platform::triple;
use crate::{Result, buck, partial_path, repository};

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let credentials = repository().join("external/.tracker-credentials");
    let bundle = match std::env::var("BE3_AGENT_CREDENTIALS") {
        Ok(bundle) if !bundle.is_empty() => bundle,
        _ => {
            fetch_credentials(&credentials);
            std::fs::read_to_string(&credentials).map_err(|_| {
                format!(
                    "Set BE3_AGENT_CREDENTIALS or save the copied agent JSON in {}.",
                    credentials.display()
                )
            })?
        }
    };
    let be = ensure()?;
    let mut command = Command::new(be);
    command.args(arguments);
    for name in [
        "BE3_SERVER",
        "BE3_ORGANIZATION",
        "BE3_WORKSPACE",
        "BE3_TOKEN",
        "BE3_RECOVERY_PHRASE",
    ] {
        command.env_remove(name);
    }
    command.env("BE3_AGENT_CREDENTIALS", bundle);
    if let Some(key) = crate::server::gateway().stored() {
        command.env("BE3_AUTHORIZATION", key);
    }
    buck::exec(command)
}

/// Where deployments put `be` (guides/hosting.md, Deploying): `be/release` names the build the
/// server runs, and `data/<build>/be-<triple>/` holds its `be` and `build-info.json`. Nothing is
/// pinned: the server refuses a `be` from another build, so this always asks which one is live,
/// and trusts the CDN for the bytes.
const MIRROR: &str = "https://be3.pfg.pw";

pub fn ensure() -> Result<std::path::PathBuf> {
    let release = live_release()?;
    let destination = repository().join(format!("target/tools/be-{release}/be"));
    let directory = destination.parent().unwrap_or(repository()).to_path_buf();
    let metadata = directory.join("build-info.json");
    if is_executable(&destination) && metadata.is_file() {
        return Ok(destination);
    }
    let triple = triple();
    let base = format!("{MIRROR}/data/{release}/be-{triple}");
    eprintln!("Downloading be {release} from {base}/be...");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    let partial_metadata = partial_path(&metadata);
    fetch(&format!("{base}/build-info.json"), &partial_metadata)?;
    std::fs::rename(&partial_metadata, &metadata)
        .map_err(|error| format!("could not write {}: {error}", metadata.display()))?;
    let partial = partial_path(&destination);
    fetch(&format!("{base}/be"), &partial)?;
    make_executable(&partial)?;
    std::fs::rename(&partial, &destination)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))?;
    eprintln!("Installed be {release} at {}", destination.display());
    Ok(destination)
}

/// The build `be/release` names, a Git tree hash.
fn live_release() -> Result<String> {
    let path = repository().join("target/tools/be-release");
    std::fs::create_dir_all(path.parent().unwrap_or(repository()))
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    let partial = partial_path(&path);
    fetch(&format!("{MIRROR}/be/release"), &partial)?;
    let read = std::fs::read_to_string(&partial);
    let _ = std::fs::remove_file(&partial);
    let release = read
        .map_err(|error| format!("could not read {}: {error}", partial.display()))?
        .trim()
        .to_owned();
    if release.is_empty()
        || !release
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!(
            "{MIRROR}/be/release does not name a build: '{release}'"
        ));
    }
    Ok(release)
}

fn fetch(url: &str, destination: &std::path::Path) -> Result<()> {
    let _ = std::fs::remove_file(destination);
    if fetch_https(destination, url)? {
        return Ok(());
    }
    let _ = std::fs::remove_file(destination);
    Err(format!(
        "Could not download {url}. Has be been deployed for this platform ({})?",
        triple()
    ))
}

fn fetch_credentials(credentials: &std::path::Path) {
    if std::fs::metadata(credentials).is_ok_and(|metadata| metadata.len() > 0) {
        return;
    }
    let partial = partial_path(credentials);
    let _ = std::fs::create_dir_all(credentials.parent().unwrap_or(repository()));
    let fetched = Command::new("curl")
        .args(["-sSf", "--max-time", "30", "-o"])
        .arg(&partial)
        .arg("https://api.be3.pfg.pw/authorized-storage/tracker-credentials")
        .status()
        .is_ok_and(|status| status.success());
    if fetched && restrict(&partial) && std::fs::rename(&partial, credentials).is_ok() {
        eprintln!(
            "Fetched the tracker's credentials into {}.",
            credentials.display()
        );
    } else {
        let _ = std::fs::remove_file(&partial);
    }
}

fn restrict(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).is_ok()
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
