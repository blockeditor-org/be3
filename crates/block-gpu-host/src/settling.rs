use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::Waker;

use block_gpu_abi::device as abi;

pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug, PartialEq)]
enum State {
    Pending,
    Mapped { range: Range<u64>, writable: bool },
}

#[derive(Default)]
struct Held {
    states: HashMap<u32, State>,
    work: HashMap<u64, bool>,
    tickets: u64,
    pending: usize,
    lost: Option<String>,
    stopped: bool,
}

#[derive(Default)]
pub(crate) struct Shared {
    held: Mutex<Held>,
    changed: Condvar,
}

pub(crate) struct Settling {
    shared: Arc<Shared>,
    wake: Option<Wake>,
    #[cfg(not(target_arch = "wasm32"))]
    poller: Option<std::thread::JoinHandle<()>>,
}

impl Shared {
    fn held(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|held| held.into_inner())
    }
}

impl Settling {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::default(),
            wake: None,
            #[cfg(not(target_arch = "wasm32"))]
            poller: None,
        }
    }

    pub(crate) fn waking(&mut self, wake: Wake) {
        self.wake = Some(wake);
    }

    pub(crate) fn wake(&self) {
        if let Some(wake) = &self.wake {
            wake();
        }
    }

    pub(crate) fn waker(&self) -> Waker {
        Waker::from(Arc::new(Waking(self.wake.clone())))
    }

    pub(crate) fn losing(&self) -> impl Fn(wgpu::DeviceLostReason, String) + Send + Sync + 'static {
        let shared = Arc::clone(&self.shared);
        let wake = self.wake.clone();
        move |reason, message| {
            let message = match (reason, message.is_empty()) {
                (_, false) => message,
                (wgpu::DeviceLostReason::Destroyed, true) => {
                    "the host's gpu device was destroyed".to_owned()
                }
                (_, true) => "the host's gpu device was lost".to_owned(),
            };
            shared.held().lost.get_or_insert(message);
            if let Some(wake) = &wake {
                wake();
            }
        }
    }

    pub(crate) fn lost(&self) -> Option<String> {
        self.shared.held().lost.clone()
    }

    pub(crate) fn work_done(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> u64 {
        let ticket = {
            let mut held = self.shared.held();
            held.tickets += 1;
            let ticket = held.tickets;
            held.work.insert(ticket, false);
            held.pending += 1;
            ticket
        };
        let shared = Arc::clone(&self.shared);
        let wake = self.wake.clone();
        queue.on_submitted_work_done(move || {
            {
                let mut held = shared.held();
                if let Some(done) = held.work.get_mut(&ticket) {
                    *done = true;
                    held.pending -= 1;
                }
            }
            if let Some(wake) = wake {
                wake();
            }
        });
        self.poll_in_background(device);
        ticket
    }

    pub(crate) fn is_work_done(&mut self, ticket: u64) -> Result<bool, String> {
        let mut held = self.shared.held();
        match held.work.get(&ticket) {
            None => Err(format!("no submitted work is awaited under ticket {ticket}")),
            Some(false) => Ok(false),
            Some(true) => {
                held.work.remove(&ticket);
                Ok(true)
            }
        }
    }

    pub(crate) fn forget_every_map(&mut self) {
        let mut held = self.shared.held();
        let pending = held
            .states
            .values()
            .filter(|state| **state == State::Pending)
            .count();
        held.pending -= pending;
        held.states.clear();
    }

    pub(crate) fn mapped_at_creation(&mut self, buffer: u32, size: u64) {
        self.shared.held().states.insert(
            buffer,
            State::Mapped {
                range: 0..size,
                writable: true,
            },
        );
    }

    pub(crate) fn state(&self, buffer: u32) -> abi::MapState {
        match self.shared.held().states.get(&buffer) {
            None => abi::MapState::Unmapped,
            Some(State::Pending) => abi::MapState::Pending,
            Some(State::Mapped { .. }) => abi::MapState::Mapped,
        }
    }

    pub(crate) fn map(
        &mut self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
        handle: u32,
        mode: abi::MapMode,
        range: Range<u64>,
    ) -> Result<(), String> {
        {
            let mut held = self.shared.held();
            if held.states.contains_key(&handle) {
                return Err(format!(
                    "buffer {handle} was asked to map while it is mapped or mapping"
                ));
            }
            held.states.insert(handle, State::Pending);
            held.pending += 1;
        }
        let shared = Arc::clone(&self.shared);
        let wake = self.wake.clone();
        let writable = mode == abi::MapMode::Write;
        let mapped = range.clone();
        let mode = match mode {
            abi::MapMode::Read => wgpu::MapMode::Read,
            abi::MapMode::Write => wgpu::MapMode::Write,
        };
        buffer.map_async(mode, range, move |result| {
            {
                let mut held = shared.held();
                if held.states.get(&handle) == Some(&State::Pending) {
                    held.pending -= 1;
                    match result {
                        Ok(()) => held.states.insert(
                            handle,
                            State::Mapped {
                                range: mapped,
                                writable,
                            },
                        ),
                        Err(_) => held.states.remove(&handle),
                    };
                }
            }
            if let Some(wake) = wake {
                wake();
            }
        });
        self.poll_in_background(device);
        Ok(())
    }

    pub(crate) fn mapped(
        &self,
        buffer: u32,
        range: &Range<u64>,
        writing: bool,
    ) -> Result<(), String> {
        let held = self.shared.held();
        let Some(State::Mapped {
            range: mapped,
            writable,
        }) = held.states.get(&buffer)
        else {
            return Err(format!("buffer {buffer} is not mapped"));
        };
        if writing && !writable {
            return Err(format!(
                "buffer {buffer} is mapped for reading, not writing"
            ));
        }
        if range.start < mapped.start || range.end > mapped.end || range.start > range.end {
            return Err(format!(
                "{}..{} is outside the range {}..{} buffer {buffer} has mapped",
                range.start, range.end, mapped.start, mapped.end
            ));
        }
        if range.start % wgpu::MAP_ALIGNMENT != 0 || (range.end - range.start) % 4 != 0 {
            return Err(format!(
                "a mapped range starts at a multiple of {} and is a multiple of 4 long, not {}..{}",
                wgpu::MAP_ALIGNMENT,
                range.start,
                range.end
            ));
        }
        Ok(())
    }

    pub(crate) fn forget(&mut self, buffer: u32) {
        let mut held = self.shared.held();
        if held.states.remove(&buffer) == Some(State::Pending) {
            held.pending -= 1;
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_in_background(&mut self, _device: &wgpu::Device) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_in_background(&mut self, device: &wgpu::Device) {
        self.shared.changed.notify_all();
        if self.poller.is_some() {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let device = device.clone();
        self.poller = std::thread::Builder::new()
            .name("plugin gpu maps".into())
            .spawn(move || {
                loop {
                    {
                        let mut held = shared.held();
                        while held.pending == 0 && !held.stopped {
                            held = shared
                                .changed
                                .wait(held)
                                .unwrap_or_else(|held| held.into_inner());
                        }
                        if held.stopped {
                            return;
                        }
                    }
                    let _ = device.poll(wgpu::PollType::wait_indefinitely());
                }
            })
            .ok();
    }
}

struct Waking(Option<Wake>);

impl std::task::Wake for Waking {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(wake) = &self.0 {
            wake();
        }
    }
}

impl Drop for Settling {
    fn drop(&mut self) {
        self.shared.held().stopped = true;
        self.shared.changed.notify_all();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(poller) = self.poller.take() {
            let _ = poller.join();
        }
    }
}
