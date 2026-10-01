use std::{fs, path::PathBuf};

use super::{Deliver, FileFilter, PickResult, PickedFile};

pub(super) fn open(filter: &FileFilter, deliver: Deliver<PickResult>) {
    let filter = filter.clone();
    std::thread::spawn(move || deliver.send(pick(&filter)));
}

fn pick(filter: &FileFilter) -> PickResult {
    let Some(path) = choose(filter)? else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
        .to_owned();
    let data =
        fs::read(&path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    Ok(Some(PickedFile { name, data }))
}

#[cfg(not(target_os = "linux"))]
fn choose(filter: &FileFilter) -> Result<Option<PathBuf>, String> {
    let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
    let dialog = rfd::AsyncFileDialog::new()
        .add_filter(&filter.name, &extensions)
        .pick_file();
    Ok(pollster::block_on(dialog).map(|file| file.path().to_path_buf()))
}

#[cfg(target_os = "linux")]
fn choose(filter: &FileFilter) -> Result<Option<PathBuf>, String> {
    let portal = match pollster::block_on(portal(filter)) {
        Ok(path) => return Ok(path),
        Err(error) => error,
    };
    zenity(filter).map_err(|zenity| {
        eprintln!("No file dialog: the portal failed with \"{portal}\", zenity with \"{zenity}\"");
        "Choosing a file needs xdg-desktop-portal with a file chooser backend, such as xdg-desktop-portal-gtk, or zenity. Install either one.".to_owned()
    })
}

#[cfg(target_os = "linux")]
async fn portal(filter: &FileFilter) -> Result<Option<PathBuf>, ashpd::Error> {
    use ashpd::desktop::{
        ResponseError,
        file_chooser::{FileFilter as PortalFilter, OpenFileRequest},
    };

    let accepted = filter
        .extensions
        .iter()
        .fold(PortalFilter::new(&filter.name), |accepted, extension| {
            accepted.glob(&glob(extension))
        });
    let request = OpenFileRequest::default()
        .modal(true)
        .multiple(false)
        .filters((!filter.extensions.is_empty()).then_some(accepted))
        .send()
        .await?;
    match request.response() {
        Ok(files) => Ok(files.uris().first().and_then(|uri| uri.to_file_path().ok())),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "linux")]
fn zenity(filter: &FileFilter) -> Result<Option<PathBuf>, String> {
    let mut command = std::process::Command::new("zenity");
    command.args(["--no-markup", "--file-selection"]);
    if !filter.extensions.is_empty() {
        let globs: Vec<String> = filter.extensions.iter().map(|end| glob(end)).collect();
        command.arg("--file-filter");
        command.arg(format!("{} | {}", filter.name, globs.join(" ")));
    }
    let output = command.output().map_err(|error| error.to_string())?;
    let chosen = String::from_utf8_lossy(&output.stdout);
    let chosen = chosen.trim_end_matches('\n');
    Ok((output.status.success() && !chosen.is_empty()).then(|| PathBuf::from(chosen)))
}

#[cfg(target_os = "linux")]
fn glob(extension: &str) -> String {
    match extension {
        "" | "*" => "*".to_owned(),
        extension => format!("*.{extension}"),
    }
}
