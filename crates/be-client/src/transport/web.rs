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

pub(crate) async fn connect(url: &str) -> Result<(Writer, Reader), String> {
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

    handshake
        .await
        .map_err(|_| "the websocket handshake was abandoned".to_owned())??;

    Ok((
        Writer(Rc::clone(&socket)),
        Reader {
            socket,
            frames: incoming,
            _handlers: vec![on_open, on_error, on_close],
            _on_message: on_message,
        },
    ))
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
