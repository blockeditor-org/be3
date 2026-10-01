use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

use super::{Deliver, FileFilter, PickResult, PickedFile};

type Slot = Rc<RefCell<Option<Deliver<PickResult>>>>;

pub(super) fn open(filter: &FileFilter, deliver: Deliver<PickResult>) {
    let slot: Slot = Rc::new(RefCell::new(Some(deliver)));
    if let Err(error) = show(filter, &slot)
        && let Some(deliver) = slot.borrow_mut().take()
    {
        deliver.send(Err(error));
    }
}

fn show(filter: &FileFilter, slot: &Slot) -> Result<(), String> {
    let document = web_sys::window()
        .ok_or("no browser window is available")?
        .document()
        .ok_or("no browser document is available")?;
    let body = document.body().ok_or("the page has no body to attach to")?;
    let input: HtmlInputElement = document
        .create_element("input")
        .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?
        .dyn_into()
        .map_err(|_| "could not open a file picker".to_owned())?;
    input.set_type("file");
    input.set_accept(&accept(filter));
    let _ = input.style().set_property("display", "none");
    body.append_child(&input)
        .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?;

    let chosen = slot.clone();
    let chosen_input = input.clone();
    let on_change = Closure::<dyn FnMut()>::new(move || {
        let Some(deliver) = chosen.borrow_mut().take() else {
            return;
        };
        chosen_input.remove();
        let Some(file) = chosen_input.files().and_then(|files| files.get(0)) else {
            deliver.send(Ok(None));
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            deliver.send(read(file).await);
        });
    });
    let cancelled = slot.clone();
    let cancelled_input = input.clone();
    let on_cancel = Closure::<dyn FnMut()>::new(move || {
        if let Some(deliver) = cancelled.borrow_mut().take() {
            cancelled_input.remove();
            deliver.send(Ok(None));
        }
    });
    for (event, listener) in [("change", &on_change), ("cancel", &on_cancel)] {
        input
            .add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())
            .map_err(|error| format!("could not open a file picker: {}", describe(&error)))?;
    }

    on_change.forget();
    on_cancel.forget();

    input.click();
    Ok(())
}

fn accept(filter: &FileFilter) -> String {
    let mime_types = filter.mime_types.iter().cloned();
    let extensions = filter.extensions.iter().map(|end| format!(".{end}"));
    mime_types.chain(extensions).collect::<Vec<_>>().join(",")
}

async fn read(file: web_sys::File) -> PickResult {
    let name = file.name();
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|error| format!("Could not read {name}: {}", describe(&error)))?;
    let data = js_sys::Uint8Array::new(&buffer).to_vec();
    Ok(Some(PickedFile { name, data }))
}

fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{error:?}"))
}
