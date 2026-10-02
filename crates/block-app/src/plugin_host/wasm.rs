use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread,
    time::Instant,
};

use crate::editors::plugin::discovery::{self, Module};

use block_plugin_api::{Message, PluginManifest, ScreenLayout, decode_frame, encode_frame};
use block_wasm_host::{Host, Plugin};

use super::{
    backend::Deadline,
    surface::{SurfaceFrame, gpu},
};

const SCREENS_SURFACE: u32 = 0;
const NO_ENTRY_POINT: &str = "This plugin has no wasm entry point.";
const NO_GPU: &str = "The plugin host has no graphics device.";
const STOPPED: &str = "The plugin worker stopped.";

thread_local! {
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn cache_in(directory: PathBuf) {
    CACHE.with(|cache| *cache.borrow_mut() = Some(directory));
}

fn cache() -> Option<PathBuf> {
    CACHE.with(|cache| cache.borrow().clone())
}

struct Target {
    texture: wgpu::Texture,
    generation: u64,
}

struct Produced {
    outbound: Vec<Vec<u8>>,
    presented: Option<Target>,
}

impl Produced {
    fn is_empty(&self) -> bool {
        self.outbound.is_empty() && self.presented.is_none()
    }
}

enum Command {
    Step(Vec<Vec<u8>>),
}

enum Event {
    Ready(Produced),
    Stepped(Produced),
    Failed(String),
}

struct Worker {
    commands: Sender<Command>,
    events: Receiver<Event>,
    pending: Vec<Vec<u8>>,
    waiting: Arc<AtomicBool>,
    received: Vec<Vec<u8>>,
    ready: bool,
    stepping: bool,
    carrying: bool,
    target: Option<Target>,
    presented: bool,
    presents: u64,
}

pub(super) struct Wasm {
    worker: Option<Worker>,
    module: Option<Module>,
    started: Instant,
    error: Option<String>,
}

impl super::backend::Backend for Wasm {
    type Frame = SurfaceFrame;

    fn new(plugin: &PluginManifest) -> Self {
        Self {
            worker: None,
            module: discovery::module(&plugin.identity.id, &plugin.entry_point),
            started: Instant::now(),
            error: None,
        }
    }

    fn start(&mut self, plugin: &PluginManifest) {
        self.shutdown();
        self.started = Instant::now();
        self.error = None;
        let Some(module) = self.module.clone() else {
            self.error = Some(NO_ENTRY_POINT.to_owned());
            return;
        };
        let Some((device, queue)) = gpu() else {
            self.error = Some(NO_GPU.to_owned());
            return;
        };
        let host = match Host::new(device, queue, cache().as_deref()) {
            Ok(host) => host,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let (commands, orders) = mpsc::channel();
        let (reports, events) = mpsc::channel();
        let waiting = Arc::new(AtomicBool::new(false));
        let watched = Arc::clone(&waiting);
        let spawned = thread::Builder::new()
            .name(format!("plugin {}", plugin.identity.id))
            .spawn(move || run(host, module, orders, reports, watched));
        match spawned {
            Ok(_) => {
                self.worker = Some(Worker {
                    commands,
                    events,
                    pending: Vec::new(),
                    waiting,
                    received: Vec::new(),
                    ready: false,
                    stepping: false,
                    carrying: false,
                    target: None,
                    presented: false,
                    presents: 0,
                })
            }
            Err(error) => self.error = Some(format!("the plugin worker could not start: {error}")),
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        let mut failure = None;
        for message in messages {
            match encode_frame(&message) {
                Ok(frame) => worker.pending.push(frame),
                Err(error) => failure = Some(error.to_string()),
            }
        }
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
        }
        if !worker.pending.is_empty() {
            worker.waiting.store(true, Ordering::SeqCst);
            self.poll_worker();
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        self.poll_worker();
        let Some(worker) = &mut self.worker else {
            return Vec::new();
        };
        decode(std::mem::take(&mut worker.received), &mut self.error)
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<SurfaceFrame> {
        None
    }

    fn received_frame(&mut self) -> Option<SurfaceFrame> {
        let worker = self.worker.as_mut()?;
        if !std::mem::take(&mut worker.presented) {
            return None;
        }
        let target = worker.target.as_ref()?;
        Some(SurfaceFrame {
            texture: target.texture.clone(),
            generation: target.generation,
            presents: worker.presents,
            damage: None,
        })
    }

    fn settled(&mut self) -> bool {
        self.poll_worker();
        self.worker.as_ref().is_none_or(|worker| {
            !worker.ready || (worker.pending.is_empty() && !(worker.stepping && worker.carrying))
        })
    }

    fn wait(&mut self, deadline: Deadline) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let Some(timeout) = deadline.remaining() else {
            return false;
        };
        let failure = match worker.events.recv_timeout(timeout) {
            Ok(event) => worker.handle(event).or_else(|| worker.poll()),
            Err(RecvTimeoutError::Timeout) => return false,
            Err(RecvTimeoutError::Disconnected) => Some(STOPPED.to_owned()),
        };
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
            self.worker = None;
        }
        true
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match &self.worker {
            Some(worker) if worker.ready => "running",
            Some(_) => "starting",
            None => "stopped",
        }
    }

    fn uptime(&self) -> Option<std::time::Duration> {
        self.worker.is_some().then(|| self.started.elapsed())
    }

    fn shutdown(&mut self) {
        self.worker = None;
    }
}

impl Wasm {
    fn poll_worker(&mut self) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        if let Some(failure) = worker.poll() {
            self.error.get_or_insert(failure);
            self.worker = None;
        }
    }
}

impl Worker {
    fn poll(&mut self) -> Option<String> {
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    if let Some(failure) = self.handle(event) {
                        return Some(failure);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Some(STOPPED.to_owned()),
            }
        }
        if !self.ready || self.stepping {
            return None;
        }
        self.waiting.store(false, Ordering::SeqCst);
        let carrying = !self.pending.is_empty();
        let step = Command::Step(std::mem::take(&mut self.pending));
        match self.commands.send(step) {
            Ok(()) => {
                self.stepping = true;
                self.carrying = carrying;
                None
            }
            Err(_) => Some(STOPPED.to_owned()),
        }
    }

    fn handle(&mut self, event: Event) -> Option<String> {
        match event {
            Event::Ready(produced) => {
                self.ready = true;
                self.absorb(produced);
            }
            Event::Stepped(produced) => {
                self.stepping = false;
                self.absorb(produced);
            }
            Event::Failed(error) => return Some(error),
        }
        None
    }

    fn absorb(&mut self, produced: Produced) {
        self.received.extend(produced.outbound);
        if let Some(target) = produced.presented {
            self.target = Some(target);
            self.presented = true;
            self.presents += 1;
        }
    }
}

