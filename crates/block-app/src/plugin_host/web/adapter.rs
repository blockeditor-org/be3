use block_plugin_api::{Message, encode_frame};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};

const WORKER_SOURCE: &str = r#"
// The worker one plugin runs in.
//
// A plugin is the same wasm every other platform runs, so the worker hands it
// to plugin.js, which answers the plugin's gpu abi with block-gpu-shim and a
// device of the worker's own. What the plugin shows is drawn into canvases the
// page transferred here and keeps behind the app, so a frame never leaves the
// worker. Stepping is scheduled rather than immediate: a step that produced
// something schedules the next one, and a quiet plugin stops until the host
// says something.
let plugin = null;
const queued = [];
const shown = new Map();
let scheduled = false;

function fail(error) {
    self.postMessage({ kind: "error", message: String((error && error.message) || error) });
}

function schedule() {
    if (scheduled || plugin === null) {
        return;
    }
    scheduled = true;
    setTimeout(step, 0);
}

function drain() {
    plugin.paint();
    const frames = plugin.collect();
    if (frames.length > 0) {
        self.postMessage({ kind: "frames", frames });
    }
    if (frames.length > 0 || plugin.woken()) {
        schedule();
    }
    const failure = plugin.failure();
    if (failure) {
        fail(failure);
    }
}

function step() {
    scheduled = false;
    if (plugin === null) {
        return;
    }
    try {
        plugin.step();
        drain();
    } catch (error) {
        plugin = null;
        fail(error);
    }
}

function show(data) {
    plugin.show(data.id, data.canvas, data.x, data.y, data.width, data.height);
}

self.onmessage = async (event) => {
    const data = event.data;
    try {
        if (data.kind === "start") {
            const bootstrap = await import(new URL("./plugin.js", data.url).href);
            plugin = await bootstrap.boot(
                new URL("./block_gpu_shim.js", data.url).href,
                data.url,
                schedule,
            );
            self.postMessage({ kind: "ready" });
            for (const pending of shown.values()) show(pending);
            shown.clear();
            drain();
            for (const frame of queued.splice(0)) plugin.deliver(frame);
            schedule();
        } else if (data.kind === "frames") {
            for (const frame of data.frames) {
                if (plugin) plugin.deliver(frame);
                else queued.push(frame);
            }
            schedule();
        } else if (data.kind === "show") {
            if (plugin) {
                show(data);
                drain();
            } else {
                const pending = shown.get(data.id);
                shown.set(data.id, { ...data, canvas: data.canvas ?? pending?.canvas });
            }
        } else if (data.kind === "forget") {
            if (plugin) plugin.forget(data.id);
            else shown.delete(data.id);
        } else if (data.kind === "shutdown") {
            if (plugin) plugin.shutdown();
            self.close();
        }
    } catch (error) {
        fail(error);
    }
};
"#;

#[derive(Default)]
struct Inbox {
    ready: bool,
    delivered: Vec<Vec<Vec<u8>>>,
    error: Option<String>,
}

pub(super) struct WebProtocolAdapter {
    worker: web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    spoken: bool,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl WebProtocolAdapter {
    pub(super) fn start(url: &str) -> Result<Self, String> {
        let worker = spawn()?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let onmessage = listen(&worker, Rc::clone(&inbox));
        let message = js_sys::Object::new();
        set(&message, "kind", &"start".into());
        set(&message, "url", &absolute(url).into());
        worker
            .post_message(&message)
            .map_err(|_| "the plugin worker could not be started".to_owned())?;
        Ok(Self {
            worker,
            inbox,
            spoken: false,
            _onmessage: onmessage,
        })
    }

    pub(super) fn ready(&self) -> bool {
        self.inbox.borrow().ready
    }

    pub(super) fn running(&self) -> bool {
        self.spoken
    }

    pub(super) fn send(&mut self, messages: Vec<Message>) -> Result<(), String> {
        let frames = js_sys::Array::new();
        for message in messages {
            let frame = encode_frame(&message).map_err(|error| error.to_string())?;
            frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
        }
        if frames.length() == 0 {
            return Ok(());
        }
        let message = js_sys::Object::new();
        set(&message, "kind", &"frames".into());
        set(&message, "frames", &frames);
        self.worker
            .post_message(&message)
            .map_err(|_| "the plugin worker stopped listening".to_owned())
    }

    pub(super) fn show(
        &self,
        id: u32,
        canvas: Option<&web_sys::OffscreenCanvas>,
        [x, y, width, height]: [u32; 4],
    ) -> Result<(), String> {
        let message = js_sys::Object::new();
        set(&message, "kind", &"show".into());
        set(&message, "id", &id.into());
        set(&message, "x", &x.into());
        set(&message, "y", &y.into());
        set(&message, "width", &width.into());
        set(&message, "height", &height.into());
        let posted = match canvas {
            Some(canvas) => {
                set(&message, "canvas", canvas);
                self.worker
                    .post_message_with_transfer(&message, &js_sys::Array::of1(canvas))
            }
            None => self.worker.post_message(&message),
        };
        posted.map_err(|_| "a plugin screen could not be handed to its worker".to_owned())
    }

    pub(super) fn forget(&self, id: u32) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"forget".into());
        set(&message, "id", &id.into());
        let _ = self.worker.post_message(&message);
    }

    pub(super) fn poll(&mut self) -> Result<Vec<Message>, String> {
        let (delivered, error) = {
            let mut inbox = self.inbox.borrow_mut();
            (std::mem::take(&mut inbox.delivered), inbox.error.take())
        };
        if let Some(error) = error {
            return Err(error);
        }
        let mut messages = Vec::new();
        for frame in delivered.into_iter().flatten() {
            messages.push(decode(&frame)?);
            self.spoken = true;
        }
        Ok(messages)
    }

    pub(super) fn shutdown(&mut self) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"shutdown".into());
        let _ = self.worker.post_message(&message);
        self.worker.terminate();
    }
}

fn listen(
    worker: &web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
) -> Closure<dyn FnMut(web_sys::MessageEvent)> {
    let onmessage = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        let data = event.data();
        let kind = get(&data, "kind").as_string().unwrap_or_default();
        let mut inbox = inbox.borrow_mut();
        match kind.as_str() {
            "ready" => inbox.ready = true,
            "frames" => {
                let frames = js_sys::Array::from(&get(&data, "frames"))
                    .iter()
                    .map(|frame| js_sys::Uint8Array::new(&frame).to_vec())
                    .collect();
                inbox.delivered.push(frames);
            }
            _ => {
                inbox.error = Some(
                    get(&data, "message")
                        .as_string()
                        .unwrap_or_else(|| "The plugin worker failed.".to_owned()),
                );
            }
        }
        crate::host::wake();
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage
}

fn spawn() -> Result<web_sys::Worker, String> {
    let source = js_sys::Array::of1(&WORKER_SOURCE.into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&source, &options)
        .map_err(|_| "the plugin worker could not be assembled".to_owned())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "the plugin worker could not be addressed".to_owned())?;
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&url, &options)
        .map_err(|_| "the plugin worker could not be started".to_owned());
    let _ = web_sys::Url::revoke_object_url(&url);
    worker
}

fn absolute(url: &str) -> String {
    let base = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.base_uri().ok().flatten())
        .unwrap_or_default();
    web_sys::Url::new_with_base(url, &base)
        .map(|url| url.href())
        .unwrap_or_else(|_| url.to_owned())
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(object, &key.into(), value);
}

fn get(object: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

fn decode(frame: &[u8]) -> Result<Message, String> {
    block_plugin_api::decode_frame(frame).map_err(|error| error.to_string())
}
