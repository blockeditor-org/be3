#![cfg(target_arch = "wasm32")]

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_dom(root_id: String, icons_font: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    let options = beui::RunOptions::new("beui demo");
    beui::run_dom(
        &root_id,
        Some(&icons_font),
        options,
        beui_demo::DemoApp::new(),
    )
    .await
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui::accessibility_tree()
}
