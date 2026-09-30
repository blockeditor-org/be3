#![cfg(target_arch = "wasm32")]

mod exports;
mod screens;

use std::{cell::RefCell, collections::VecDeque};

use block_gpu_host::Gpu;
use wasm_bindgen::{JsCast, prelude::*};

use screens::Screens;

const SCREENS_SURFACE: u32 = 0;

thread_local! {
    static SHIM: RefCell<Option<Shim>> = const { RefCell::new(None) };
}

struct Shim {
    gpu: Gpu,
    screens: Screens,
    inbox: VecDeque<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
    scratch: Vec<u8>,
    started: f64,
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
        inbox: VecDeque::new(),
        outbox: Vec::new(),
        scratch: Vec::new(),
        started: now(),
        woken: false,
        failure: None,
    };
    SHIM.with(|current| *current.borrow_mut() = Some(shim));
    Ok(())
}

#[wasm_bindgen]
pub fn show(id: u32, canvas: JsValue, x: u32, y: u32, width: u32, height: u32) {
    with(
        |shim| {
            let canvas = canvas.dyn_into::<web_sys::OffscreenCanvas>().ok();
            let shown = shim.screens.show(id, canvas, [x, y, width, height]);
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
pub fn paint() {
    with(
        |shim| {
            if shim.gpu.take_presented().contains(&SCREENS_SURFACE) {
                shim.screens.presented();
            }
            let painted = shim.screens.paint(shim.gpu.surface(SCREENS_SURFACE));
            shim.fail(painted);
        },
        (),
    );
}

#[wasm_bindgen]
pub fn deliver(frame: &[u8]) {
    with(|shim| shim.inbox.push_back(frame.to_vec()), ());
}

#[wasm_bindgen]
pub fn collect() -> js_sys::Array {
    let frames = js_sys::Array::new();
    with(
        |shim| {
            for frame in std::mem::take(&mut shim.outbox) {
                frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
            }
        },
        (),
    );
    frames
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
    js_sys::Date::now()
}
