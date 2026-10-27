use beui_types::files::{FileFilter, FilePick, FileSave, SavedFile};

pub fn pick_file(filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
    #[cfg(target_arch = "wasm32")]
    web::pick_file(filter, Box::new(picked));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = filter;
        picked(Err("this page cannot pick files".to_owned()));
    }
}

pub fn save_file(file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
    #[cfg(target_arch = "wasm32")]
    saved(web::save_file(file));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = file;
        saved(FileSave::Failed("this page cannot save files".to_owned()));
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use beui_types::files::{FileFilter, FilePick, FileSave, PickedFile, SavedFile};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    type Picked = Rc<RefCell<Option<Box<dyn FnOnce(FilePick)>>>>;

    fn answer(picked: &Picked, pick: FilePick) {
        if let Some(picked) = picked.borrow_mut().take() {
            picked(pick);
            reactive::flush();
        }
    }

    pub(super) fn pick_file(filter: FileFilter, picked: Box<dyn FnOnce(FilePick)>) {
        let picked: Picked = Rc::new(RefCell::new(Some(picked)));
        let Some(input) = crate::dom::document()
            .and_then(|document| document.create_element("input").ok())
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            answer(&picked, Err("no document to pick a file from".to_owned()));
            return;
        };
        input.set_type("file");
        let accept: Vec<String> = filter
            .extensions
            .iter()
            .map(|extension| format!(".{extension}"))
            .chain(filter.mime_types.iter().cloned())
            .collect();
        input.set_accept(&accept.join(","));
        let cancelled = Rc::clone(&picked);
        let on_cancel = Closure::once_into_js(move || answer(&cancelled, Ok(None)));
        input.set_oncancel(Some(on_cancel.unchecked_ref()));
        let chosen = input.clone();
        let on_change = Closure::once_into_js(move || {
            let Some(file) = chosen.files().and_then(|files| files.get(0)) else {
                answer(&picked, Ok(None));
                return;
            };
            let Ok(reader) = web_sys::FileReader::new() else {
                answer(&picked, Err("the browser cannot read files".to_owned()));
                return;
            };
            let name = file.name();
            let read = reader.clone();
            let loaded = Rc::clone(&picked);
            let on_load = Closure::once_into_js(move || {
                let pick = match read.result() {
                    Ok(result) => Ok(Some(PickedFile {
                        name,
                        data: js_sys::Uint8Array::new(&result).to_vec(),
                    })),
                    Err(_) => Err("the file could not be read".to_owned()),
                };
                answer(&loaded, pick);
            });
            reader.set_onloadend(Some(on_load.unchecked_ref()));
            if reader.read_as_array_buffer(&file).is_err() {
                answer(&picked, Err("the file could not be read".to_owned()));
            }
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
    }

    pub(super) fn save_file(file: SavedFile) -> FileSave {
        let failed = |why: &str| FileSave::Failed(why.to_owned());
        let Some(document) = crate::dom::document() else {
            return failed("no document to save from");
        };
        let bytes = js_sys::Uint8Array::from(file.data.as_slice());
        let parts = js_sys::Array::of1(&bytes);
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(&file.mime_type);
        let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        else {
            return failed("the browser cannot hold the file");
        };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
            return failed("the browser cannot hold the file");
        };
        let Some(anchor) = document
            .create_element("a")
            .ok()
            .and_then(|element| element.dyn_into::<web_sys::HtmlAnchorElement>().ok())
        else {
            return failed("no document to save from");
        };
        anchor.set_href(&url);
        anchor.set_download(&file.name);
        anchor.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        FileSave::Saved
    }
}
