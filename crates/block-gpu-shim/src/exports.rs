use block_gpu_host::{
    Gpu,
    block_gpu_abi::embed::device::Caller,
};
use wasm_bindgen::JsCast;

use crate::{Shim, with};

impl Shim {
    fn guest_bytes(&self) -> Option<js_sys::Uint8Array> {
        let memory = self.memory.as_ref()?;
        Some(js_sys::Uint8Array::new(&memory.buffer()))
    }

    fn range(&self, pointer: u32, length: u32) -> Result<(js_sys::Uint8Array, u32), String> {
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
        Ok((bytes, end))
    }
}

impl Caller for Shim {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let (bytes, end) = self.range(pointer, length)?;
        Ok(bytes.subarray(pointer, end).to_vec())
    }

    fn write(&mut self, pointer: u32, data: &[u8]) -> Result<(), String> {
        let (bytes, end) = self.range(pointer, data.len() as u32)?;
        bytes.subarray(pointer, end).copy_from(data);
        Ok(())
    }

    fn host<R>(&mut self, function: &'static str, call: impl FnOnce(&mut Gpu) -> R) -> Option<R> {
        let started = crate::now();
        let result = call(&mut self.gpu);
        let took = crate::now() - started;
        self.gpu_time += took;
        if function == "submit" {
            self.submit_time += took;
        }
        Some(result)
    }

    fn refuse(&mut self, message: String) {
        self.gpu.report(message);
    }

    fn returned(&mut self) -> &mut Vec<u8> {
        &mut self.returned
    }
}

macro_rules! exported_gpu {
    ($($glue:path, $module:literal, $name:literal, $export:literal, ($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        $(
            const _: () = {
                #[unsafe(export_name = $export)]
                extern "C" fn exported($($argument: $kind),*) $(-> $result)? {
                    with(|shim| $glue(shim, $($argument),*), Default::default())
                }
            };
        )*
    };
}

block_gpu_host::block_gpu_abi::gpu_local_imports!(exported_gpu);

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn host_wake() {
    with(|shim| shim.woken = true, ())
}

pub(crate) fn memory(memory: wasm_bindgen::JsValue) -> Option<js_sys::WebAssembly::Memory> {
    memory.dyn_into().ok()
}
