use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use block_gpu_host::{
    Gpu, Imports,
    block_gpu_abi::embed::device::Caller,
    transport::{Transport, Transported},
};
use wasmtime::SharedMemory;
use wasmtime_wasi::p1::WasiP1Ctx;

use crate::threads::{Spawner, Spawns};
use crate::wake::{Wake, Wakes};

pub type Connect = Arc<dyn Fn() -> Result<(wgpu::Device, wgpu::Queue), String> + Send + Sync>;

pub(crate) enum Device {
    Ready(Box<Gpu>),
    Unconnected(Connect),
    Unavailable(String),
}

impl Device {
    pub(crate) fn ready(&self) -> Option<&Gpu> {
        match self {
            Self::Ready(gpu) => Some(gpu.as_ref()),
            Self::Unconnected(_) | Self::Unavailable(_) => None,
        }
    }

    pub(crate) fn ready_mut(&mut self) -> Option<&mut Gpu> {
        match self {
            Self::Ready(gpu) => Some(gpu.as_mut()),
            Self::Unconnected(_) | Self::Unavailable(_) => None,
        }
    }
}

pub struct State {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) memory: SharedMemory,
    pub(crate) device: Device,
    pub(crate) imports: Imports,
    pub(crate) error: Option<String>,
    pub(crate) fatal: Option<String>,
    pub(crate) returned: Vec<u8>,
    pub(crate) transport: Transport,
    pub(crate) threads: Arc<Spawner>,
    pub(crate) wake: Arc<Wake>,
    pub(crate) gpu_time: Duration,
    pub(crate) submit_time: Duration,
}

impl Drop for State {
    fn drop(&mut self) {
        self.threads.release();
    }
}

pub(crate) struct Threaded {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) threads: Arc<Spawner>,
    pub(crate) wake: Arc<Wake>,
}

impl Spawns for State {
    fn spawner(&self) -> &Arc<Spawner> {
        &self.threads
    }
}

impl Spawns for Threaded {
    fn spawner(&self) -> &Arc<Spawner> {
        &self.threads
    }
}

impl Wakes for State {
    fn wake(&self) -> &Arc<Wake> {
        &self.wake
    }
}

impl Wakes for Threaded {
    fn wake(&self) -> &Arc<Wake> {
        &self.wake
    }
}

pub(crate) fn read(memory: &SharedMemory, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
    let data = memory.data();
    let start = pointer as usize;
    let end = start
        .checked_add(length as usize)
        .ok_or_else(|| "a plugin pointer overflowed".to_owned())?;
    if end > data.len() {
        return Err(format!(
            "a plugin pointer ran past its memory: {start}..{end} of {}",
            data.len()
        ));
    }
    let mut bytes = vec![0u8; length as usize];
    for (offset, byte) in bytes.iter_mut().enumerate() {
        // unsafe(ffi)
        *byte = unsafe { *data[start + offset].get() };
    }
    Ok(bytes)
}

impl State {
    pub(crate) fn gpu(&mut self) -> Result<&mut Gpu, String> {
        if let Device::Unconnected(connect) = &self.device {
            self.device = match connect() {
                Ok((device, queue)) => Device::Ready(Box::new(
                    Gpu::new(device, queue).sharing(Arc::clone(&self.imports)),
                )),
                Err(message) => Device::Unavailable(format!(
                    "the plugin used the gpu, and no device could be opened: {message}"
                )),
            };
        }
        match &mut self.device {
            Device::Ready(gpu) => Ok(gpu.as_mut()),
            Device::Unavailable(message) => Err(message.clone()),
            Device::Unconnected(_) => Err("the plugin's gpu was never connected".to_owned()),
        }
    }

    pub(crate) fn take_error(&mut self) -> Option<String> {
        self.error
            .take()
            .or_else(|| self.device.ready_mut().and_then(Gpu::take_error))
    }
}

impl Caller for State {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        read(&self.memory, pointer, length)
    }

    fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        let data = self.memory.data();
        let start = pointer as usize;
        let end = start.saturating_add(bytes.len());
        if end > data.len() {
            return Err(format!(
                "a plugin pointer ran past its memory: {start}..{end} of {}",
                data.len()
            ));
        }
        for (offset, byte) in bytes.iter().enumerate() {
            // unsafe(ffi)
            unsafe { *data[start + offset].get() = *byte };
        }
        Ok(())
    }

    #[inline]
    fn host<R>(&mut self, function: &'static str, call: impl FnOnce(&mut Gpu) -> R) -> Option<R> {
        if let Device::Ready(gpu) = &mut self.device {
            let started = Instant::now();
            let result = call(gpu.as_mut());
            let took = started.elapsed();
            self.gpu_time += took;
            if function == "submit" {
                self.submit_time += took;
            }
            return Some(result);
        }
        let gpu = match self.gpu() {
            Ok(gpu) => gpu,
            Err(message) => {
                self.fatal = Some(message);
                return None;
            }
        };
        let started = Instant::now();
        let result = call(gpu);
        let took = started.elapsed();
        self.gpu_time += took;
        if function == "submit" {
            self.submit_time += took;
        }
        Some(result)
    }

    fn refuse(&mut self, message: String) {
        match self.device.ready_mut() {
            Some(gpu) => gpu.report(message),
            None => {
                self.error.get_or_insert(message);
            }
        }
    }

    fn returned(&mut self) -> &mut Vec<u8> {
        &mut self.returned
    }
}

impl Transported for State {
    fn transport(&mut self) -> &mut Transport {
        &mut self.transport
    }
}
