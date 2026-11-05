use std::{cell::RefCell, future::Future, rc::Rc};

use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use super::Frame;

pub(crate) struct Writer(Rc<web_sys::WebSocket>);

pub(crate) struct Reader {
    socket: Rc<web_sys::WebSocket>,
    frames: mpsc::UnboundedReceiver<Frame>,
    _handlers: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

pub(crate) fn spawn(future: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

/// Posts `body` to `url` with `fetch`, sending the page's cookies for the server's origin (the
/// gateway's; guides/hosting.md), and answers the frames of the answer. No content type is set,
/// so the post needs no preflight.
pub(crate) async fn post_once(url: &str, body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_credentials(web_sys::RequestCredentials::Include);
    init.set_body(&js_sys::Uint8Array::from(body).into());
    let response = fetch(url, &init).await?;
    let buffer = wasm_bindgen_futures::JsFuture::from(
        response.array_buffer().map_err(|error| describe(&error))?,
    )
    .await
    .map_err(|error| describe(&error))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    if response.status() != 200 {
        return Err(format!(
            "the server answered {}: {}",
            response.status(),
            String::from_utf8_lossy(&bytes)
        ));
    }
    super::frames(&bytes)
}

async fn fetch(url: &str, init: &web_sys::RequestInit) -> Result<web_sys::Response, String> {
    let global = js_sys::global();
    let fetch = js_sys::Reflect::get(&global, &JsValue::from_str("fetch"))
        .ok()
        .and_then(|fetch| fetch.dyn_into::<js_sys::Function>().ok())
        .ok_or_else(|| "this page cannot fetch".to_owned())?;
    let promise = fetch
        .call2(&global, &JsValue::from_str(url), init)
        .map_err(|error| describe(&error))?;
    Ok(
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|error| format!("could not reach {url}: {}", describe(&error)))?
            .unchecked_into(),
    )
}

/// The longest refusal read to describe a failed request.
const REFUSAL_BYTES: usize = 16 * 1024;

/// Sends one `method` request to `url` with a CORS `fetch` that sends no cookies, and answers
/// the body of a 2xx answer, refusing one longer than `limit` as soon as it is. The headers the
/// browser sets itself are dropped from `headers`.
pub(crate) async fn request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &[u8],
    limit: usize,
) -> Result<Vec<u8>, crate::http::Error> {
    use crate::http::Error;
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_credentials(web_sys::RequestCredentials::Omit);
    let pairs = js_sys::Array::new();
    for (name, value) in headers {
        if ["host", "content-length", "connection", "transfer-encoding"]
            .iter()
            .any(|own| name.eq_ignore_ascii_case(own))
        {
            continue;
        }
        pairs.push(&js_sys::Array::of2(
            &JsValue::from_str(name),
            &JsValue::from_str(value),
        ));
    }
    init.set_headers(&pairs);
    if method != "GET" {
        init.set_body(&js_sys::Uint8Array::from(body).into());
    }
    let response = fetch(url, &init).await.map_err(Error::Failed)?;
    let status = response.status();
    if (200..300).contains(&status) {
        return read_body(&response, limit).await;
    }
    let message = match read_body(&response, REFUSAL_BYTES).await {
        Ok(body) => String::from_utf8_lossy(&body).trim().to_owned(),
        Err(_) => String::new(),
    };
    Err(Error::from_status(status, message))
}

/// Reads `response`'s body a chunk at a time, failing with `TooLarge` as soon as it is longer than
/// `limit`.
async fn read_body(
    response: &web_sys::Response,
    limit: usize,
) -> Result<Vec<u8>, crate::http::Error> {
    use crate::http::Error;
    let length = response
        .headers()
        .get("content-length")
        .ok()
        .flatten()
        .and_then(|length| length.trim().parse::<usize>().ok());
    if length.is_some_and(|length| length > limit) {
        return Err(Error::TooLarge { limit });
    }
    let Some(stream) = response.body() else {
        return Ok(Vec::new());
    };
    let reader: web_sys::ReadableStreamDefaultReader = stream.get_reader().unchecked_into();
    let mut body = Vec::new();
    loop {
        let chunk = wasm_bindgen_futures::JsFuture::from(reader.read())
            .await
            .map_err(|error| Error::Failed(describe(&error)))?;
        let done = js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))
            .ok()
            .and_then(|done| done.as_bool())
            .unwrap_or(true);
        if done {
            return Ok(body);
        }
        let value: js_sys::Uint8Array = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))
            .map_err(|error| Error::Failed(describe(&error)))?
            .unchecked_into();
        if value.length() as usize > limit - body.len() {
            let _ = reader.cancel();
            return Err(Error::TooLarge { limit });
        }
        body.extend_from_slice(&value.to_vec());
    }
}

pub(crate) async fn connect(url: &str) -> Result<(Writer, Reader), String> {
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        return Err(format!(
            "the browser build speaks only websockets, not {url}"
        ));
    }
    let socket = web_sys::WebSocket::new(url)
        .map_err(|error| format!("failed to connect to {url}: {}", describe(&error)))?;
    socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
    let socket = Rc::new(socket);
    let (frames, incoming) = mpsc::unbounded();
    let (opened, handshake) = oneshot::channel();
    let opened = Rc::new(RefCell::new(Some(opened)));

    let on_message = {
        let frames = frames.clone();
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            let _ = frames.unbounded_send(Frame::Binary(bytes));
        })
    };
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

    let on_open = {
        let opened = Rc::clone(&opened);
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            if let Some(opened) = opened.borrow_mut().take() {
                let _ = opened.send(Ok(()));
            }
        })
    };
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

    let on_error = {
        let frames = frames.clone();
        let opened = Rc::clone(&opened);
        let url = url.to_owned();
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            match opened.borrow_mut().take() {
                Some(opened) => {
                    let _ = opened.send(Err(format!("failed to connect to {url}")));
                }
                None => {
                    let _ = frames.unbounded_send(Frame::Closed);
                }
            }
        })
    };
    socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

    let on_close = {
        let opened = Rc::clone(&opened);
        Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            match opened.borrow_mut().take() {
                Some(opened) => {
                    let _ = opened.send(Err("the server closed the connection".to_owned()));
                }
                None => {
                    let _ = frames.unbounded_send(Frame::Closed);
                }
            }
        })
    };
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

    let reader = Reader {
        socket: Rc::clone(&socket),
        frames: incoming,
        _handlers: vec![on_open, on_error, on_close],
        _on_message: on_message,
    };
    handshake
        .await
        .map_err(|_| "the websocket handshake was abandoned".to_owned())??;

    Ok((Writer(socket), reader))
}

impl Writer {
    pub(crate) async fn send(&mut self, bytes: Vec<u8>) -> Result<(), String> {
        self.0
            .send_with_js_u8_array(&js_sys::Uint8Array::from(bytes.as_slice()))
            .map_err(|error| describe(&error))
    }
}

impl Reader {
    pub(crate) async fn next(&mut self) -> Option<Frame> {
        self.frames.next().await
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.socket.set_onmessage(None);
        self.socket.set_onopen(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
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
