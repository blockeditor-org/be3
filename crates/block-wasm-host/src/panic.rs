use std::fmt;

use block_guest_abi as abi;
use wasmtime::{Caller, Linker};

use crate::threads::Spawns;

#[derive(Debug)]
struct GuestPanic(String);

impl fmt::Display for GuestPanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GuestPanic {}

#[derive(Debug)]
struct Stopped(String);

impl fmt::Display for Stopped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Stopped {}

pub(crate) fn link<T: Spawns + 'static>(linker: &mut Linker<T>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::HOST_MODULE,
            abi::HOST_PANIC,
            |caller: Caller<'_, T>, pointer: u32, length: u32| -> wasmtime::Result<()> {
                let message = match caller.data().spawner().read(pointer, length) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                    Err(error) => format!("the plugin panicked, and its message was lost: {error}"),
                };
                Err(panicked(message))
            },
        )
        .map(|_| ())
        .map_err(|error| format!("{} could not be linked: {error}", abi::HOST_PANIC))
}

pub(crate) fn panicked(message: String) -> wasmtime::Error {
    wasmtime::Error::new(GuestPanic(message))
}

pub(crate) fn stopped(message: String) -> wasmtime::Error {
    wasmtime::Error::new(Stopped(message))
}

pub(crate) fn message(error: &wasmtime::Error) -> Option<String> {
    if let Some(Stopped(message)) = error.downcast_ref::<Stopped>() {
        return Some(message.clone());
    }
    let GuestPanic(message) = error.downcast_ref::<GuestPanic>()?;
    eprintln!("block-wasm-host: a plugin panicked: {error:?}");
    Some(message.clone())
}
