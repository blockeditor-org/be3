use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use block_app_stack::discovery;

use block_guest_host::{
    Presented, Produced,
    thread::{self, Runnable, Spawn, Threaded},
};
use block_plugin_manifest::PluginManifest;
use block_wasm_host::{Host, Plugin};

use super::{
    backend::StepTime,
    surface::{PresentedTexture, SurfaceFrame, gpu},
};

const NO_GPU: &str = "The plugin host has no graphics device.";

pub(super) type Frame = SurfaceFrame;

pub(super) type Channel = Threaded<PresentedTexture>;

pub(super) type Picture = wgpu::Texture;

pub(super) type Mailbox = block_wasm_host::Imports;

thread_local! {
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(super) fn wait_for_workers(within: Duration) {
    thread::wait_for_workers(within);
}

pub fn cache_in(directory: PathBuf) {
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
) -> Result<(Channel, Mailbox), String> {
    let host = match gpu() {
        Some((device, queue)) => Host::new(device, queue, cache().as_deref())?,
        None => Host::on_demand(|| Err(NO_GPU.to_owned()), cache().as_deref())?,
    }
    .with_direct(block_plugin_api::embed::IMPORTS);
    let interrupter = host.interrupter();
    let mailbox = host.imports();
    let channel = thread::spawn(
        Spawn {
            name: format!("plugin {}", plugin.identity.id),
            wake: Arc::new(block_app_base::host::wake),
            waiting,
            interrupt: Box::new(move || interrupter.interrupt()),
            contain: |run| block_app_base::panic_guard::contain(run),
        },
        move |wake| {
            let plugin = host.load_from(&module, discovery::open)?;
            plugin.on_wake(move || wake());
            Ok(Editor(plugin))
        },
    )?;
    Ok((channel, mailbox))
}

pub(super) fn offer(mailbox: &Mailbox, _channel: &mut Channel, image: u64, picture: &Picture) {
    mailbox
        .lock()
        .unwrap_or_else(|held| held.into_inner())
        .insert(image, picture.clone());
}

pub(super) fn withdraw(mailbox: &Mailbox, _channel: &mut Channel, image: u64) {
    mailbox
        .lock()
        .unwrap_or_else(|held| held.into_inner())
        .remove(&image);
}

pub(super) fn export(_channel: &mut Channel, _surfaces: Vec<u32>) {}

pub(super) fn release(_picture: &Picture) {}

pub(super) fn pictures(frame: &SurfaceFrame, surfaces: &[u32]) -> Vec<(u32, Picture)> {
    frame
        .textures
        .iter()
        .filter(|presented| surfaces.contains(&presented.surface))
        .filter_map(|presented| {
            Some((
                presented.surface,
                super::surface::copied(&presented.texture)?,
            ))
        })
        .collect()
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
        plugin.take_wake();
        let started = Instant::now();
        plugin.deliver(frames)?;
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

#[cfg(test)]
mod tests;
