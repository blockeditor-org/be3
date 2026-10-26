// @yeskeepfile: the comments below are the invariants the FFI relies on, which
// nothing here can test (no Windows machine runs these crates' tests).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::JoinHandle;

use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HWND, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_FENCE_FLAG_NONE, D3D11_TEXTURE2D_DESC, ID3D11Fence, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::Threading::{INFINITE, WaitForMultipleObjects};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::Interface;

use crate::gpu::{Device, Release, Texture};
use crate::handle::WinEvent;
use crate::{Event, Lost, Pick, Ring, Size, Target, copied, pool_after};

// The latest frame, the one a consumer holds, and one to write into.
const SLOTS: usize = 3;
const BUFFERS: i32 = 2;
const FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;

type Notify = Box<dyn Fn(Event) + Send + Sync>;

pub struct Capture {
    shared: Arc<Shared>,
    stop: Arc<WinEvent>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    device: Device,
    // Set by Windows' handlers (a frame arrived, the item closed), by a dropped
    // frame and by a release fence reaching what the thread waits for.
    wake: Arc<WinEvent>,
    state: Mutex<State>,
    version: AtomicU64,
    lost: Mutex<Option<Lost>>,
}

struct State {
    ring: Ring,
    releases: Vec<Release>,
    // A slot's texture is taken out while the thread writes it, which only
    // happens to a slot that is neither the latest nor held.
    textures: Vec<Option<Texture>>,
    version: u64,
}

#[derive(Clone)]
pub struct Frame(Arc<Held>);

struct Held {
    shared: Arc<Shared>,
    slot: usize,
    texture: wgpu::Texture,
    size: Size,
    version: u64,
}

impl Frame {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.0.texture
    }

    pub fn size(&self) -> Size {
        self.0.size
    }

    pub fn version(&self) -> u64 {
        self.0.version
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.shared.released(self.slot);
    }
}

fn failed(what: &'static str) -> impl FnOnce(windows::core::Error) -> String {
    move |error| format!("{what}: {error}")
}

// unsafe(ffi)
fn completed(state: &State) -> Vec<u64> {
    state
        .releases
        .iter()
        .map(|release| unsafe { release.fence.GetCompletedValue() })
        .collect()
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // unsafe(ffi)
    fn released(&self, slot: usize) {
        let mut state = self.lock();
        let completed = unsafe { state.releases[slot].fence.GetCompletedValue() };
        if let Some(value) = state.ring.release(slot, completed) {
            self.device.0.signal(state.releases[slot].semaphore, value);
        }
        drop(state);
        self.wake.set();
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        let releases = std::mem::take(&mut state.releases);
        for (slot, release) in releases.into_iter().enumerate() {
            let value = state.ring.release_value(slot);
            self.device.0.retire(release, value);
        }
    }
}

