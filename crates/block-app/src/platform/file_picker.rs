#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub(super) use android::main_activity;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

use super::Deliver;

#[cfg(target_os = "android")]
use android::open;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use desktop::open;
#[cfg(target_arch = "wasm32")]
use web::open;

#[derive(Clone, Default)]
pub(crate) struct FileFilter {
    #[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
    pub(crate) name: String,
    pub(crate) default_file_name: String,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) extensions: Vec<String>,
    #[cfg_attr(
        all(not(target_os = "android"), not(target_arch = "wasm32")),
        allow(dead_code)
    )]
    pub(crate) mime_types: Vec<String>,
}

pub(crate) struct PickedFile {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
}

pub(crate) type PickResult = Result<Option<PickedFile>, String>;

pub(crate) fn pick_file(filter: &FileFilter, deliver: impl FnOnce(PickResult) + Send + 'static) {
    let default_file_name = filter.default_file_name.clone();
    open(
        filter,
        Deliver::new(Ok(None), move |result: PickResult| {
            deliver(result.map(|file| {
                file.map(|mut file| {
                    if file.name.is_empty() {
                        file.name = default_file_name;
                    }
                    file
                })
            }));
        }),
    );
}
