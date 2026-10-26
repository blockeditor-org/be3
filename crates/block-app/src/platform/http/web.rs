use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub(crate) fn fetch(
    url: String,
    headers: Vec<(&'static str, String)>,
    aborted: super::Aborted,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    let controller = web_sys::AbortController::new().ok();
    let signal = controller.as_ref().map(web_sys::AbortController::signal);
    *aborted.controller.borrow_mut() = controller;
    wasm_bindgen_futures::spawn_local(async move {
        let fetched = fetching(url, headers, signal).await;
        aborted.controller.borrow_mut().take();
        if !aborted.aborted() {
            deliver(fetched);
        }
    });
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

pub(crate) async fn run(
    url: String,
    headers: Vec<(&'static str, String)>,
) -> Result<Vec<u8>, String> {
    fetching(url, headers, None).await
}

async fn fetching(
    url: String,
    headers: Vec<(&'static str, String)>,
    signal: Option<web_sys::AbortSignal>,
) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no browser window is available")?;

    let js_headers = web_sys::Headers::new()
        .map_err(|error| format!("failed to build request headers: {}", describe(&error)))?;
    for (key, value) in &headers {
        js_headers.append(key, value).map_err(|error| {
            format!(
                "failed to set the {key} request header: {}",
                describe(&error)
            )
        })?;
    }

    let options = web_sys::RequestInit::new();
    options.set_method("GET");
    options.set_headers(&js_headers);
    if let Some(signal) = &signal {
        options.set_signal(Some(signal));
    }

    let request = web_sys::Request::new_with_str_and_init(&url, &options)
        .map_err(|error| format!("invalid URL {url}: {}", describe(&error)))?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|error| format!("request failed: {}", describe(&error)))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|error| format!("unexpected fetch result: {}", describe(&error)))?;

    let status = response.status();
    let buffer = response
        .array_buffer()
        .map_err(|error| format!("unreadable response: {}", describe(&error)))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|error| format!("unreadable response: {}", describe(&error)))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    if bytes.len() > super::MAX_BODY_BYTES {
        return Err(format!(
            "{url} sent more than {} bytes",
            super::MAX_BODY_BYTES
        ));
    }

    if !(200..300).contains(&status) {
        return Err(format!(
            "HTTP {status}: {}",
            String::from_utf8_lossy(&bytes)
        ));
    }
    Ok(bytes)
}
