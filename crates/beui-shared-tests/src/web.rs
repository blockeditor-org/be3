use beui::reactive::view;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Fixture;

mod live;

#[wasm_bindgen]
pub fn start(fixture: String) -> Result<(), JsValue> {
    wasi_threads::initialize_main_thread();
    beui::html::run("app", move || {
        view! {
            <Fixture name={fixture} />
        }
    })
    .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub async fn automate(words: Vec<String>) -> Result<String, JsValue> {
    beui::html::automate(words)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

#[wasm_bindgen]
pub fn checks() -> String {
    wasi_threads::initialize_main_thread();
    live::run()
}
