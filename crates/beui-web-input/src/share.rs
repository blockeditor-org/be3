//! The browser's share sheet, `navigator.share`, which both web cores hand a clip to. Bound
//! here with wasm-bindgen alone, since web-sys's `ShareData` is behind a feature.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    type SharingNavigator;

    #[wasm_bindgen(method, getter, js_name = share)]
    fn share_function(this: &SharingNavigator) -> JsValue;

    #[wasm_bindgen(method, catch, js_name = share)]
    fn share(this: &SharingNavigator, data: &ShareData) -> Result<SharePromise, JsValue>;

    #[wasm_bindgen(js_name = Object)]
    type ShareData;

    #[wasm_bindgen(constructor, js_class = Object)]
    fn new() -> ShareData;

    #[wasm_bindgen(method, setter)]
    fn set_text(this: &ShareData, text: &str);

    #[wasm_bindgen(method, setter)]
    fn set_url(this: &ShareData, url: &str);

    #[wasm_bindgen(js_name = Promise)]
    type SharePromise;

    #[wasm_bindgen(method, js_name = catch)]
    fn catch(this: &SharePromise, handler: &JsValue) -> SharePromise;
}

/// The navigator, where it has `share`: only in a secure context of a browser with a sheet.
fn navigator() -> Option<SharingNavigator> {
    let navigator: SharingNavigator = web_sys::window()?.navigator().unchecked_into();
    navigator
        .share_function()
        .is_function()
        .then_some(navigator)
}

/// Whether the browser has a share sheet.
pub fn has_share_sheet() -> bool {
    navigator().is_some()
}

/// Hands `clip`'s plain text to the share sheet, as its `url` when it is one link. The browser
/// shares only within the user gesture a press makes.
pub fn share(clip: &beui_types::Clip) {
    let Some(navigator) = navigator() else {
        return;
    };
    let Some(text) = clip.plain_text() else {
        return;
    };
    let data = ShareData::new();
    match beui_types::clip::shared_link(&text) {
        Some(link) => data.set_url(link),
        None => data.set_text(&text),
    }
    // A share outside a gesture throws, and one the person cancels rejects.
    if let Ok(shared) = navigator.share(&data) {
        let _ = shared.catch(&Closure::once_into_js(|_: JsValue| {}));
    }
}