fn decode(frames: Vec<Vec<u8>>, error: &mut Option<String>) -> Vec<Message> {
    let mut messages = Vec::with_capacity(frames.len());
    for frame in frames {
        match decode_frame(&frame) {
            Ok(message) => messages.push(message),
            Err(failure) => {
                error.get_or_insert(format!("{failure:?}"));
            }
        }
    }
    messages
}

fn open(host: &Host, module: &Module) -> Result<Plugin, String> {
    #[cfg(not(target_os = "android"))]
    {
        host.load(module)
    }
    #[cfg(target_os = "android")]
    {
        host.load_bytes(module)
    }
}

fn run(
    host: Host,
    module: Module,
    orders: Receiver<Command>,
    reports: Sender<Event>,
    waiting: Arc<AtomicBool>,
) {
    let mut plugin = match open(&host, &module) {
        Ok(plugin) => plugin,
        Err(error) => {
            let _ = reports.send(Event::Failed(error));
            crate::host::wake();
            return;
        }
    };
    plugin.on_wake(crate::host::wake);
    if let Err(error) = plugin.start() {
        let _ = reports.send(Event::Failed(error));
        crate::host::wake();
        return;
    }
    if !report(
        &reports,
        Event::Ready(produced(&mut plugin)),
        true,
        &waiting,
    ) {
        return;
    }
    for order in orders {
        let Command::Step(frames) = order;
        for frame in frames {
            plugin.send(frame);
        }
        plugin.take_wake();
        let event = match plugin.step() {
            Ok(()) => Event::Stepped(produced(&mut plugin)),
            Err(error) => Event::Failed(error),
        };
        let woken = plugin.take_wake();
        let failed = matches!(event, Event::Failed(_));
        if !report(&reports, event, woken, &waiting) || failed {
            return;
        }
    }
    plugin.stop();
}

fn report(reports: &Sender<Event>, event: Event, woken: bool, waiting: &AtomicBool) -> bool {
    let quiet = !woken && matches!(&event, Event::Stepped(produced) if produced.is_empty());
    let sent = reports.send(event).is_ok();
    if !quiet || waiting.load(Ordering::SeqCst) {
        crate::host::wake();
    }
    sent
}

fn produced(plugin: &mut Plugin) -> Produced {
    let outbound = plugin.take_outbound();
    let presented = plugin
        .take_presented()
        .contains(&SCREENS_SURFACE)
        .then(|| plugin.surface(SCREENS_SURFACE))
        .flatten()
        .map(|(texture, generation)| Target {
            texture,
            generation,
        });
    Produced {
        outbound,
        presented,
    }
}
