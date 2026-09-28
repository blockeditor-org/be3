#[cfg(target_os = "android")]
mod android;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

use std::sync::mpsc::{Receiver, TryRecvError};

#[cfg(target_os = "android")]
use android::save;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use desktop::save;
#[cfg(target_arch = "wasm32")]
use web::save;

pub(crate) struct SavedFile {
    pub(crate) name: String,
    #[cfg_attr(
        all(not(target_os = "android"), not(target_arch = "wasm32")),
        allow(dead_code)
    )]
    pub(crate) mime_type: String,
    pub(crate) data: Vec<u8>,
}

type SaveResult = Result<bool, String>;

#[derive(Default)]
pub(crate) struct FileSaver {
    pending: Option<Receiver<SaveResult>>,
}

impl FileSaver {
    pub(crate) fn save(&mut self, file: SavedFile) {
        self.pending = Some(save(file));
    }

    pub(crate) fn poll(&mut self) -> Option<SaveResult> {
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Ok(false),
        };
        self.pending = None;
        Some(result)
    }
}
