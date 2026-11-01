use std::fs;
use std::path::PathBuf;

use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_types::{FileSave, SavedFile};

pub fn open(request: FileSaveRequest, answer: FileSaveAnswer) {
    let unanswered = answer.clone();
    let spawned = std::thread::Builder::new()
        .name("beui-file-saver".into())
        .spawn(move || {
            answer.send(match save(&request.file) {
                Ok(true) => FileSave::Saved,
                Ok(false) => FileSave::Cancelled,
                Err(error) => FileSave::Failed(error),
            });
        });
    if let Err(error) = spawned {
        unanswered.send(FileSave::Failed(format!(
            "Could not open a save dialog: {error}"
        )));
    }
}

fn save(file: &SavedFile) -> Result<bool, String> {
    let Some(path) = choose(&file.name)? else {
        return Ok(false);
    };
    fs::write(&path, &file.data)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(true)
}

#[cfg(not(target_os = "linux"))]
fn choose(name: &str) -> Result<Option<PathBuf>, String> {
    let dialog = rfd::AsyncFileDialog::new().set_file_name(name).save_file();
    Ok(pollster::block_on(dialog).map(|file| file.path().to_path_buf()))
}

#[cfg(target_os = "linux")]
fn choose(name: &str) -> Result<Option<PathBuf>, String> {
    let portal = match pollster::block_on(portal(name)) {
        Ok(path) => return Ok(path),
        Err(error) => error,
    };
    zenity(name).map_err(|zenity| {
        eprintln!("No save dialog: the portal failed with \"{portal}\", zenity with \"{zenity}\"");
        "Saving a file needs xdg-desktop-portal with a file chooser backend, such as xdg-desktop-portal-gtk, or zenity. Install either one.".to_owned()
    })
}

#[cfg(target_os = "linux")]
async fn portal(name: &str) -> Result<Option<PathBuf>, ashpd::Error> {
    use ashpd::desktop::{ResponseError, file_chooser::SaveFileRequest};

    let request = SaveFileRequest::default()
        .modal(true)
        .current_name(name)
        .send()
        .await?;
    match request.response() {
        Ok(files) => Ok(files.uris().first().and_then(|uri| uri.to_file_path().ok())),
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "linux")]
fn zenity(name: &str) -> Result<Option<PathBuf>, String> {
    let output = std::process::Command::new("zenity")
        .args(["--no-markup", "--file-selection", "--save"])
        .arg(format!("--filename={name}"))
        .output()
        .map_err(|error| error.to_string())?;
    let chosen = String::from_utf8_lossy(&output.stdout);
    let chosen = chosen.trim_end_matches('\n');
    Ok((output.status.success() && !chosen.is_empty()).then(|| PathBuf::from(chosen)))
}
