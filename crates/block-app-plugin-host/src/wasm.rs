use std::{
    cell::RefCell,
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, PoisonError, atomic::AtomicBool},
    time::Duration,
};

use block_app_stack::discovery;

use block_guest_host::{
    Presented, Produced,
    thread::{self, Runnable, Threaded},
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

type Gpu = Result<(wgpu::Device, wgpu::Queue), String>;

thread_local! {
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static PREPARED: RefCell<Vec<Prepared>> = const { RefCell::new(Vec::new()) };
}

struct Prepared {
    plugin_id: String,
    module: String,
    started: Result<(Channel, Mailbox, Arc<AtomicBool>), String>,
    device: Arc<Handover>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        self.device.hand(Err(NO_GPU.to_owned()));
    }
}

#[derive(Default)]
struct Handover {
    device: Mutex<Option<Gpu>>,
    handed: Condvar,
}

impl Handover {
    fn hand(&self, gpu: Gpu) {
        let mut device = self.device.lock().unwrap_or_else(PoisonError::into_inner);
        device.get_or_insert(gpu);
        self.handed.notify_all();
    }

    fn wait(&self) -> Gpu {
        let device = self.device.lock().unwrap_or_else(PoisonError::into_inner);
        let device = self
            .handed
            .wait_while(device, |device| device.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        device.clone().unwrap_or_else(|| Err(NO_GPU.to_owned()))
    }
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

pub(super) fn prepare(plugin: &PluginManifest, module: String) {
    let device = Arc::<Handover>::default();
    let handover = Arc::clone(&device);
    let started = Host::awaiting(move || handover.wait(), cache().as_deref())
        .and_then(|host| start(plugin, &module, host));
    PREPARED.with(|prepared| {
        prepared.borrow_mut().push(Prepared {
            plugin_id: plugin.identity.id.clone(),
            module,
            started,
            device,
        });
    });
}

pub(super) fn hand_over_device() {
    let device = gpu().ok_or_else(|| NO_GPU.to_owned());
    PREPARED.with(|prepared| {
        for prepared in prepared.borrow().iter() {
            prepared.device.hand(device.clone());
        }
    });
}

pub(super) fn discard_prepared() {
    let discarded = PREPARED.with(|prepared| std::mem::take(&mut *prepared.borrow_mut()));
    drop(discarded);
}

pub(super) fn spawn(
    plugin: &PluginManifest,
    module: String,
) -> Result<(Channel, Mailbox, Arc<AtomicBool>), String> {
    let prepared = PREPARED.with(|prepared| {
        let mut prepared = prepared.borrow_mut();
        let index = prepared.iter().position(|prepared| {
            prepared.plugin_id == plugin.identity.id && prepared.module == module
        })?;
        Some(prepared.remove(index))
    });
    if let Some(mut prepared) = prepared {
        return std::mem::replace(&mut prepared.started, Err(NO_GPU.to_owned()));
    }
    let host = match gpu() {
        Some((device, queue)) => Host::new(device, queue, cache().as_deref())?,
        None => Host::on_demand(|| Err(NO_GPU.to_owned()), cache().as_deref())?,
    };
    start(plugin, &module, host)
}

fn start(
    plugin: &PluginManifest,
    module: &str,
    host: Host,
) -> Result<(Channel, Mailbox, Arc<AtomicBool>), String> {
    let host = host.with_direct(block_plugin_api::embed::IMPORTS);
    let module = module.to_owned();
    let waiting = Arc::<AtomicBool>::default();
    let mailbox = host.imports();
    let channel = host.spawn(
        format!("plugin {}", plugin.identity.id),
        move |host| host.load_from(&module, discovery::open).map(Editor),
        Arc::new(block_app_base::host::wake),
        Arc::clone(&waiting),
        |run| block_app_base::panic_guard::contain(run),
    )?;
    Ok((channel, mailbox, waiting))
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
        let sent = Runnable::start(&mut self.0)?;
        Ok(produced(&mut self.0, sent))
    }

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<PresentedTexture>, String> {
        let sent = Runnable::step(&mut self.0, frames)?;
        Ok(produced(&mut self.0, sent))
    }

    fn take_wake(&mut self) -> bool {
        self.0.take_wake()
    }

    fn on_wake(&self, wake: block_guest_host::Wake) {
        Runnable::on_wake(&self.0, wake);
    }

    fn stop(&mut self) {
        self.0.stop();
    }
}

fn produced(plugin: &mut Plugin, sent: Produced<Infallible>) -> Produced<PresentedTexture> {
    let (gpu, submit) = plugin.take_gpu_time();
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
        outbound: sent.outbound,
        presented,
        took: StepTime {
            gpu,
            submit,
            ..sent.took
        },
    }
}

#[cfg(test)]
mod tests;
