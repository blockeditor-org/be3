use std::{
    cell::RefCell,
    collections::VecDeque,
    convert::Infallible,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use wasm_bindgen::{JsCast, prelude::*};

use crate::{Channel, Event, Presented, Produced, Received, StepTime, Wake};

const WORKER_SOURCE: &str = concat!(include_str!("guest.js"), "\nserve();\n");

pub trait Pictures: Presented + Sized + 'static {
    fn read(report: &JsValue) -> Vec<Self>;
}

impl Pictures for Infallible {
    fn read(_report: &JsValue) -> Vec<Self> {
        Vec::new()
    }
}

struct Inbox<P> {
    events: VecDeque<Event<P>>,
    woken: bool,
}

impl<P> Default for Inbox<P> {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            woken: false,
        }
    }
}

pub struct WebGuest<P> {
    worker: web_sys::Worker,
    inbox: Rc<RefCell<Inbox<P>>>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl<P: Pictures> WebGuest<P> {
    pub fn start(
        url: &str,
        imports: &[(&str, &str)],
        extension: Option<&str>,
        wake: Wake,
        waiting: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let worker = spawn()?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let onmessage = listen(&worker, Rc::clone(&inbox), wake, waiting);
        let message = message("start");
        set(&message, "url", &absolute(url).into());
        if let Some(extension) = extension {
            set(&message, "extension", &extension.into());
        }
        let direct = js_sys::Array::new();
        for (module, name) in imports {
            direct.push(&js_sys::Array::of2(&(*module).into(), &(*name).into()));
        }
        set(&message, "imports", &direct);
        worker
            .post_message(&message)
            .map_err(|_| "the guest's worker could not be started".to_owned())?;
        Ok(Self {
            worker,
            inbox,
            _onmessage: onmessage,
        })
    }

    pub fn post(&self, message: &js_sys::Object, transfer: Option<&JsValue>) -> bool {
        match transfer {
            Some(transfer) => self
                .worker
                .post_message_with_transfer(message, &js_sys::Array::of1(transfer)),
            None => self.worker.post_message(message),
        }
        .is_ok()
    }
}

impl<P> Drop for WebGuest<P> {
    fn drop(&mut self) {
        let _ = self.worker.post_message(&message("shutdown"));
        self.worker.terminate();
    }
}

impl<P: Pictures> Channel for WebGuest<P> {
    type Presented = P;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        let buffers = js_sys::Array::new();
        for frame in frames {
            buffers.push(&js_sys::Uint8Array::from(frame.as_slice()).buffer());
        }
        let message = message("step");
        set(&message, "frames", &buffers);
        self.worker
            .post_message_with_transfer(&message, &buffers)
            .is_ok()
    }

    fn take_woken(&mut self) -> bool {
        std::mem::take(&mut self.inbox.borrow_mut().woken)
    }

    fn receive(&mut self) -> Received<P> {
        match self.inbox.borrow_mut().events.pop_front() {
            Some(event) => Received::Event(event),
            None => Received::Empty,
        }
    }
}

fn milliseconds(data: &JsValue, key: &str) -> Duration {
    Duration::from_secs_f64(get(data, key).as_f64().unwrap_or_default().max(0.0) / 1000.0)
}

fn produced<P: Pictures>(data: &JsValue) -> Produced<P> {
    Produced {
        outbound: js_sys::Array::from(&get(data, "frames"))
            .iter()
            .map(|frame| js_sys::Uint8Array::new(&frame).to_vec())
            .collect(),
        presented: P::read(data),
        painted: get(data, "painted").is_truthy(),
        took: StepTime {
            step: milliseconds(data, "step"),
            gpu: milliseconds(data, "gpu"),
            submit: milliseconds(data, "submit"),
        },
    }
}

fn listen<P: Pictures>(
    worker: &web_sys::Worker,
    inbox: Rc<RefCell<Inbox<P>>>,
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

pub fn message(kind: &str) -> js_sys::Object {
    let message = js_sys::Object::new();
    set(&message, "kind", &kind.into());
    message
}

pub fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(object, &key.into(), value);
}

pub fn get(object: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}
