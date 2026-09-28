use std::{fs, sync::mpsc::Receiver};

use super::{SaveResult, SavedFile};

pub(super) fn save(file: SavedFile) -> Receiver<SaveResult> {
    let (sender, receiver) = crate::host::waking_channel();
    let _ = sender.send(write(file));
    receiver
}

fn write(file: SavedFile) -> SaveResult {
    let Some(path) = rfd::FileDialog::new().set_file_name(&file.name).save_file() else {
        return Ok(false);
    };
    fs::write(&path, &file.data)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    Ok(true)
}
