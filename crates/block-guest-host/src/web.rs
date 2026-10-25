use std::{
    cell::RefCell,
    collections::VecDeque,
    convert::Infallible,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use wasm_bindgen::{JsCast, prelude::*};

use crate::{Channel, Event, Produced, Received, StepTime, Wake};

const WORKER_SOURCE: &str = r#"
let exports = null;
let inbox = [];
let outbox = [];
let woken = false;

function fail(error) {
    exports = null;
    self.postMessage({ kind: "error", message: String((error && error.message) || error) });
}

function report(kind, step) {
    const frames = outbox;
    const wasWoken = woken;
    outbox = [];
    woken = false;
    self.postMessage({ kind, frames, woken: wasWoken, step }, frames);
}

function memory() {
    return new Uint8Array(exports.memory.buffer);
}

const host = {
    host_send(pointer, length) {
        outbox.push(memory().slice(pointer, pointer + length).buffer);
    },
    host_receive(pointer, capacity) {
        if (inbox.length === 0) return -1n;
        const frame = inbox[0];
        if (frame.length > capacity) return BigInt(frame.length);
        memory().set(frame, pointer);
        inbox.shift();
        return BigInt(frame.length);
    },
    host_wake() {
        woken = true;
        self.postMessage({ kind: "wake" });
    },
    host_panic(pointer, length) {
        throw new Error(new TextDecoder().decode(memory().slice(pointer, pointer + length)));
    },
};

function imports(module) {
    const resolved = { __proto__: null };
    for (const wanted of WebAssembly.Module.imports(module)) {
        if (wanted.module !== "be3_host" || !Object.hasOwn(host, wanted.name)) {
            throw new Error(`the guest imports ${wanted.module}.${wanted.name}, which its host does not answer`);
        }
        (resolved[wanted.module] ??= { __proto__: null })[wanted.name] = host[wanted.name];
    }
    return resolved;
}

self.onmessage = async (event) => {
    const data = event.data;
    try {
        if (data.kind === "start") {
            const response = await fetch(data.url);
            if (!response.ok) throw new Error(`${data.url} could not be fetched: ${response.status}`);
            const module = await WebAssembly.compile(await response.arrayBuffer());
            const instance = await WebAssembly.instantiate(module, imports(module));
            exports = instance.exports;
            exports.__wasm_call_ctors?.();
            exports.plugin_start();
            report("ready", 0);
        } else if (data.kind === "step") {
            if (exports === null) return;
            for (const frame of data.frames) inbox.push(new Uint8Array(frame));
            const started = performance.now();
            exports.plugin_step();
            report("stepped", performance.now() - started);
        } else if (data.kind === "shutdown") {
            if (exports !== null) exports.plugin_shutdown();
            self.close();
        }
    } catch (error) {
        fail(error);
    }
};
"#;

#[derive(Default)]
struct Inbox {
    events: VecDeque<Event<Infallible>>,
    woken: bool,
}

pub struct WebGuest {
    worker: web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl WebGuest {
    pub fn start(url: &str, wake: Wake, waiting: Arc<AtomicBool>) -> Result<Self, String> {
        let worker = spawn()?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let onmessage = listen(&worker, Rc::clone(&inbox), wake, waiting);
        let message = js_sys::Object::new();
        set(&message, "kind", &"start".into());
        set(&message, "url", &absolute(url).into());
        worker
            .post_message(&message)
            .map_err(|_| "the guest's worker could not be started".to_owned())?;
        Ok(Self {
            worker,
            inbox,
            _onmessage: onmessage,
        })
    }
}

impl Drop for WebGuest {
    fn drop(&mut self) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"shutdown".into());
        let _ = self.worker.post_message(&message);
        self.worker.terminate();
    }
}

impl Channel for WebGuest {
    type Presented = Infallible;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        let buffers = js_sys::Array::new();
        for frame in frames {
            buffers.push(&js_sys::Uint8Array::from(frame.as_slice()).buffer());
        }
        let message = js_sys::Object::new();
        set(&message, "kind", &"step".into());
        set(&message, "frames", &buffers);
        self.worker
            .post_message_with_transfer(&message, &buffers)
            .is_ok()
    }

    fn take_woken(&mut self) -> bool {
        std::mem::take(&mut self.inbox.borrow_mut().woken)
    }

    fn receive(&mut self) -> Received<Infallible> {
        match self.inbox.borrow_mut().events.pop_front() {
            Some(event) => Received::Event(event),
            None => Received::Empty,
        }
    }
}

fn produced(data: &JsValue) -> Produced<Infallible> {
    let outbound = js_sys::Array::from(&get(data, "frames"))
        .iter()
        .map(|frame| js_sys::Uint8Array::new(&frame).to_vec())
        .collect();
    let step = get(data, "step").as_f64().unwrap_or_default().max(0.0) / 1000.0;
    Produced::sent(
        outbound,
        StepTime {
            step: Duration::from_secs_f64(step),
            ..StepTime::default()
        },
    )
}

fn listen(
    worker: &web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    wake: Wake,
    waiting: Arc<AtomicBool>,
) -> Closure<dyn FnMut(web_sys::MessageEvent)> {
    let onmessage = Closure::wrap(Box::new(move |message: web_sys::MessageEvent| {
        let data = message.data();
        let kind = get(&data, "kind").as_string().unwrap_or_default();
        let event = match kind.as_str() {
            "wake" => {
                inbox.borrow_mut().woken = true;
                wake();
                return;
            }
            "ready" => Event::Ready(produced(&data)),
            "stepped" => Event::Stepped(produced(&data)),
            _ => Event::Failed(
                get(&data, "message")
                    .as_string()
                    .unwrap_or_else(|| "The guest's worker failed.".to_owned()),
            ),
        };
        let woken = get(&data, "woken").is_truthy();
        let quiet = event.quiet(woken);
        let mut held = inbox.borrow_mut();
        held.woken |= woken;
        held.events.push_back(event);
        drop(held);
        if crate::wakes(quiet, &waiting) {
            wake();
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage
}

fn spawn() -> Result<web_sys::Worker, String> {
    let source = js_sys::Array::of1(&WORKER_SOURCE.into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&source, &options)
        .map_err(|_| "the guest's worker could not be assembled".to_owned())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "the guest's worker could not be addressed".to_owned())?;
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&url, &options)
        .map_err(|_| "the guest's worker could not be started".to_owned());
    let _ = web_sys::Url::revoke_object_url(&url);
    worker
}

fn absolute(url: &str) -> String {
    let base = match web_sys::window() {
        Some(window) => window
            .document()
            .and_then(|document| document.base_uri().ok().flatten())
            .unwrap_or_default(),
        None => js_sys::Reflect::get(&js_sys::global(), &"location".into())
            .and_then(|location| js_sys::Reflect::get(&location, &"href".into()))
            .ok()
            .and_then(|href| href.as_string())
            .unwrap_or_default(),
    };
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
