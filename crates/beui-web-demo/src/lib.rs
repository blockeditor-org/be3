#![cfg(target_arch = "wasm32")]

#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_demo(root_id: String) -> Result<(), wasm_bindgen::JsValue> {
    wasi_threads::initialize_main_thread();
    let options = beui::RunOptions::default();
    beui::run_web(
        &root_id,
        options,
        Vec::new(),
        beui_demo::DemoApp::new(),
    )
    .await
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui::accessibility_tree()
}
