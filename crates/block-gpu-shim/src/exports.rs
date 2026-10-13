use block_gpu_abi as abi;
use block_gpu_host::{
    Gpu,
    calls::{self, Guest, Transport},
};
use wasm_bindgen::JsCast;

use crate::{Shim, with};

impl Shim {
    fn guest_bytes(&self) -> Option<js_sys::Uint8Array> {
        let memory = self.memory.as_ref()?;
        Some(js_sys::Uint8Array::new(&memory.buffer()))
    }
}

impl Guest for Shim {
    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let bytes = self
            .guest_bytes()
            .ok_or("the plugin called its host before its memory was bound")?;
        let end = pointer
            .checked_add(length)
            .ok_or_else(|| "a plugin pointer overflowed".to_owned())?;
        if end > bytes.length() {
            return Err(format!(
                "a plugin pointer ran past its memory: {pointer}..{end} of {}",
                bytes.length()
            ));
        }
        Ok(bytes.subarray(pointer, end).to_vec())
    }

    fn write(&mut self, pointer: u32, capacity: u32, data: &[u8]) -> u32 {
        let needed = data.len() as u32;
        if needed > capacity {
            return needed;
        }
        let Some(bytes) = self.guest_bytes() else {
            return 0;
        };
        if pointer.saturating_add(needed) > bytes.length() {
            return 0;
        }
        bytes.subarray(pointer, pointer + needed).copy_from(data);
        needed
    }

    fn report(&mut self, message: String) {
        self.failure.get_or_insert(message);
    }

    fn gpu<R: Default>(&mut self, call: impl FnOnce(&mut Gpu) -> R) -> R {
        call(&mut self.gpu)
    }

    fn take_error(&mut self) -> Option<String> {
        self.failure.take().or_else(|| self.gpu.take_error())
    }

    fn transport(&mut self) -> &mut Transport {
        &mut self.transport
    }
}

macro_rules! exported {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $name($($argument: $kind),*) $(-> $result)? {
                with(|shim| calls::$name(shim, $($argument),*), Default::default())
            }
        )+
    };
}

abi::gpu_functions!(exported);
abi::transport_functions!(exported);

#[unsafe(no_mangle)]
pub extern "C" fn host_wake() {
    with(|shim| shim.woken = true, ())
}

pub(crate) fn memory(memory: wasm_bindgen::JsValue) -> Option<js_sys::WebAssembly::Memory> {
    memory.dyn_into().ok()
}
