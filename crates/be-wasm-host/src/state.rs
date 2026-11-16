use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use be_wasm_gpu_host::{
    Gpu, Imports,
    be_wasm_gpu_abi::embed::device::Caller,
    transport::{Transport, Transported},
};

use crate::engine;
use crate::threads::Spawner;
use crate::wake::Wake;

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

/// A guest's linear memory as the host reads and writes it.
pub(crate) enum Memory {
    /// Not known yet: a module that exports its memory has not been instantiated.
    None,
    /// A shared memory, which never moves (V8 reserves its maximum) and only grows, so a call
    /// reads where it is and how long it is without V8, which a fast-API call may not reach.
    Shared(v8::SharedRef<v8::BackingStore>),
    /// A memory of the guest's own, which may move when it grows: every call that reads it asks
    /// V8 for its buffer, so only calls made through a scope may.
    Own(v8::Global<v8::Object>),
}

impl Memory {
    pub(crate) fn shared(&self) -> Option<&v8::SharedRef<v8::BackingStore>> {
        match self {
            Self::Shared(store) => Some(store),
            Self::None | Self::Own(_) => None,
        }
    }
}

/// Where a memory's bytes are, for one call. Copying a shared memory's bytes races with the
/// guest's other threads, as under any engine: what the host reads or leaves may be torn by a
/// thread writing at once, which only the guest's own data suffers.
#[derive(Clone, Copy)]
pub(crate) struct View {
    base: *mut u8,
    size: usize,
}

impl View {
    pub(crate) const EMPTY: Self = Self {
        base: std::ptr::null_mut(),
        size: 0,
    };

    pub(crate) fn of(store: &v8::BackingStore) -> Self {
        Self {
            base: store
                .data()
                .map_or(std::ptr::null_mut(), |data| data.as_ptr().cast()),
            size: store.byte_length(),
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.size
    }

    /// Whether `length` bytes at `pointer` lie inside the memory.
    pub(crate) fn check(&self, pointer: u32, length: usize) -> Result<(), String> {
        self.range(pointer, length).map(|_| ())
    }

    fn range(&self, pointer: u32, length: usize) -> Result<std::ops::Range<usize>, String> {
        if self.base.is_null() {
            return Err(NO_MEMORY.to_owned());
        }
        let start = pointer as usize;
        let end = start
            .checked_add(length)
            .ok_or_else(|| "an app pointer overflowed".to_owned())?;
        if end > self.size {
            return Err(format!(
                "an app pointer ran past its memory: {start}..{end} of {}",
                self.size
            ));
        }
        Ok(start..end)
    }

    pub(crate) fn read(&self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let range = self.range(pointer, length as usize)?;
        let mut bytes = vec![0u8; range.len()];
        // unsafe(ffi): the range lies inside the memory, checked above, and the memory stays where
        // it is for the call. Another guest thread may write it meanwhile, as it may under any
        // engine; the bytes read are then whatever it wrote.
        unsafe {
            std::ptr::copy_nonoverlapping(self.base.add(range.start), bytes.as_mut_ptr(), range.len())
        };
        Ok(bytes)
    }

    pub(crate) fn write(&self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        let range = self.range(pointer, bytes.len())?;
        // unsafe(ffi): as in read.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.base.add(range.start), bytes.len()) };
        Ok(())
    }
}

pub struct State {
    pub(crate) memory: Memory,
    /// The `WebAssembly.Memory` on this isolate, when it is shared, which the host grows.
    pub(crate) memory_object: Option<v8::Global<v8::Object>>,
    pub(crate) device: Device,
    pub(crate) imports: Imports,
    pub(crate) error: Option<String>,
    /// Why a host call stopped the guest, which the call that ran it reports.
    pub(crate) fatal: Option<String>,
    /// The message the guest panicked with.
    pub(crate) panicked: Option<String>,
    pub(crate) returned: Vec<u8>,
    pub(crate) transport: Transport,
    pub(crate) threads: Arc<Spawner>,
    pub(crate) wake: Arc<Wake>,
    pub(crate) wasi: Arc<crate::wasi::Wasi>,
    pub(crate) gpu_time: Duration,
    pub(crate) submit_time: Duration,
    pub(crate) isolate: v8::IsolateHandle,
}

impl State {
    /// The memory as it is now, when it is shared.
    pub(crate) fn view(&self) -> View {
        self.memory.shared().map_or(View::EMPTY, |store| View::of(store))
    }

    /// The memory as it is now, whichever it is.
    pub(crate) fn view_in(&self, scope: &mut v8::PinScope<'_, '_>) -> View {
        match &self.memory {
            Memory::None => View::EMPTY,
            Memory::Shared(store) => View::of(store),
            Memory::Own(memory) => {
                let memory = v8::Local::new(scope, memory);
                engine::get(scope, memory, "buffer")
                    .and_then(|buffer| v8::Local::<v8::ArrayBuffer>::try_from(buffer).ok())
                    .map_or(View::EMPTY, |buffer| View::of(&buffer.get_backing_store()))
            }
        }
    }

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
                    "the app used the gpu, and no device could be opened: {message}"
                )),
            };
        }
        match &mut self.device {
            Device::Ready(gpu) => Ok(gpu.as_mut()),
            Device::Unavailable(message) => Err(message.clone()),
            Device::Unconnected { .. } => Err("the app's gpu was never connected".to_owned()),
        }
    }

    #[inline]
    pub(crate) fn host<R>(
        &mut self,
        function: &'static str,
        call: impl FnOnce(&mut Gpu) -> R,
    ) -> Option<R> {
        if let Device::Ready(gpu) = &mut self.device {
            if be_wasm_gpu_host::only_records(function) {
                return Some(call(gpu.as_mut()));
            }
            let started = Instant::now();
            let result = call(gpu.as_mut());
            self.took(function, started.elapsed());
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
        self.took(function, started.elapsed());
        Some(result)
    }

    fn took(&mut self, function: &str, took: Duration) {
        self.gpu_time += took;
        if function == "submit" {
            self.submit_time += took;
        }
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

/// The State behind an import's data, which is the `v8::External` every host function of an
/// isolate is made with.
///
/// # Safety
///
/// The State outlives every call of the isolate's functions: Running frees it only after its
/// isolate, and an app thread once its guest has returned, after which nothing runs on its
/// isolate (an External has no finalizer). V8 runs one call of an isolate at a time, so nothing
/// else holds the State while an import runs.
pub(crate) unsafe fn state_of<'a>(data: v8::Local<v8::Value>) -> &'a mut State {
    let external: v8::Local<v8::External> = data
        .try_into()
        .unwrap_or_else(|_| unreachable!("every host function is made with its State"));
    // unsafe(ffi): as the function's contract says.
    unsafe { &mut *external.value().cast::<State>() }
}

const NO_MEMORY: &str = "the app has no memory to read from";

pub(crate) struct Guest<'a> {
    state: &'a mut State,
    view: View,
}

impl<'a> Guest<'a> {
    pub(crate) fn new(state: &'a mut State, view: View) -> Self {
        Self { state, view }
    }
}

impl Caller for Guest<'_> {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        self.view.read(pointer, length)
    }

    fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        self.view.write(pointer, bytes)
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
