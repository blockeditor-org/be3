use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use crate::editors::plugin::discovery::{self, Module};

use block_plugin_api::{Message, PluginManifest, ScreenLayout, decode_frame, encode_frame};
use block_wasm_host::{Host, Plugin};

use super::{
    backend::{Deadline, StepTime},
    surface::{PresentedTexture, SurfaceFrame, gpu},
};
const NO_ENTRY_POINT: &str = "This plugin has no wasm entry point.";
const NO_GPU: &str = "The plugin host has no graphics device.";
const STOPPED: &str = "The plugin worker stopped.";

thread_local! {
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static STOPPING: RefCell<Vec<Thread>> = const { RefCell::new(Vec::new()) };
}

struct Thread {
    handle: thread::JoinHandle<()>,
    ended: Receiver<()>,
}

pub(super) fn wait_for_workers(within: Duration) {
    let stopping = STOPPING.with(|stopping| std::mem::take(&mut *stopping.borrow_mut()));
    let deadline = Instant::now() + within;
    for thread in stopping {
        let left = deadline.saturating_duration_since(Instant::now());
        match thread.ended.recv_timeout(left) {
            Err(RecvTimeoutError::Disconnected) => {
                let _ = thread.handle.join();
            }
            Ok(()) | Err(RecvTimeoutError::Timeout) => {
                eprintln!("block-app: a plugin worker did not stop in time, so it is left running");
            }
        }
    }
}

pub(crate) fn cache_in(directory: PathBuf) {
    CACHE.with(|cache| *cache.borrow_mut() = Some(directory));
}

fn cache() -> Option<PathBuf> {
    CACHE.with(|cache| cache.borrow().clone())
}

struct Produced {
    outbound: Vec<Vec<u8>>,
    presented: Vec<PresentedTexture>,
    took: StepTime,
}

impl Produced {
    fn is_empty(&self) -> bool {
        self.outbound.is_empty() && self.presented.is_empty()
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
    presented: Vec<PresentedTexture>,
    presents: u64,
    took: Option<StepTime>,
    thread: Option<Thread>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        STOPPING.with(|stopping| {
            let mut stopping = stopping.borrow_mut();
            stopping.retain(|stopping| !stopping.handle.is_finished());
            stopping.push(thread);
        });
    }
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
        let (ending, ended) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name(format!("plugin {}", plugin.identity.id))
            .spawn(move || {
                let _ending: Sender<()> = ending;
                run(host, module, orders, reports, watched);
            });
        match spawned {
            Ok(handle) => {
                self.worker = Some(Worker {
                    commands,
                    events,
                    pending: Vec::new(),
                    waiting,
                    received: Vec::new(),
                    ready: false,
                    stepping: false,
                    carrying: false,
                    presented: Vec::new(),
                    presents: 0,
                    took: None,
                    thread: Some(Thread { handle, ended }),
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
        let textures = std::mem::take(&mut worker.presented);
        if textures.is_empty() {
            return None;
        }
        Some(SurfaceFrame {
            textures,
            presents: worker.presents,
            damage: None,
        })
    }

    fn settled(&mut self) -> bool {
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

    fn took(&mut self) -> Option<StepTime> {
        self.worker.as_mut()?.took.take()
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
        if !produced.presented.is_empty() {
            self.took = Some(produced.took);
            for presented in produced.presented {
                self.presented
                    .retain(|held| held.surface != presented.surface);
                self.presented.push(presented);
            }
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
        Event::Ready(produced(&mut plugin, StepTime::default())),
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
        let started = Instant::now();
        let event = match plugin.step() {
            Ok(()) => {
                let step = started.elapsed();
                let (gpu, submit) = plugin.take_gpu_time();
                Event::Stepped(produced(&mut plugin, StepTime { step, gpu, submit }))
            }
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

fn produced(plugin: &mut Plugin, took: StepTime) -> Produced {
    let outbound = plugin.take_outbound();
    let mut presented: Vec<PresentedTexture> = Vec::new();
    for surface in plugin.take_presented() {
        let Some((texture, generation)) = plugin.surface(surface) else {
            continue;
        };
        presented.retain(|held| held.surface != surface);
        presented.push(PresentedTexture {
            surface,
            texture,
            generation,
        });
    }
    Produced {
        outbound,
        presented,
        took,
    }
}