impl Capture {
    pub fn start(
        device: &Device,
        target: Target,
        notify: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Capture, String> {
        let mut releases = Vec::with_capacity(SLOTS);
        for _ in 0..SLOTS {
            match device.0.release() {
                Ok(release) => releases.push(release),
                Err(error) => {
                    for release in releases {
                        device.0.retire(release, 0);
                    }
                    return Err(error);
                }
            }
        }
        let wake = match WinEvent::new(false) {
            Ok(wake) => Arc::new(wake),
            Err(error) => {
                for release in releases {
                    device.0.retire(release, 0);
                }
                return Err(error);
            }
        };
        let shared = Arc::new(Shared {
            device: device.clone(),
            wake,
            state: Mutex::new(State {
                ring: Ring::new(SLOTS),
                releases,
                textures: (0..SLOTS).map(|_| None).collect(),
                version: 0,
            }),
            version: AtomicU64::new(0),
            lost: Mutex::new(None),
        });
        // Manual reset, so both of the thread's waits see it.
        let stop = Arc::new(WinEvent::new(true)?);
        let (started, starting) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("window capture".to_owned())
            .spawn({
                let shared = shared.clone();
                let stop = stop.clone();
                move || run(shared, stop, target, Box::new(notify), started)
            })
            .map_err(|error| format!("the capture thread could not start: {error}"))?;
        match starting.recv() {
            Ok(Ok(())) => Ok(Capture {
                shared,
                stop,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err("the capture thread ended before it started".to_owned())
            }
        }
    }

    pub fn latest(&self) -> Option<Frame> {
        let mut state = self.shared.lock();
        let slot = state.ring.latest()?;
        let texture = state.textures[slot].as_ref()?;
        let (texture, size) = (texture.wgpu.clone(), texture.size);
        let version = state.version;
        state.ring.hold_latest();
        drop(state);
        Some(Frame(Arc::new(Held {
            shared: self.shared.clone(),
            slot,
            texture,
            size,
            version,
        })))
    }

    pub fn version(&self) -> u64 {
        self.shared.version.load(Ordering::Acquire)
    }

    pub fn lost(&self) -> Option<Lost> {
        self.shared
            .lost
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.set();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// unsafe(ffi)
fn run(
    shared: Arc<Shared>,
    stop: Arc<WinEvent>,
    target: Target,
    notify: Notify,
    started: mpsc::Sender<Result<(), String>>,
) {
    // Windows.Graphics.Capture's objects live on this thread alone, in the
    // multithreaded apartment; the frame pool is free-threaded, so its events
    // come on Windows' own threads and only set `wake`.
    let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
    match Session::start(&shared, target) {
        Ok(mut session) => {
            let _ = started.send(Ok(()));
            let ended = session.run(&shared, &stop, &notify);
            drop(session);
            if let Some(lost) = ended {
                *shared.lost.lock().unwrap_or_else(PoisonError::into_inner) = Some(lost);
                notify(Event::Lost);
            }
        }
        Err(error) => {
            let _ = started.send(Err(error));
        }
    }
    if initialized {
        unsafe { RoUninitialize() };
    }
}

struct Session {
    item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    winrt: IDirect3DDevice,
    arrived: i64,
    closing: i64,
    closed: Arc<AtomicBool>,
    pool_size: Size,
    pending: Option<Direct3D11CaptureFrame>,
    copied: ID3D11Fence,
    copies: u64,
    copy_done: WinEvent,
    registered: Vec<u64>,
}

enum Delivery {
    Nothing,
    Waiting,
    Published,
    Stopped,
}

fn size_int(size: Size) -> SizeInt32 {
    SizeInt32 {
        Width: size.width as i32,
        Height: size.height as i32,
    }
}

// unsafe(ffi)
impl Session {
    fn start(shared: &Shared, target: Target) -> Result<Session, String> {
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err("this version of Windows cannot capture windows".to_owned());
        }
        let device = &shared.device.0;
        let mut copied: Option<ID3D11Fence> = None;
        unsafe {
            device
                .d3d
                .CreateFence(0, D3D11_FENCE_FLAG_NONE, &mut copied)
        }
        .map_err(failed("a fence could not be created"))?;
        let copied = copied.ok_or("Direct3D 11 made no fence")?;
        let copy_done = WinEvent::new(false)?;
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(failed("Windows.Graphics.Capture did not start"))?;
        // The handles are the caller's; Windows checks them, failing for one
        // that names no monitor or window.
        let item: GraphicsCaptureItem = match target {
            Target::Monitor(handle) => unsafe {
                interop.CreateForMonitor(HMONITOR(handle as *mut c_void))
            },
            Target::Window(handle) => unsafe {
                interop.CreateForWindow(HWND(handle as *mut c_void))
            },
        }
        .map_err(failed("the monitor or window cannot be captured"))?;
        let dxgi: IDXGIDevice = device
            .d3d
            .cast()
            .map_err(failed("the Direct3D device is not a DXGI device"))?;
        let winrt: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
            .and_then(|device| device.cast())
            .map_err(failed("the Direct3D device could not be handed to WinRT"))?;
        let size = item.Size().map_err(failed("the item has no size"))?;
        let pool_size = Size::of(size.Width, size.Height).unwrap_or(Size {
            width: 1,
            height: 1,
        });
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt,
            FORMAT,
            BUFFERS,
            size_int(pool_size),
        )
        .map_err(failed("the frame pool could not be created"))?;
        let wake = shared.wake.clone();
        let arrived = pool
            .FrameArrived(&TypedEventHandler::new(move |_, _| {
                wake.set();
                Ok(())
            }))
            .map_err(failed("the frame pool cannot report frames"))?;
        let closed = Arc::new(AtomicBool::new(false));
        let wake = shared.wake.clone();
        let flag = closed.clone();
        let closing = match item.Closed(&TypedEventHandler::new(move |_, _| {
            flag.store(true, Ordering::Release);
            wake.set();
            Ok(())
        })) {
            Ok(token) => token,
            Err(error) => {
                let _ = pool.RemoveFrameArrived(arrived);
                let _ = pool.Close();
                return Err(failed("the item cannot report closing")(error));
            }
        };
        let session = match pool.CreateCaptureSession(&item) {
            Ok(session) => session,
            Err(error) => {
                let _ = item.RemoveClosed(closing);
                let _ = pool.RemoveFrameArrived(arrived);
                let _ = pool.Close();
                return Err(failed("the capture session could not be created")(error));
            }
        };
        let session = Session {
            item,
            pool,
            session,
            winrt,
            arrived,
            closing,
            closed,
            pool_size,
            pending: None,
            copied,
            copies: 0,
            copy_done,
            registered: vec![0; SLOTS],
        };
        // Both are newer than capture itself (Windows 10 2004 and Windows 11);
        // where they are missing the border and cursor stay, which is all.
        let _ = session.session.SetIsBorderRequired(false);
        let _ = session.session.SetIsCursorCaptureEnabled(false);
        session
            .session
            .StartCapture()
            .map_err(failed("the capture did not start"))?;
        Ok(session)
    }

    fn run(&mut self, shared: &Shared, stop: &WinEvent, notify: &Notify) -> Option<Lost> {
        let handles = [stop.raw(), shared.wake.raw()];
        loop {
            let woke = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
            if woke == WAIT_OBJECT_0 {
                return None;
            }
            if woke.0 != WAIT_OBJECT_0.0 + 1 {
                return Some(Lost::Failed("waiting for frames failed".to_owned()));
            }
            if self.closed.load(Ordering::Acquire) {
                return Some(Lost::Closed);
            }
            // Only the newest frame matters; closing the others hands their
            // buffers back to the pool.
            while let Ok(frame) = self.pool.TryGetNextFrame() {
                if let Some(older) = self.pending.replace(frame) {
                    let _ = older.Close();
                }
            }
            match self.deliver(shared, stop) {
                Ok(Delivery::Published) => notify(Event::Frame),
                Ok(Delivery::Nothing | Delivery::Waiting) => {}
                Ok(Delivery::Stopped) => return None,
                Err(error) => return Some(Lost::Failed(error)),
            }
        }
    }

    fn deliver(&mut self, shared: &Shared, stop: &WinEvent) -> Result<Delivery, String> {
        let Some(frame) = self.pending.as_ref() else {
            return Ok(Delivery::Nothing);
        };
        let content = frame
            .ContentSize()
            .map_err(failed("the frame has no size"))?;
        let Some(content) = Size::of(content.Width, content.Height) else {
            if let Some(frame) = self.pending.take() {
                let _ = frame.Close();
            }
            return Ok(Delivery::Nothing);
        };
        // The surface is the pool's buffer, as large as the pool when it was
        // made; the content is its top left corner and may be larger or
        // smaller, until the pool is recreated at the content's size.
        let source: ID3D11Texture2D = frame
            .Surface()
            .and_then(|surface| surface.cast::<IDirect3DDxgiInterfaceAccess>())
            .and_then(|access| unsafe { access.GetInterface() })
            .map_err(failed("the frame has no texture"))?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };
        let size = copied(
            content,
            Size {
                width: desc.Width,
                height: desc.Height,
            },
        );
        let mut state = shared.lock();
        let completed = completed(&state);
        for (slot, value) in state.ring.settle(&completed) {
            shared
                .device
                .0
                .signal(state.releases[slot].semaphore, value);
        }
        let slot = match state.ring.pick(size, &completed) {
            Pick::Write(slot) => slot,
            Pick::Wait(waits) => {
                // Each registration sets `wake` once, when Vulkan's signal
                // reaches the value; one per value is enough.
                for (slot, value) in waits {
                    if self.registered[slot] != value {
                        unsafe {
                            state.releases[slot]
                                .fence
                                .SetEventOnCompletion(value, shared.wake.raw())
                        }
                        .map_err(failed("the release fence cannot be waited for"))?;
                        self.registered[slot] = value;
                    }
                }
                return Ok(Delivery::Waiting);
            }
            Pick::Busy => return Ok(Delivery::Waiting),
        };
        let current = state.textures[slot].take();
        drop(state);
        let written = self.copy(shared, stop, current, &source, size);
        let mut state = shared.lock();
        let texture = match written {
            Ok(Some(texture)) => texture,
            Ok(None) => {
                state.ring.abandon(slot);
                return Ok(Delivery::Stopped);
            }
            Err(error) => {
                state.ring.abandon(slot);
                return Err(error);
            }
        };
        state.textures[slot] = Some(texture);
        state.ring.written(slot, size);
        state.version = shared.version.fetch_add(1, Ordering::AcqRel) + 1;
        drop(state);
        if let Some(frame) = self.pending.take() {
            let _ = frame.Close();
        }
        if let Some(resized) = pool_after(self.pool_size, Some(content)) {
            self.pool
                .Recreate(&self.winrt, FORMAT, BUFFERS, size_int(resized))
                .map_err(failed("the frame pool could not be resized"))?;
            self.pool_size = resized;
        }
        Ok(Delivery::Published)
    }

    // Copies into the slot's texture (made anew at another size) and waits on
    // this thread until the copy has finished on the GPU: wgpu has no way to
    // make its submissions wait for a semaphore, so the frame is published
    // only once its pixels are there. None when stopped meanwhile.
    fn copy(
        &mut self,
        shared: &Shared,
        stop: &WinEvent,
        current: Option<Texture>,
        source: &ID3D11Texture2D,
        size: Size,
    ) -> Result<Option<Texture>, String> {
        let device = &shared.device.0;
        let texture = match current {
            Some(texture) if texture.size == size => texture,
            _ => device.texture(size)?,
        };
        let region = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: size.width,
            bottom: size.height,
            back: 1,
        };
        // B8G8R8A8_UNORM into B8G8R8A8_UNORM_SRGB is a copy within one format
        // family, which CopySubresourceRegion allows.
        unsafe {
            device
                .context
                .CopySubresourceRegion(&texture.d3d, 0, 0, 0, 0, source, 0, Some(&region))
        };
        self.copies += 1;
        unsafe { device.context.Signal(&self.copied, self.copies) }
            .map_err(failed("the copy could not be fenced"))?;
        unsafe { device.context.Flush() };
        unsafe {
            self.copied
                .SetEventOnCompletion(self.copies, self.copy_done.raw())
        }
        .map_err(failed("the copy cannot be waited for"))?;
        let handles = [stop.raw(), self.copy_done.raw()];
        let woke = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if woke == WAIT_OBJECT_0 {
            return Ok(None);
        }
        if woke.0 != WAIT_OBJECT_0.0 + 1 {
            return Err("waiting for the copy failed".to_owned());
        }
        Ok(Some(texture))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(frame) = self.pending.take() {
            let _ = frame.Close();
        }
        let _ = self.pool.RemoveFrameArrived(self.arrived);
        let _ = self.item.RemoveClosed(self.closing);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
