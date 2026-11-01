use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use block_gpu_host::{
    Gpu, Imports,
    block_gpu_abi::embed::device::Caller,
    transport::{Transport, Transported},
};
use wasmtime::{AsContextMut, Caller as Calling, Extern, SharedMemory, Store, StoreLimits};
use wasmtime_wasi::p1::WasiP1Ctx;

use crate::threads::{Spawner, Spawns};
use crate::wake::{Wake, Wakes};

pub type Connect = Arc<dyn Fn() -> Result<(wgpu::Device, wgpu::Queue), String> + Send + Sync>;

pub(crate) enum Device {
    Ready(Box<Gpu>),
    Unconnected { connect: Connect, wakes: bool },
    Unavailable(String),
}

impl Device {
    pub(crate) fn ready(&self) -> Option<&Gpu> {
        match self {
            Self::Ready(gpu) => Some(gpu.as_ref()),
            Self::Unconnected { .. } | Self::Unavailable(_) => None,
        }
    }

    pub(crate) fn ready_mut(&mut self) -> Option<&mut Gpu> {
        match self {
            Self::Ready(gpu) => Some(gpu.as_mut()),
            Self::Unconnected { .. } | Self::Unavailable(_) => None,
        }
    }
}

pub(crate) enum Memory {
    Shared(SharedMemory),
    Own(Option<wasmtime::Memory>),
}

pub struct State {
    pub(crate) wasi: WasiP1Ctx,
    pub(crate) memory: Memory,
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
    pub(crate) limits: StoreLimits,
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
        if let Device::Unconnected { connect, wakes } = &self.device {
            self.device = match connect() {
                Ok((device, queue)) => {
                    let gpu = Gpu::new(device, queue).sharing(Arc::clone(&self.imports));
                    let wake = Arc::clone(&self.wake);
                    Device::Ready(Box::new(match wakes {
                        true => gpu.waking(Arc::new(move || wake.wake())),
                        false => gpu,
                    }))
                }
                Err(message) => Device::Unavailable(format!(
                    "the plugin used the gpu, and no device could be opened: {message}"
                )),
            };
        }
        match &mut self.device {
            Device::Ready(gpu) => Ok(gpu.as_mut()),
            Device::Unavailable(message) => Err(message.clone()),
            Device::Unconnected { .. } => Err("the plugin's gpu was never connected".to_owned()),
        }
    }

    #[inline]
    pub(crate) fn host<R>(
        &mut self,
        function: &'static str,
        call: impl FnOnce(&mut Gpu) -> R,
    ) -> Option<R> {
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

    pub(crate) fn refuse(&mut self, message: String) {
        match self.device.ready_mut() {
            Some(gpu) => gpu.report(message),
            None => {
                self.error.get_or_insert(message);
            }
        }
    }

    pub(crate) fn take_error(&mut self) -> Option<String> {
        self.error
            .take()
            .or_else(|| self.device.ready_mut().and_then(Gpu::take_error))
    }
}

pub(crate) trait Holds: AsContextMut<Data = State> {
    fn state(&mut self) -> &mut State;
}

impl Holds for Store<State> {
    fn state(&mut self) -> &mut State {
        self.data_mut()
    }
}

impl Holds for Calling<'_, State> {
    fn state(&mut self) -> &mut State {
        self.data_mut()
    }
}

pub(crate) struct Guest<'a> {
    state: &'a mut State,
    own: Option<&'a mut [u8]>,
}

impl<'a> Guest<'a> {
    pub(crate) fn of(context: &'a mut impl Holds) -> Self {
        let own = match context.as_context().data().memory {
            Memory::Own(memory) => memory,
            Memory::Shared(_) => None,
        };
        match own {
            Some(memory) => {
                let (own, state) = memory.data_and_store_mut(context);
                Self {
                    state,
                    own: Some(own),
                }
            }
            None => Self {
                state: context.state(),
                own: None,
            },
        }
    }

    pub(crate) fn calling(caller: &'a mut Calling<'_, State>) -> Self {
        if let Memory::Own(None) = caller.data().memory
            && let Some(Extern::Memory(memory)) = caller.get_export(MEMORY_EXPORT)
        {
            caller.data_mut().memory = Memory::Own(Some(memory));
        }
        Self::of(caller)
    }

    pub(crate) fn state(&mut self) -> &mut State {
        self.state
    }

    fn range(&self, pointer: u32, length: usize) -> Result<std::ops::Range<usize>, String> {
        let size = match (&self.own, &self.state.memory) {
            (Some(own), _) => own.len(),
            (None, Memory::Shared(shared)) => shared.data().len(),
            (None, Memory::Own(_)) => return Err(NO_MEMORY.to_owned()),
        };
        let start = pointer as usize;
        let end = start
            .checked_add(length)
            .ok_or_else(|| "a plugin pointer overflowed".to_owned())?;
        if end > size {
            return Err(format!(
                "a plugin pointer ran past its memory: {start}..{end} of {size}"
            ));
        }
        Ok(start..end)
    }
}

pub(crate) trait Reads: Sized {
    fn read(caller: &mut Calling<'_, Self>, pointer: u32, length: u32) -> Result<Vec<u8>, String>;
}

impl Reads for State {
    fn read(caller: &mut Calling<'_, Self>, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        Guest::calling(caller).read(pointer, length)
    }
}

impl Reads for Threaded {
    fn read(caller: &mut Calling<'_, Self>, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        caller.data().threads.read(pointer, length)
    }
}

const MEMORY_EXPORT: &str = "memory";
const NO_MEMORY: &str = "the plugin has no memory to read from";

impl Caller for Guest<'_> {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let range = self.range(pointer, length as usize)?;
        match (&self.own, &self.state.memory) {
            (Some(own), _) => Ok(own[range].to_vec()),
            (None, Memory::Shared(shared)) => read(shared, pointer, length),
            (None, Memory::Own(_)) => Err(NO_MEMORY.to_owned()),
        }
    }

    fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        let range = self.range(pointer, bytes.len())?;
        match (&mut self.own, &self.state.memory) {
            (Some(own), _) => own[range].copy_from_slice(bytes),
            (None, Memory::Shared(shared)) => {
                for (cell, byte) in shared.data()[range].iter().zip(bytes) {
                    // unsafe(ffi)
                    unsafe { *cell.get() = *byte };
                }
            }
            (None, Memory::Own(_)) => return Err(NO_MEMORY.to_owned()),
        }
        Ok(())
    }

    #[inline]
    fn host<R>(&mut self, function: &'static str, call: impl FnOnce(&mut Gpu) -> R) -> Option<R> {
        self.state.host(function, call)
    }

    fn refuse(&mut self, message: String) {
        self.state.refuse(message);
    }

    fn returned(&mut self) -> &mut Vec<u8> {
        &mut self.state.returned
    }
}

impl Transported for Guest<'_> {
    fn transport(&mut self) -> &mut Transport {
        &mut self.state.transport
    }
}
