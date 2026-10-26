#![cfg(target_arch = "wasm32")]

mod dylink;
mod exports;
mod screens;

use std::cell::RefCell;

use block_gpu_host::Gpu;
use wasm_bindgen::{JsCast, prelude::*};

use screens::Screens;

thread_local! {
    static SHIM: RefCell<Option<Shim>> = const { RefCell::new(None) };
}

struct Shim {
    gpu: Gpu,
    screens: Screens,
    memory: Option<js_sys::WebAssembly::Memory>,
    returned: Vec<u8>,
    gpu_time: f64,
    submit_time: f64,
    woken: bool,
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
        web_sys::console::error_1(&format!("the plugin's gpu shim panicked: {info}").into());
    }));
    let (screens, device, queue) = Screens::open()
        .await
        .map_err(|error| JsValue::from_str(&error))?;
    let shim = Shim {
        gpu: Gpu::new(device, queue),
        screens,
        memory: None,
        returned: Vec::new(),
        gpu_time: 0.0,
        submit_time: 0.0,
        woken: false,
        failure: None,
    };
    SHIM.with(|current| *current.borrow_mut() = Some(shim));
    Ok(())
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
pub fn forget(id: u32) {
    with(|shim| shim.screens.forget(id), ());
}

#[wasm_bindgen]
pub fn paint() -> bool {
    with(
        |shim| {
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
pub fn woken() -> bool {
    with(|shim| std::mem::take(&mut shim.woken), false)
}

#[wasm_bindgen]
pub fn failure() -> Option<String> {
    with(
        |shim| shim.failure.take().or_else(|| shim.gpu.take_error()),
        None,
    )
}

fn now() -> f64 {
    js_sys::global()
        .dyn_into::<web_sys::WorkerGlobalScope>()
        .ok()
        .and_then(|scope| scope.performance())
        .map_or(0.0, |performance| performance.now())
}
