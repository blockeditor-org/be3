#![cfg(target_arch = "wasm32")]

use beui::datetime::Date;
use beui::reactive::view;
use beui_demo::DemoRoot;

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_demo(root_id: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    let options = beui::native::RunOptions::default();
    let demo = beui_host::native::build(|| {
        view! {
            <DemoRoot today={Date::today()} />
        }
    });
    beui_host::native::run_web(&root_id, options, Vec::new(), demo)
        .await
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui_host::native::accessibility_tree()
}
