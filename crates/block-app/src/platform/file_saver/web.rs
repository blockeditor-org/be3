use std::sync::mpsc::Receiver;

use wasm_bindgen::JsCast;

use super::{SaveResult, SavedFile};

pub(super) fn save(file: SavedFile) -> Receiver<SaveResult> {
    let (sender, receiver) = crate::host::waking_channel();
    let _ = sender.send(download(&file).map(|()| true));
    receiver
}

fn download(file: &SavedFile) -> Result<(), String> {
    let document = web_sys::window()
        .ok_or("no browser window is available")?
        .document()
        .ok_or("no browser document is available")?;
    let body = document.body().ok_or("the page has no body to attach to")?;
    let bytes = js_sys::Uint8Array::from(file.data.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());
    let properties = web_sys::BlobPropertyBag::new();
    properties.set_type(&file.mime_type);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &properties)
        .map_err(|_| format!("Could not prepare {}", file.name))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| format!("Could not prepare {}", file.name))?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|_| format!("Could not download {}", file.name))?
        .dyn_into()
        .map_err(|_| format!("Could not download {}", file.name))?;
    anchor.set_href(&url);
    anchor.set_download(&file.name);
    let _ = anchor.style().set_property("display", "none");
    let attached = body.append_child(&anchor);
    anchor.click();
    if attached.is_ok() {
        anchor.remove();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}
