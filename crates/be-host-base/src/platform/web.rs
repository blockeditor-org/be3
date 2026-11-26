use std::future::Future;

pub trait Request<T>: Future<Output = T> + 'static {}

impl<T, F: Future<Output = T> + 'static> Request<T> for F {}

pub fn spawn_request(request: impl Request<()>) {
    wasm_bindgen_futures::spawn_local(request);
}

/// Asks the browser to keep this origin's storage rather than evict it under pressure
/// (`navigator.storage.persist()`): true once it will, false when it refuses or cannot say.
/// Read through `Reflect`, as the page and a worker each have a `navigator` of their own.
pub async fn keep_storage() -> bool {
    use wasm_bindgen::JsCast;
    let get = |of: &wasm_bindgen::JsValue, name: &str| {
        js_sys::Reflect::get(of, &wasm_bindgen::JsValue::from_str(name))
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null())
    };
    let Some(storage) =
        get(&js_sys::global(), "navigator").and_then(|navigator| get(&navigator, "storage"))
    else {
        return false;
    };
    let Some(persist) = get(&storage, "persist")
        .and_then(|persist| persist.dyn_into::<js_sys::Function>().ok())
    else {
        return false;
    };
    let Ok(promise) = persist
        .call0(&storage)
        .and_then(|answer| answer.dyn_into::<js_sys::Promise>())
    else {
        return false;
    };
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .and_then(|kept| kept.as_bool())
        .unwrap_or(false)
}
