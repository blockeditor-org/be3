#[cfg(target_os = "android")]
mod android;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

use super::Deliver;

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

pub(crate) type SaveResult = Result<bool, String>;

pub(crate) fn save_file(file: SavedFile, deliver: impl FnOnce(SaveResult) + Send + 'static) {
    save(file, Deliver::new(Ok(false), deliver));
}
