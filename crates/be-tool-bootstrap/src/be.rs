use std::process::{Command, ExitCode};

use crate::download::{download_verified, is_executable, make_executable};
use crate::pins::Pins;
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
    let pins = Pins::read()?;
    let be = ensure(&pins)?;
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

pub fn ensure(pins: &Pins) -> Result<std::path::PathBuf> {
    let release = pins.get("be_release").map_err(|_| {
        "No be release is pinned yet (be_release in scripts/internal/pins).".to_owned()
    })?;
    let destination = repository().join(format!("target/tools/be-{release}/be"));
    let directory = destination.parent().unwrap_or(repository()).to_path_buf();
    let metadata_file = pins.get("be_metadata_file")?;
    if is_executable(&destination) && directory.join(metadata_file).is_file() {
        return Ok(destination);
    }
    let triple = triple();
    let sha256 = pins.sha256("be", triple)?;
    let url = format!("{}/{release}/be-{triple}/be", pins.get("be_mirror")?);
    eprintln!("Downloading be {release} from {url}...");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    let partial = partial_path(&destination);
    download_verified(&url, &partial, sha256, &[])?;
    make_executable(&partial)?;
    let metadata = if metadata_file == "build-id" {
        format!("{release}\n")
    } else {
        format!("{{\"id\":\"{release}\",\"app_id\":\"Block\"}}\n")
    };
    std::fs::write(directory.join(metadata_file), metadata)
        .map_err(|error| format!("could not write {}: {error}", directory.display()))?;
    std::fs::rename(&partial, &destination)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))?;
    eprintln!("Installed be {release} at {}", destination.display());
    Ok(destination)
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
