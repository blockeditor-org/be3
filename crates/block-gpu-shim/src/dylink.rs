use wasm_bindgen::prelude::*;
use wasm_dylink::{Declared, Sdk};

fn refused(error: String) -> JsValue {
    js_sys::Error::new(&error).into()
}

#[wasm_bindgen]
pub fn sdk_identity(exports: Vec<String>) -> Result<Option<String>, JsValue> {
    wasm_dylink::sdk_identity(exports.iter().map(String::as_str))
        .transpose()
        .map_err(refused)
}

#[wasm_bindgen]
pub fn sdk_file_name(identity: &str) -> String {
    wasm_dylink::sdk_file_name(identity)
}

#[wasm_bindgen]
pub fn check_sdk(bytes: &[u8], identity: &str) -> Result<(), JsValue> {
    wasm_dylink::check_sdk(bytes, identity).map_err(|error| {
        refused(format!(
            "the plugin SDK {identity} could not be loaded: {error}"
        ))
    })
}

#[wasm_bindgen]
pub fn link_pages(exports: Vec<String>) -> Result<u32, JsValue> {
    Declared::of(exports.iter().map(String::as_str))
        .map(|declared| declared.pages())
        .map_err(refused)
}

#[wasm_bindgen]
pub fn link_plan(
    exports: Vec<String>,
    import_modules: Vec<String>,
    import_names: Vec<String>,
    sdk_functions: Vec<String>,
    sdk_globals: Vec<String>,
    sdk_addresses: Vec<i32>,
    placed: u32,
    table_size: u32,
) -> Result<String, JsValue> {
    let declared = Declared::of(exports.iter().map(String::as_str)).map_err(refused)?;
    let sdk = Sdk {
        functions: sdk_functions.into_iter().collect(),
        addresses: sdk_globals.into_iter().zip(sdk_addresses).collect(),
    };
    let imports = import_modules
        .iter()
        .map(String::as_str)
        .zip(import_names.iter().map(String::as_str));
    let layout = wasm_dylink::plan(declared, placed, u64::from(table_size), imports, &sdk)
        .map_err(refused)?;
    serde_json::to_string(&layout).map_err(|error| refused(error.to_string()))
}
