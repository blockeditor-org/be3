#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use beui::reactive::Set;
use paint_review::{Fetch, PaintReviewApp};
use paint_review_manifest::Manifest;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
pub async fn run_review(root_id: String) -> Result<(), JsValue> {
    wasi_threads::initialize_main_thread();
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no browser window"))?;
    let page = web_sys::Url::new(&window.location().href()?)?;
    let parameters = page.search_params();
    let changes = parameters.get("changes");
    let painting = parameters.get("painting");
    let root = web_sys::Url::new_with_base("../../", &page.href())?.href();

    let images = root.clone();
    let app = PaintReviewApp::new(painting, move |request| {
        let url = format!("{images}{}.png", request.hash());
        let hash = request.hash().to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            let image = match is_hash(&hash) {
                true => fetch(&url).await.and_then(|bytes| decode(&bytes)),
                false => Err(format!("{hash} is not a hash")),
            };
            request.deliver(image);
        });
    });
    let manifest = app.manifest();
    wasm_bindgen_futures::spawn_local(async move {
        let loaded = match changes {
            Some(hash) if is_hash(&hash) => {
                let url = format!("{root}{hash}.json");
                fetch(&url).await.and_then(|bytes| {
                    serde_json::from_slice::<Manifest>(&bytes)
                        .map_err(|error| format!("{url} is not a list of paintings: {error}"))
                })
            }
            Some(hash) => Err(format!("{hash} is not a hash")),
            None => Err(
                "the page's address names no changes: it needs ?changes= and the hash of a list \
                 of paintings"
                    .to_owned(),
            ),
        };
        manifest.set(match loaded {
            Ok(loaded) => Fetch::Ready(Rc::new(loaded)),
            Err(error) => Fetch::Failed(error),
        });
    });

    let options = beui_host::native::RunOptions::default();
    beui_host::native::run_web(&root_id, options, Vec::new(), app)
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn accessibility_tree() -> Option<String> {
    beui_host::native::accessibility_tree()
}

fn is_hash(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no browser window")?;
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|error| format!("{url} could not be fetched: {}", describe(&error)))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|error| format!("{url} answered with no response: {}", describe(&error)))?;
    if !response.ok() {
        return Err(format!("{url} answered HTTP {}", response.status()));
    }
    let buffer = response
        .array_buffer()
        .map_err(|error| format!("{url} could not be read: {}", describe(&error)))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|error| format!("{url} could not be read: {}", describe(&error)))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn decode(bytes: &[u8]) -> Result<beui::Image, String> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|error| format!("not a PNG: {error}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok(beui::Image::from_rgba(width, height, image.into_raw()))
}

fn describe(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{error:?}"))
}
