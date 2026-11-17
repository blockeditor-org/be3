use std::{
    cell::RefCell,
    convert::Infallible,
    sync::{Arc, Condvar, Mutex, PoisonError, atomic::AtomicBool},
    time::Duration,
};

use be_host_stack::discovery;

use be_sdk_manifest::AppManifest;
use be_wasm_guest_host::{
    Presented, Produced,
    thread::{self, Runnable, Threaded},
};
use be_wasm_host::{App, Host, sandbox::Staged};

use super::{
    backend::StepTime,
    wire::Wire,
    surface::{PresentedTexture, SurfaceFrame, gpu},
};

const NO_GPU: &str = "The app host has no graphics device.";

pub(super) type Frame = SurfaceFrame;

pub(super) type Channel = Threaded<PresentedTexture>;

pub(super) type Picture = wgpu::Texture;

pub(super) type Mailbox = be_wasm_host::Imports;

type Gpu = Result<(wgpu::Device, wgpu::Queue), String>;

thread_local! {
    static PREPARED: RefCell<Vec<Prepared>> = const { RefCell::new(Vec::new()) };
}

struct Prepared {
    app_id: String,
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

pub(super) fn prepare(app: &AppManifest, module: String) {
    let device = Arc::<Handover>::default();
    let handover = Arc::clone(&device);
    let started =
        Host::awaiting(move || handover.wait()).and_then(|host| start(app, &module, host));
    PREPARED.with(|prepared| {
        prepared.borrow_mut().push(Prepared {
            app_id: app.identity.id.clone(),
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
    app: &AppManifest,
    module: String,
) -> Result<(Channel, Mailbox, Arc<AtomicBool>), String> {
    let prepared = PREPARED.with(|prepared| {
        let mut prepared = prepared.borrow_mut();
        let index = prepared
            .iter()
            .position(|prepared| prepared.app_id == app.identity.id && prepared.module == module)?;
        Some(prepared.remove(index))
    });
    if let Some(mut prepared) = prepared {
        return std::mem::replace(&mut prepared.started, Err(NO_GPU.to_owned()));
    }
    let host = match gpu() {
        Some((device, queue)) => Host::new(device, queue)?,
        None => Host::on_demand(|| Err(NO_GPU.to_owned()))?,
    };
    start(app, &module, host)
}

/// The wire to `app`, which serves it the sandbox API for its workers: it may spawn only its own
/// app's workers (`be_wasm_sandbox_host::Server::for_app`), found by `staged_worker`.
pub(super) fn wire(app: &AppManifest) -> Wire {
    Wire::granting(app.extensions.clone()).with_sandbox(be_wasm_sandbox_host::Server::for_app(
        Arc::new(be_host_base::host::wake),
        app.identity.id.clone(),
        Box::new(staged_worker),
    ))
}

/// Tells be-wasm-host what every staged app imports, once, so the libraries they link are
/// trimmed to that and compiled once for all of them (be_wasm_host::keep_exports).
fn keep_staged_exports() {
    static KEPT: std::sync::Once = std::sync::Once::new();
    KEPT.call_once(|| {
        let mut names = be_lib_stable_hash::HashSet::default();
        for manifest in discovery::manifests() {
            let workers = manifest
                .workers
                .iter()
                .map(|worker| worker_path(&manifest.identity.id, worker));
            for module in std::iter::once(manifest.entry_point.clone()).chain(workers) {
                match discovery::open(&module).and_then(be_wasm_host::imported_names) {
                    Ok(imported) => names.extend(imported),
                    Err(error) => eprintln!("{module}: {error}"),
                }
            }
        }
        be_wasm_host::keep_exports(names);
    });
}

/// The module `module` of the staged app with the id `app_id`, as the sandbox spawns it
/// (`be_wasm_sandbox_host::Loader`), read with the libraries it is linked against from the staged
/// apps, as the host loads every app: the manifest's `entry_point`, or one of its `workers`, staged
/// under `workers/<app id>/`. It is the one place that maps a module's name to a file, so any
/// other name, a path or another app's module, is refused.
pub fn staged_app(app_id: &str, module: &str) -> Result<Staged, String> {
    let manifest = manifest(app_id)?;
    if module == manifest.entry_point {
        return Ok(staged(manifest.entry_point.clone()));
    }
    staged_worker(app_id, module)
}

/// One of the workers of the staged app with the id `app_id`, as `staged_app` finds it: what the
/// sandbox an app is served finds, since an app starts only its workers, never itself.
pub fn staged_worker(app_id: &str, module: &str) -> Result<Staged, String> {
    if manifest(app_id)?.workers.iter().any(|worker| worker == module) {
        return Ok(staged(worker_path(app_id, module)));
    }
    Err(format!("{app_id} stages no worker named {module:?}"))
}

fn manifest(app_id: &str) -> Result<Arc<AppManifest>, String> {
    discovery::manifests()
        .into_iter()
        .find(|manifest| manifest.identity.id == app_id)
        .ok_or_else(|| format!("no staged app has the id {app_id}"))
}

/// Where a worker module of the app `app_id` is staged (crates/be-tool-buck's stage).
fn worker_path(app_id: &str, worker: &str) -> String {
    format!("workers/{app_id}/{worker}")
}

/// A staged module, which tells be-wasm-host what every staged app imports before its first read,
/// on the thread that loads it.
fn staged(module: String) -> Staged {
    Staged::new(
        module,
        Arc::new(|name| {
            keep_staged_exports();
            discovery::open(name)
        }),
    )
}

fn start(
    app: &AppManifest,
    module: &str,
    host: Host,
) -> Result<(Channel, Mailbox, Arc<AtomicBool>), String> {
    let staged = staged(module.to_owned());
    let waiting = Arc::<AtomicBool>::default();
    let mailbox = host.imports();
    let channel = host.spawn(
        format!("app {}", app.identity.id),
        move |host| staged.load(host).map(Editor),
        Arc::new(be_host_base::host::wake),
        Arc::clone(&waiting),
        |run| be_host_base::panic_guard::contain(run),
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

struct Editor(App);

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

    fn on_wake(&self, wake: be_wasm_guest_host::Wake) {
        Runnable::on_wake(&self.0, wake);
    }

    fn stop(&mut self) {
        self.0.stop();
    }
}

fn produced(app: &mut App, sent: Produced<Infallible>) -> Produced<PresentedTexture> {
    let (gpu, submit) = app.take_gpu_time();
    let mut presented: Vec<PresentedTexture> = Vec::new();
    for surface in app.take_presented() {
        let Some((texture, generation)) = app.surface(surface) else {
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
