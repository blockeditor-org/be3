use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use crate::editors::plugin::discovery;

use block_plugin_api::PluginManifest;
use block_wasm_host::{Host, Plugin};

use super::{
    backend::StepTime,
    surface::{PresentedTexture, SurfaceFrame, gpu},
    worker::{self, Event, Produced, Received},
};

const NO_GPU: &str = "The plugin host has no graphics device.";

pub(super) type Frame = SurfaceFrame;

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

impl worker::Presented for PresentedTexture {
    fn surface(&self) -> u32 {
        self.surface
    }
}

pub(super) struct Channel {
    steps: Sender<Vec<Vec<u8>>>,
    events: Receiver<Event<PresentedTexture>>,
    thread: Option<Thread>,
}

impl Drop for Channel {
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

fn received(
    received: Result<Event<PresentedTexture>, TryRecvError>,
) -> Received<PresentedTexture> {
    match received {
        Ok(event) => Received::Event(event),
        Err(TryRecvError::Empty) => Received::Empty,
        Err(TryRecvError::Disconnected) => Received::Stopped,
    }
}

impl worker::Channel for Channel {
    type Presented = PresentedTexture;

    fn step(&mut self, frames: Vec<Vec<u8>>) -> bool {
        self.steps.send(frames).is_ok()
    }

    fn receive(&mut self) -> Received<PresentedTexture> {
        received(self.events.try_recv())
    }

    fn wait(&mut self, timeout: Duration) -> Option<Received<PresentedTexture>> {
        Some(match self.events.recv_timeout(timeout) {
            Ok(event) => Received::Event(event),
            Err(RecvTimeoutError::Timeout) => Received::Empty,
            Err(RecvTimeoutError::Disconnected) => Received::Stopped,
        })
    }
}

pub(super) fn frame(textures: Vec<PresentedTexture>, presents: u64) -> SurfaceFrame {
    SurfaceFrame {
        textures,
        presents,
        damage: None,
    }
}

pub(super) fn spawn(
    plugin: &PluginManifest,
    module: String,
    waiting: Arc<AtomicBool>,
) -> Result<Channel, String> {
    let (device, queue) = gpu().ok_or(NO_GPU)?;
    let host = Host::new(device, queue, cache().as_deref())?;
    let (steps, orders) = mpsc::channel();
    let (reports, events) = mpsc::channel();
    let (ending, ended) = mpsc::channel();
    let handle = thread::Builder::new()
        .name(format!("plugin {}", plugin.identity.id))
        .spawn(move || {
            let _ending: Sender<()> = ending;
            let failed = reports.clone();
            let ran = crate::panic_guard::contain(|| run(host, module, orders, reports, waiting));
            if let Err(panic) = ran {
                let _ = failed.send(Event::Failed(format!(
                    "The app failed while running it: {panic}"
                )));
                crate::host::wake();
            }
        })
        .map_err(|error| format!("the plugin worker could not start: {error}"))?;
    Ok(Channel {
        steps,
        events,
        thread: Some(Thread { handle, ended }),
    })
}

fn run(
    host: Host,
    module: String,
    orders: Receiver<Vec<Vec<u8>>>,
    reports: Sender<Event<PresentedTexture>>,
    waiting: Arc<AtomicBool>,
) {
    let mut plugin = match host.load_from(&module, discovery::read) {
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
    for frames in orders {
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

fn report(
    reports: &Sender<Event<PresentedTexture>>,
    event: Event<PresentedTexture>,
    woken: bool,
    waiting: &AtomicBool,
) -> bool {
    let wakes = event.wakes(woken, waiting);
    let sent = reports.send(event).is_ok();
    if wakes {
        crate::host::wake();
    }
    sent
}

fn produced(plugin: &mut Plugin, took: StepTime) -> Produced<PresentedTexture> {
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
        painted: !presented.is_empty(),
        outbound,
        presented,
        took,
    }
}
