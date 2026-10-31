use wasm_bindgen::prelude::*;
use wasm_dylink::Described;

fn refused(error: String) -> JsValue {
    js_sys::Error::new(&error).into()
}

fn described(modules: &str) -> Result<Vec<Described>, JsValue> {
    serde_json::from_str(modules).map_err(|error| refused(error.to_string()))
}

#[wasm_bindgen]
pub fn libraries(exports: Vec<String>) -> Result<Vec<String>, JsValue> {
    wasm_dylink::libraries(exports.iter().map(String::as_str)).map_err(refused)
}

#[wasm_bindgen]
pub fn library_file_name(identity: &str) -> String {
    wasm_dylink::library_file_name(identity)
}

#[wasm_bindgen]
pub fn check_library(bytes: &[u8], identity: &str) -> Result<(), JsValue> {
    wasm_dylink::check_library(bytes, identity).map_err(|error| {
        refused(format!(
            "the library {identity} could not be loaded: {error}"
        ))
    })
}

#[wasm_bindgen]
pub fn link_pages(modules: &str) -> Result<u32, JsValue> {
    wasm_dylink::pages(&described(modules)?).map_err(refused)
}

#[wasm_bindgen]
pub fn link_plan(modules: &str, placed: u32, table_size: u32) -> Result<String, JsValue> {
    let plan =
        wasm_dylink::plan(&described(modules)?, placed, u64::from(table_size)).map_err(refused)?;
    serde_json::to_string(&plan).map_err(|error| refused(error.to_string()))
}
