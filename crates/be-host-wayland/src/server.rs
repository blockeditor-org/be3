use std::cell::RefCell;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::{Client, Display, ListeningSocket};

use crate::runtime_dir::RuntimeDir;
use crate::state::{ClientState, State};

pub struct Server {
    display: Display<State>,
    event_loop: EventLoop<'static, State>,
    socket: Option<ListeningSocket>,
    runtime: Option<RuntimeDir>,
    name: Option<OsString>,
    pub state: State,
}

impl Server {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Self::listening(RuntimeDir::from_env()?)
    }

    pub(crate) fn listening(runtime: RuntimeDir) -> Result<Self, Box<dyn Error>> {
        let mut server = Self::headless()?;
        let socket = match runtime.private() {
            true => ListeningSocket::bind_absolute(runtime.path().join("wayland-0"))?,
            false => ListeningSocket::bind_auto("wayland", 1..33)?,
        };
        server.name = match runtime.private() {
            true => Some("wayland-0".into()),
            false => socket.socket_name().map(OsStr::to_owned),
        };
        server.socket = Some(socket);
        server.runtime = Some(runtime);
        Ok(server)
    }

    pub fn headless() -> Result<Self, Box<dyn Error>> {
        let display = Display::new()?;
        let event_loop = EventLoop::try_new()?;
        let state = State::new(&display.handle(), Waiter::new()?, event_loop.handle());
        Ok(Self {
            display,
            event_loop,
            socket: None,
            runtime: None,
            name: None,
            state,
        })
    }

    pub fn socket_name(&self) -> Option<&OsStr> {
        self.name.as_deref()
    }

    pub fn runtime_dir(&self) -> Option<&Path> {
        self.runtime.as_ref().map(RuntimeDir::path)
    }

    pub fn start_xwayland(&mut self) -> std::io::Result<u32> {
        self.state.start_xwayland()
    }

    pub fn x11_display(&self) -> Option<u32> {
        self.state.xwayland.display()
    }

    pub fn connect(&mut self, stream: UnixStream) -> std::io::Result<Client> {
        self.display
            .handle()
            .insert_client(stream, Arc::new(ClientState::default()))
    }

    pub fn dispatch(&mut self) {
        if let Some(socket) = &self.socket {
            while let Ok(Some(stream)) = socket.accept() {
                let _ = self
                    .display
                    .handle()
                    .insert_client(stream, Arc::new(ClientState::default()));
            }
        }
        self.state.release_blockers();
        let _ = self
            .event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut self.state);
        self.state.settle_xwayland();
        let _ = self.display.dispatch_clients(&mut self.state);
        for client in self.state.take_disconnected() {
            let _ = self
                .display
                .backend()
                .dispatch_single_client(&mut self.state, client);
        }
        self.state.release_blockers();
        self.state.fetch_selection();
        self.state.cleanup();
        self.flush();
    }

    pub fn flush(&mut self) {
        let _ = self.display.flush_clients();
    }

    pub fn watch(&mut self, wake: impl Fn() + Send + 'static) -> std::io::Result<Watch> {
        let mut fds = vec![
            rustix::io::dup(self.display.backend().poll_fd())?,
            rustix::io::dup(self.event_loop.as_fd())?,
        ];
        if let Some(socket) = &self.socket {
            fds.push(rustix::io::dup(socket.as_fd())?);
        }
        let waiter = self.state.waiter().clone();
        let thread = Arc::clone(&waiter.waiting);
        std::thread::Builder::new()
            .name("wayland watch".to_owned())
            .spawn(move || watch(&fds, &thread, &wake))?;
        Ok(Watch { waiter })
    }

    pub fn join_loop<D: 'static>(
        &mut self,
        handle: &LoopHandle<'static, D>,
        wake: impl Fn() + Clone + 'static,
    ) -> std::io::Result<()> {
        let mut fds = vec![
            rustix::io::dup(self.display.backend().poll_fd())?,
            rustix::io::dup(self.event_loop.as_fd())?,
        ];
        if let Some(socket) = &self.socket {
            fds.push(rustix::io::dup(socket.as_fd())?);
        }
        for fd in fds {
            let wake = wake.clone();
            handle
                .insert_source(
                    Generic::new(fd, Interest::READ, Mode::Edge),
                    move |_, _, _| {
                        wake();
                        Ok(PostAction::Continue)
                    },
                )
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
        let handle = handle.clone();
        let woken = wake.clone();
        let sink: Sink = Box::new(move |fd: OwnedFd, ready: Arc<AtomicBool>| {
            let wake = woken.clone();
            let inserted = handle.insert_source(
                Generic::new(fd, Interest::READ, Mode::OneShot),
                move |_, _, _| {
                    ready.store(true, Ordering::SeqCst);
                    wake();
                    Ok(PostAction::Remove)
                },
            );
            if let Err(error) = inserted {
                eprintln!("be-wayland: a buffer's fence could not be watched: {error}");
            }
        });
        *self.state.waiter().sink.borrow_mut() = Some(sink);
        wake();
        Ok(())
    }
}

