use be_server_portal::{PAGE_ID, Page, ROOT_ID};
use beui::reactive::view;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::PortalPage;

#[wasm_bindgen]
pub fn start(page: String) -> Result<(), JsValue> {
    be_wasm_threads::initialize_page();
    let page = Page::from_json(&page)
        .map_err(|error| JsValue::from_str(&format!("{PAGE_ID}: {error}")))?;
    beui::html::hydrate(ROOT_ID, move || {
        view! {
            <PortalPage page />
        }
    })
    .map_err(|error| JsValue::from_str(&error))
}
