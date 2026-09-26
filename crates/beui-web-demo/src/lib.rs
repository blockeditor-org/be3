#![cfg(target_arch = "wasm32")]
#![allow(dead_code)]

include!(env!("BEUI_DEMO"));

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_dom(root_id: String, icons_font: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    let mut options = beui::RunOptions::new("beui demo");
    options.icons_font = Some(icons_font);
    beui::run_dom(&root_id, options, DemoApp::new())
        .await
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui::accessibility_tree()
}
