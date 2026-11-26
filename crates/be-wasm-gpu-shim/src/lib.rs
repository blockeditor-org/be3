#![cfg(target_arch = "wasm32")]

mod dylink;
mod exports;
mod screens;

use std::cell::RefCell;

use be_wasm_gpu_host::Gpu;
use wasm_bindgen::{JsCast, prelude::*};

use screens::Screens;

thread_local! {
    static SHIM: RefCell<Option<Shim>> = const { RefCell::new(None) };
    static WAKE: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

struct Shim {
    gpu: Gpu,
    screens: Screens,
    memory: Option<js_sys::WebAssembly::Memory>,
    returned: Vec<u8>,
    gpu_time: f64,
    submit_time: f64,
    failure: Option<String>,
}

impl Shim {
    fn fail(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.failure.get_or_insert(error);
        }
    }
}

fn with<R>(act: impl FnOnce(&mut Shim) -> R, absent: R) -> R {
    SHIM.with(|shim| match shim.borrow_mut().as_mut() {
        Some(shim) => act(shim),
        None => absent,
    })
}

#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("the app's gpu shim panicked: {info}").into());
    }));
    let (screens, device, queue) = Screens::open()
        .await
        .map_err(|error| JsValue::from_str(&error))?;
    let shim = Shim {
        gpu: Gpu::new(device, queue)
            .waking(std::sync::Arc::new(wake))
            .owning_device(),
        screens,
        memory: None,
        returned: Vec::new(),
        gpu_time: 0.0,
        submit_time: 0.0,
        failure: None,
    };
    SHIM.with(|current| *current.borrow_mut() = Some(shim));
    Ok(())
}

fn wake() {
    WAKE.with(|wake| {
        if let Some(wake) = wake.borrow().as_ref() {
            let _ = wake.call0(&JsValue::NULL);
        }
    });
}

/// Lets go of what the shim holds for the guest it served: its device, its canvases, the memory
/// it reads and the wake it calls, which may outlive that guest (a root linked on the page that
/// stopped). A guest linked after starts it afresh.
#[wasm_bindgen]
pub fn stop() {
    SHIM.with(|shim| shim.borrow_mut().take());
    WAKE.with(|wake| wake.borrow_mut().take());
}

#[wasm_bindgen]
pub fn bind_wake(wake: js_sys::Function) {
    WAKE.with(|held| *held.borrow_mut() = Some(wake));
}

#[wasm_bindgen]
pub fn bind(memory: JsValue) {
    with(|shim| shim.memory = exports::memory(memory), ());
}

#[wasm_bindgen]
pub fn show(id: u32, canvas: JsValue, surface: u32) {
    with(
        |shim| {
            let canvas = canvas.dyn_into::<web_sys::OffscreenCanvas>().ok();
            let shown = shim.screens.show(id, canvas, surface);
            shim.fail(shown);
        },
        (),
    );
}

#[wasm_bindgen]
pub fn layers(canvas: JsValue, width: u32, height: u32, pieces: Vec<f32>) {
    with(
        |shim| {
            let canvas = canvas.dyn_into::<web_sys::OffscreenCanvas>().ok();
            let placed = shim.screens.layers(canvas, [width, height], &pieces);
            shim.fail(placed);
        },
        (),
    );
}

#[wasm_bindgen]
pub fn forget(id: u32) {
    with(|shim| shim.screens.forget(id), ());
}

#[wasm_bindgen]
pub fn paint() -> bool {
    with(
        |shim| {
            for error in shim.gpu.take_logged() {
                web_sys::console::warn_1(&format!("an app's uncaught gpu error: {error}").into());
            }
            let presented = shim.gpu.take_presented();
            shim.screens.presented(&presented);
            let gpu = &shim.gpu;
            let painted = shim.screens.paint(|surface| gpu.surface(surface));
            let exported = shim
                .screens
                .paint_exports(&presented, |surface| gpu.surface(surface));
            shim.fail(painted);
            shim.fail(exported);
            !presented.is_empty()
        },
        false,
    )
}

/// The surfaces the app let go of since the last call, for the page, which keeps a canvas of a
/// surface its app still holds while no layer shows it.
#[wasm_bindgen]
pub fn released() -> Vec<u32> {
    with(|shim| shim.gpu.take_released(), Vec::new())
}

#[wasm_bindgen]
pub fn export_surfaces(surfaces: Vec<u32>) {
    with(|shim| shim.screens.export(surfaces), ());
}

#[wasm_bindgen]
pub fn exported() -> js_sys::Array {
    let pictures = js_sys::Array::new();
    with(
        |shim| {
            for (surface, bitmap) in shim.screens.take_exported() {
                let picture = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&picture, &"surface".into(), &surface.into());
                let _ = js_sys::Reflect::set(&picture, &"bitmap".into(), &bitmap);
                pictures.push(&picture);
            }
        },
        (),
    );
    pictures
}

#[wasm_bindgen]
pub fn import_image(image: u64, bitmap: web_sys::ImageBitmap) {
    with(
        |shim| {
            let texture = shim.screens.import(&bitmap);
            bitmap.close();
            shim.gpu
                .imports()
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .insert(image, texture);
        },
        (),
    );
}

#[wasm_bindgen]
pub fn forget_image(image: u64) {
    with(
        |shim| {
            shim.gpu
                .imports()
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .remove(&image);
        },
        (),
    );
}

#[wasm_bindgen]
pub fn took() -> Vec<f64> {
    with(
        |shim| {
            vec![
                std::mem::take(&mut shim.gpu_time),
                std::mem::take(&mut shim.submit_time),
            ]
        },
        vec![0.0, 0.0],
    )
}

#[wasm_bindgen]
pub fn failure() -> Option<String> {
    with(
        |shim| shim.failure.take().or_else(|| shim.gpu.take_error()),
        None,
    )
}

/// The global's clock, in a worker as on the page (a root's shim runs on the page).
fn now() -> f64 {
    js_sys::Reflect::get(&js_sys::global(), &"performance".into())
        .ok()
        .and_then(|performance| performance.dyn_into::<web_sys::Performance>().ok())
        .map_or(0.0, |performance| performance.now())
}
