use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use crate::editors::plugin::discovery;

use block_guest_host::{
    Presented, Produced,
    thread::{self, Runnable, Spawn, Threaded},
};
use block_plugin_api::PluginManifest;
use block_wasm_host::{Host, Plugin};

use super::{
    backend::StepTime,
    surface::{PresentedTexture, SurfaceFrame, gpu},
};

const NO_GPU: &str = "The plugin host has no graphics device.";

pub(super) type Frame = SurfaceFrame;

pub(super) type Channel = Threaded<PresentedTexture>;

thread_local! {
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(super) fn wait_for_workers(within: Duration) {
    thread::wait_for_workers(within);
}

pub(crate) fn cache_in(directory: PathBuf) {
    CACHE.with(|cache| *cache.borrow_mut() = Some(directory));
}

fn cache() -> Option<PathBuf> {
    CACHE.with(|cache| cache.borrow().clone())
}

impl Presented for PresentedTexture {
    fn surface(&self) -> u32 {
        self.surface
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
    let interrupter = host.interrupter();
    thread::spawn(
        Spawn {
            name: format!("plugin {}", plugin.identity.id),
            wake: Arc::new(crate::host::wake),
            waiting,
            interrupt: Box::new(move || interrupter.interrupt()),
            contain: |run| crate::panic_guard::contain(run),
        },
        move || {
            let plugin = host.load_from(&module, discovery::read)?;
            plugin.on_wake(crate::host::wake);
            Ok(Editor(plugin))
        },
    )
}

struct Editor(Plugin);

impl Runnable for Editor {
    type Presented = PresentedTexture;

    fn start(&mut self) -> Result<Produced<PresentedTexture>, String> {
        self.0.start()?;
        Ok(produced(&mut self.0, StepTime::default()))
    }

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<PresentedTexture>, String> {
        let plugin = &mut self.0;
        for frame in frames {
            plugin.send(frame);
        }
        plugin.take_wake();
        let started = Instant::now();
        plugin.step()?;
        let step = started.elapsed();
        let (gpu, submit) = plugin.take_gpu_time();
        Ok(produced(plugin, StepTime { step, gpu, submit }))
    }

    fn take_wake(&mut self) -> bool {
        self.0.take_wake()
    }

    fn stop(&mut self) {
        self.0.stop();
    }
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