type Sink = Box<dyn Fn(OwnedFd, Arc<AtomicBool>)>;

pub struct Watch {
    waiter: Waiter,
}

impl Watch {
    pub fn resume(&self) {
        self.waiter.waiting.resumed.store(true, Ordering::SeqCst);
        self.waiter.waiting.kick();
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.waiter.waiting.stopped.store(true, Ordering::SeqCst);
        self.waiter.waiting.kick();
    }
}

struct Waiting {
    fds: Mutex<Vec<(OwnedFd, Arc<AtomicBool>)>>,
    kick: OwnedFd,
    resumed: AtomicBool,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct Waiter {
    waiting: Arc<Waiting>,
    sink: Rc<RefCell<Option<Sink>>>,
}

impl Waiter {
    pub fn new() -> std::io::Result<Self> {
        let kick = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
        Ok(Self {
            waiting: Arc::new(Waiting {
                fds: Mutex::new(Vec::new()),
                kick,
                resumed: AtomicBool::new(false),
                stopped: AtomicBool::new(false),
            }),
            sink: Rc::default(),
        })
    }

    pub fn wait(&self, fd: OwnedFd, ready: Arc<AtomicBool>) {
        if let Some(sink) = &*self.sink.borrow() {
            sink(fd, ready);
            return;
        }
        self.waiting.fds.lock().unwrap().push((fd, ready));
        self.waiting.kick();
    }
}

impl Waiting {
    fn kick(&self) {
        let _ = rustix::io::write(&self.kick, &1u64.to_ne_bytes());
    }
}

pub fn readable(fd: BorrowedFd<'_>) -> bool {
    let mut polled = [PollFd::from_borrowed_fd(fd, PollFlags::IN)];
    let zero = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(poll(&mut polled, Some(&zero)), Ok(count) if count > 0)
        && !polled[0].revents().is_empty()
}

fn watch(fds: &[OwnedFd], waiter: &Waiting, wake: &dyn Fn()) {
    let mut paused = false;
    let mut blockers: Vec<(OwnedFd, Arc<AtomicBool>)> = Vec::new();
    loop {
        if waiter.stopped.load(Ordering::SeqCst) {
            return;
        }
        blockers.extend(waiter.fds.lock().unwrap().drain(..));
        if waiter.resumed.swap(false, Ordering::SeqCst) {
            paused = false;
        }
        let display = if paused { 0 } else { fds.len() };
        let mut polled: Vec<PollFd<'_>> = std::iter::once(&waiter.kick)
            .chain(fds.iter().take(display))
            .chain(blockers.iter().map(|(fd, _)| fd))
            .map(|fd| PollFd::new(fd, PollFlags::IN))
            .collect();
        match poll(&mut polled, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
        }
        let ready: Vec<bool> = polled.iter().map(|fd| !fd.revents().is_empty()).collect();
        drop(polled);
        if ready[0] {
            let mut count = [0u8; 8];
            let _ = rustix::io::read(&waiter.kick, &mut count);
        }
        let mut woke = false;
        if ready[1..=display].iter().any(|ready| *ready) {
            paused = true;
            woke = true;
        }
        let mut index = 0;
        blockers.retain(|(_, flag)| {
            let fired = ready[1 + display + index];
            index += 1;
            if fired {
                flag.store(true, Ordering::SeqCst);
                woke = true;
            }
            !fired
        });
        if woke {
            wake();
        }
    }
}

#[cfg(test)]
mod tests;
