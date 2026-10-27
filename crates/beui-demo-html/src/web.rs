use beui::reactive::view;
use beui_demo::HtmlDemo;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    wasi_threads::initialize_main_thread();
    beui::html::run("app", || {
        view! {
            <HtmlDemo />
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
