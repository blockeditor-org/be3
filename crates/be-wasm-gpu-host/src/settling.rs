use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::Waker;

use be_wasm_gpu_abi::device as abi;

pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug, PartialEq)]
enum State {
    /// Waiting for the map with this ticket: a buffer unmapped and mapped again can still hear
    /// its earlier map's callback, which must not settle the later one.
    Pending(u64),
    Mapped {
        range: Range<u64>,
        writable: bool,
    },
}

#[derive(Default)]
struct Held {
    states: HashMap<u32, State>,
    /// The tickets of maps and of submitted work, counted together.
    tickets: u64,
    /// What settled since the app last polled.
    maps: Vec<(u64, bool)>,
    work_done: Vec<u64>,
    pending: usize,
    lost: Option<String>,
    stopped: bool,
    /// Whether the background poller is inside `device.poll`, and how many of its polls have
    /// ended: a poll that has ended has run every callback it found settled.
    polling: bool,
    rounds: u64,
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

impl Held {
    fn ticket(&mut self) -> u64 {
        self.tickets += 1;
        self.tickets
    }

    /// Forgets the buffer's map: a map still pending settles as failed, and answers whether
    /// one did.
    fn forget(&mut self, buffer: u32) -> bool {
        let Some(State::Pending(ticket)) = self.states.remove(&buffer) else {
            return false;
        };
        self.pending -= 1;
        self.maps.push((ticket, false));
        true
    }
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
            held.pending += 1;
            held.ticket()
        };
        let shared = Arc::clone(&self.shared);
        let wake = self.wake.clone();
        queue.on_submitted_work_done(move || {
            {
                let mut held = shared.held();
                held.pending -= 1;
                held.work_done.push(ticket);
            }
            if let Some(wake) = wake {
                wake();
            }
        });
        self.poll_in_background(device);
        ticket
    }

    /// Takes what settled since the last call: maps with whether they mapped, and done work.
    pub(crate) fn take_settled(&self) -> (Vec<(u64, bool)>, Vec<u64>) {
        let mut held = self.shared.held();
        (
            std::mem::take(&mut held.maps),
            std::mem::take(&mut held.work_done),
        )
    }

    /// A ticket for a map that fails before it starts, settled at once.
    pub(crate) fn failed_map(&mut self) -> u64 {
        let ticket = {
            let mut held = self.shared.held();
            let ticket = held.ticket();
            held.maps.push((ticket, false));
            ticket
        };
        self.wake();
        ticket
    }

    pub(crate) fn forget_every_map(&mut self) {
        let mut held = self.shared.held();
        let buffers: Vec<u32> = held.states.keys().copied().collect();
        for buffer in buffers {
            let _ = held.forget(buffer);
        }
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

    pub(crate) fn map(
        &mut self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
        handle: u32,
        mode: abi::MapMode,
        range: Range<u64>,
    ) -> Result<u64, String> {
        let number = {
            let mut held = self.shared.held();
            if held.states.contains_key(&handle) {
                return Err(format!(
                    "buffer {handle} was asked to map while it is mapped or mapping"
                ));
            }
            let number = held.ticket();
            held.states.insert(handle, State::Pending(number));
            held.pending += 1;
            number
        };
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
                if held.states.get(&handle) == Some(&State::Pending(number)) {
                    held.pending -= 1;
                    held.maps.push((number, result.is_ok()));
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
        Ok(number)
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
        let forgot = self.shared.held().forget(buffer);
        if forgot {
            self.wake();
        }
    }

    /// Polls the device, waiting for the work submitted so far when `wait` is set. wgpu runs a
    /// submission's map and work-done callbacks on whichever thread's poll saw it finish, once
    /// that poll lets go of the device. So a wait that finds the queue empty because the
    /// background poller saw the work finish also waits for that poller's round to end, after
    /// which every callback the work owed has run.
    pub(crate) fn poll(&self, device: &wgpu::Device, wait: bool) -> Result<(), wgpu::PollError> {
        if self.shared.held().pending == 0 {
            return Ok(());
        }
        if !wait {
            return device.poll(wgpu::PollType::Poll).map(|_| ());
        }
        device.poll(wgpu::PollType::wait_indefinitely())?;
        let mut held = self.shared.held();
        let round = held.rounds;
        while held.polling && held.rounds == round {
            held = self
                .shared
                .changed
                .wait(held)
                .unwrap_or_else(|held| held.into_inner());
        }
        Ok(())
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
            .name("app gpu maps".into())
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
                        held.polling = true;
                    }
                    let _ = device.poll(wgpu::PollType::wait_indefinitely());
                    let mut held = shared.held();
                    held.polling = false;
                    held.rounds += 1;
                    drop(held);
                    shared.changed.notify_all();
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
