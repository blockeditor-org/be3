use std::sync::{Mutex, PoisonError};

use super::{Apps, Read};

/// What `load` read, kept for the host's worker (`fetched`).
static READ: Mutex<Option<Read>> = Mutex::new(None);

/// Reads the app list and installs the apps it names in this memory.
pub async fn load() {
    adopt(super::fetch_all(|path| be_host_base::platform::http::get(path, Vec::new())).await);
}

/// Installs the apps another context read (`fetched`), without reading them again.
pub(crate) fn adopt(read: Read) {
    super::install(super::settle(&read));
    *READ.lock().unwrap_or_else(PoisonError::into_inner) = Some(read);
}

/// What `load` or `adopt` read in this memory, for a stack the page starts in its host's worker.
pub(crate) fn fetched() -> Option<Read> {
    READ.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub(super) fn scan() -> Apps {
    Apps::default()
}

pub(super) fn fetch(path: String, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    be_host_base::platform::http::fetch(path, Vec::new(), Default::default(), deliver);
}
