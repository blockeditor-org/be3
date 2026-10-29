use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use wasm_bindgen::{JsCast, prelude::*};

use super::canvases::{Canvases, Placement};
use block_guest_host::{Channel, Event, Presented, Produced, Received, StepTime};

const WORKER_SOURCE: &str = r#"
// The worker one plugin runs in.
//
// A plugin is the same wasm every other platform runs, so the worker hands it
// to plugin.js, which answers the plugin's gpu abi with block-gpu-shim and a
// device of the worker's own. What the plugin shows is drawn into canvases the
// page transferred here and keeps behind the app, so a frame never leaves the
// worker. The page steps the plugin the way the native host does: it says
// "step" with what it has for the plugin, and the worker steps once and answers
// with what came out. A plugin that wakes between steps says so, so the page
// steps it again. A preview the plugin draws leaves as a bitmap of the surfaces
// the page asked to export, and a preview of another plugin's arrives as a
// bitmap the plugin imports as a texture.
let plugin = null;
const shown = new Map();
let exported = [];
const imports = [];

function fail(error) {
    plugin = null;
    self.postMessage({ kind: "error", message: String((error && error.message) || error) });
}

function report(kind, step) {
    const painted = plugin.paint();
    const frames = plugin.collect();
    const [gpu, submit] = plugin.took();
    const pictures = plugin.exported();
    self.postMessage(
        { kind, frames, painted, pictures, woken: plugin.woken(), step, gpu, submit },
        pictures.map((picture) => picture.bitmap),
    );
    const failure = plugin.failure();
    if (failure) {
        fail(failure);
    }
}

function show(data) {
    plugin.show(data.id, data.canvas, data.surface);
}

self.onmessage = async (event) => {
    const data = event.data;
    try {
        if (data.kind === "start") {
            const bootstrap = await import(new URL("./plugin.js", data.url).href);
            plugin = await bootstrap.boot(
                new URL("./block_gpu_shim.js", data.url).href,
                data.url,
                () => self.postMessage({ kind: "wake" }),
                data.imports,
            );
            for (const pending of shown.values()) show(pending);
            shown.clear();
            plugin.exportSurfaces(exported);
            for (const [image, bitmap] of imports) plugin.importImage(image, bitmap);
            imports.length = 0;
            report("ready", 0);
        } else if (data.kind === "step") {
            if (plugin === null) return;
            for (const frame of data.frames) plugin.deliver(frame);
            const started = performance.now();
            plugin.step();
            report("stepped", performance.now() - started);
        } else if (data.kind === "show") {
            if (plugin) {
                show(data);
                plugin.paint();
            } else {
                const pending = shown.get(data.id);
                shown.set(data.id, { ...data, canvas: data.canvas ?? pending?.canvas });
            }
        } else if (data.kind === "forget") {
            if (plugin) plugin.forget(data.id);
            else shown.delete(data.id);
        } else if (data.kind === "export") {
            exported = data.surfaces;
            if (plugin) plugin.exportSurfaces(exported);
        } else if (data.kind === "import") {
            if (plugin) plugin.importImage(data.image, data.bitmap);
            else imports.push([data.image, data.bitmap]);
        } else if (data.kind === "unimport") {
            if (plugin) plugin.forgetImage(data.image);
            else {
                const at = imports.findIndex(([image]) => image === data.image);
                if (at >= 0) imports.splice(at, 1);
            }
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
    events: VecDeque<Event<PresentedBitmap>>,
    stopped: bool,
    woken: bool,
}

pub struct PresentedBitmap {
    pub surface: u32,
    pub bitmap: web_sys::ImageBitmap,
}

impl Presented for PresentedBitmap {
    fn surface(&self) -> u32 {
        self.surface
    }
}

pub struct WorkerChannel {
    worker: web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    canvases: Canvases,
    exported: HashSet<u32>,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl WorkerChannel {
    pub(super) fn start(url: &str, waiting: Arc<AtomicBool>) -> Result<Self, String> {
        let worker = spawn()?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let onmessage = listen(&worker, Rc::clone(&inbox), waiting);
        let message = js_sys::Object::new();
        set(&message, "kind", &"start".into());
        set(&message, "url", &absolute(url).into());
        let direct = js_sys::Array::new();
        for (module, name) in block_plugin_api::embed::IMPORTS {
            direct.push(&js_sys::Array::of2(&(*module).into(), &(*name).into()));
        }
        set(&message, "imports", &direct);
        worker
            .post_message(&message)
            .map_err(|_| "the plugin worker could not be started".to_owned())?;
        Ok(Self {
            worker,
            inbox,
            canvases: Canvases::default(),
            exported: HashSet::new(),
            _onmessage: onmessage,
        })
    }

    pub fn offer(&self, image: u64, bitmap: &web_sys::ImageBitmap) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"import".into());
        set(&message, "image", &(image as f64).into());
        set(&message, "bitmap", bitmap);
        let _ = self.worker.post_message(&message);
    }

    pub fn withdraw(&self, image: u64) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"unimport".into());
        set(&message, "image", &(image as f64).into());
        let _ = self.worker.post_message(&message);
    }

    pub fn export(&mut self, surfaces: HashSet<u32>) {
        if self.exported == surfaces {
            return;
        }
        let listed = js_sys::Array::new();
        for surface in &surfaces {
            listed.push(&(*surface).into());
        }
        self.exported = surfaces;
        let message = js_sys::Object::new();
        set(&message, "kind", &"export".into());
        set(&message, "surfaces", &listed);
        let _ = self.worker.post_message(&message);
    }

    pub fn place(&mut self, placements: Vec<Placement>) -> Result<(), String> {
        let mut canvases = std::mem::take(&mut self.canvases);
        let placed = canvases.place(self, placements);
        self.canvases = canvases;
        placed
    }

    pub(super) fn show(
        &self,
        id: u32,
        canvas: Option<&web_sys::OffscreenCanvas>,
        surface: u32,
    ) -> Result<(), String> {
        let message = js_sys::Object::new();
        set(&message, "kind", &"show".into());
        set(&message, "id", &id.into());
        set(&message, "surface", &surface.into());
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
}

