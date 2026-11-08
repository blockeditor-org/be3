use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub fn fetch(
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

pub async fn run(url: String, headers: Vec<(&'static str, String)>) -> Result<Vec<u8>, String> {
    fetching(url, headers, None).await
}

async fn fetching(
    url: String,
    headers: Vec<(&'static str, String)>,
    signal: Option<web_sys::AbortSignal>,
) -> Result<Vec<u8>, String> {
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
    let global = js_sys::global();
    let fetched = match global.dyn_ref::<web_sys::Window>() {
        Some(window) => window.fetch_with_request(&request),
        None => global
            .unchecked_into::<web_sys::WorkerGlobalScope>()
            .fetch_with_request(&request),
    };
    let response = JsFuture::from(fetched)
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

/// How far `send`'s reading may run ahead, and whether it was aborted. Clones share it.
#[derive(Clone)]
pub struct Flow(std::rc::Rc<std::cell::RefCell<Room>>);

struct Room {
    chunks: usize,
    aborted: bool,
    waiting: Option<std::task::Waker>,
    controller: Option<web_sys::AbortController>,
}

impl Default for Flow {
    fn default() -> Self {
        Self(std::rc::Rc::new(std::cell::RefCell::new(Room {
            chunks: super::CHUNKS_AHEAD,
            aborted: false,
            waiting: None,
            controller: None,
        })))
    }
}

impl Flow {
    /// Stops the request: nothing more is delivered, and the browser drops it.
    pub fn abort(&self) {
        let (controller, waiting) = {
            let mut room = self.0.borrow_mut();
            room.aborted = true;
            (room.controller.take(), room.waiting.take())
        };
        if let Some(controller) = controller {
            controller.abort();
        }
        if let Some(waiting) = waiting {
            waiting.wake();
        }
    }

    pub fn aborted(&self) -> bool {
        self.0.borrow().aborted
    }

    /// The caller passed one delivered chunk on, so one more may be read.
    pub fn release(&self) {
        let waiting = {
            let mut room = self.0.borrow_mut();
            room.chunks += 1;
            room.waiting.take()
        };
        if let Some(waiting) = waiting {
            waiting.wake();
        }
    }

    /// Waits until another chunk may be read and takes its room; false once aborted.
    async fn take(&self) -> bool {
        std::future::poll_fn(|context| {
            let mut room = self.0.borrow_mut();
            if room.aborted {
                return std::task::Poll::Ready(false);
            }
            if room.chunks == 0 {
                room.waiting = Some(context.waker().clone());
                return std::task::Poll::Pending;
            }
            room.chunks -= 1;
            std::task::Poll::Ready(true)
        })
        .await
    }
}

/// Sends `request` with the browser's `fetch` and delivers its response as it arrives
/// (`Event`), a chunk of the body at a time and no more than `CHUNKS_AHEAD` chunks ahead of
/// `flow.release`, so the body has no limit. CORS applies as it does to the page.
pub fn send(
    request: super::Request,
    flow: Flow,
    mut deliver: impl FnMut(super::Event) + Send + 'static,
) {
    use super::Event;
    let controller = web_sys::AbortController::new().ok();
    let signal = controller.as_ref().map(web_sys::AbortController::signal);
    flow.0.borrow_mut().controller = controller;
    wasm_bindgen_futures::spawn_local(async move {
        let mut deliver = |event| {
            if !flow.aborted() {
                deliver(event);
            }
        };
        let response = match exchange(&request, signal).await {
            Ok(response) => response,
            Err(error) => return deliver(Event::Failed(error)),
        };
        let mut headers = Vec::new();
        if let Ok(Some(entries)) = js_sys::try_iter(&response.headers()) {
            for entry in entries.flatten() {
                let entry: js_sys::Array = entry.unchecked_into();
                if let (Some(name), Some(value)) =
                    (entry.get(0).as_string(), entry.get(1).as_string())
                {
                    headers.push((name, value));
                }
            }
        }
        deliver(Event::Head(super::Head {
            status: response.status(),
            headers,
        }));
        let Some(body) = response.body() else {
            return deliver(Event::Done);
        };
        let reader: web_sys::ReadableStreamDefaultReader = body.get_reader().unchecked_into();
        loop {
            if !flow.take().await {
                let _ = reader.cancel();
                return;
            }
            let read = match JsFuture::from(reader.read()).await {
                Ok(read) => read,
                Err(error) => return deliver(Event::Failed(describe(&error))),
            };
            let done = js_sys::Reflect::get(&read, &JsValue::from_str("done"))
                .ok()
                .and_then(|done| done.as_bool())
                .unwrap_or(true);
            if done {
                return deliver(Event::Done);
            }
            match js_sys::Reflect::get(&read, &JsValue::from_str("value")) {
                Ok(value) => {
                    let value: js_sys::Uint8Array = value.unchecked_into();
                    deliver(Event::Chunk(value.to_vec()));
                }
                Err(error) => return deliver(Event::Failed(describe(&error))),
            }
        }
    });
}

async fn exchange(
    request: &super::Request,
    signal: Option<web_sys::AbortSignal>,
) -> Result<web_sys::Response, String> {
    let headers = web_sys::Headers::new()
        .map_err(|error| format!("failed to build request headers: {}", describe(&error)))?;
    for (name, value) in &request.headers {
        headers.append(name, value).map_err(|error| {
            format!(
                "failed to set the {name} request header: {}",
                describe(&error)
            )
        })?;
    }
    let options = web_sys::RequestInit::new();
    options.set_method(&request.method);
    options.set_headers(&headers);
    options.set_mode(web_sys::RequestMode::Cors);
    options.set_credentials(web_sys::RequestCredentials::Omit);
    if !request.body.is_empty() {
        options.set_body(&js_sys::Uint8Array::from(request.body.as_slice()).into());
    }
    if let Some(signal) = &signal {
        options.set_signal(Some(signal));
    }
    let url = &request.url;
    let request = web_sys::Request::new_with_str_and_init(url, &options)
        .map_err(|error| format!("invalid URL {url}: {}", describe(&error)))?;
    let global = js_sys::global();
    let fetched = match global.dyn_ref::<web_sys::Window>() {
        Some(window) => window.fetch_with_request(&request),
        None => global
            .unchecked_into::<web_sys::WorkerGlobalScope>()
            .fetch_with_request(&request),
    };
    let response = JsFuture::from(fetched)
        .await
        .map_err(|error| format!("request failed: {}", describe(&error)))?;
    response
        .dyn_into()
        .map_err(|error| format!("unexpected fetch result: {}", describe(&error)))
}
