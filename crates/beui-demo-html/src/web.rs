use beui::datetime::Date;
use beui::reactive::view;
use beui_demo::DemoRoot;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn start() -> Result<(), JsValue> {
    be_wasm_threads::initialize_page();
    beui::html::run("app", || {
        view! {
            <DemoRoot today={Date::today()} />
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