impl Drop for WorkerChannel {
    fn drop(&mut self) {
        self.canvases.clear();
        let message = js_sys::Object::new();
        set(&message, "kind", &"shutdown".into());
        let _ = self.worker.post_message(&message);
        self.worker.terminate();
    }
}

impl Channel for WorkerChannel {
    type Presented = PresentedBitmap;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        let posted = js_sys::Array::new();
        for frame in frames {
            posted.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
        }
        let message = js_sys::Object::new();
        set(&message, "kind", &"step".into());
        set(&message, "frames", &posted);
        self.worker.post_message(&message).is_ok()
    }

    fn take_woken(&mut self) -> bool {
        std::mem::take(&mut self.inbox.borrow_mut().woken)
    }

    fn receive(&mut self) -> Received<PresentedBitmap> {
        let mut inbox = self.inbox.borrow_mut();
        match inbox.events.pop_front() {
            Some(event) => Received::Event(event),
            None if inbox.stopped => Received::Stopped,
            None => Received::Empty,
        }
    }
}

fn milliseconds(data: &JsValue, key: &str) -> Duration {
    Duration::from_secs_f64(get(data, key).as_f64().unwrap_or_default().max(0.0) / 1000.0)
}

fn produced(data: &JsValue) -> Produced<PresentedBitmap> {
    Produced {
        outbound: js_sys::Array::from(&get(data, "frames"))
            .iter()
            .map(|frame| js_sys::Uint8Array::new(&frame).to_vec())
            .collect(),
        presented: js_sys::Array::from(&get(data, "pictures"))
            .iter()
            .filter_map(|picture| {
                Some(PresentedBitmap {
                    surface: get(&picture, "surface").as_f64()? as u32,
                    bitmap: get(&picture, "bitmap").dyn_into().ok()?,
                })
            })
            .collect(),
        painted: get(data, "painted").is_truthy(),
        took: StepTime {
            step: milliseconds(data, "step"),
            gpu: milliseconds(data, "gpu"),
            submit: milliseconds(data, "submit"),
        },
    }
}

fn listen(
    worker: &web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    waiting: Arc<AtomicBool>,
) -> Closure<dyn FnMut(web_sys::MessageEvent)> {
    let onmessage = Closure::wrap(Box::new(move |message: web_sys::MessageEvent| {
        let data = message.data();
        let kind = get(&data, "kind").as_string().unwrap_or_default();
        let event = match kind.as_str() {
            "wake" => {
                inbox.borrow_mut().woken = true;
                block_app_base::host::wake();
                return;
            }
            "ready" => Event::Ready(produced(&data)),
            "stepped" => Event::Stepped(produced(&data)),
            _ => Event::Failed(
                get(&data, "message")
                    .as_string()
                    .unwrap_or_else(|| "The plugin worker failed.".to_owned()),
            ),
        };
        let woken = get(&data, "woken").is_truthy();
        let quiet = event.quiet(woken);
        let mut held = inbox.borrow_mut();
        held.woken |= woken;
        held.events.push_back(event);
        drop(held);
        if block_guest_host::wakes(quiet, &waiting) {
            block_app_base::host::wake();
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
