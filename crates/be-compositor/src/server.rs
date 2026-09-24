use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use smithay::reexports::wayland_server::{Client, Display, ListeningSocket};

use crate::state::{ClientState, State};

pub struct Server {
    display: Display<State>,
    socket: Option<ListeningSocket>,
    name: Option<OsString>,
    pub state: State,
}

impl Server {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut server = Self::headless()?;
        let socket = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(_) => ListeningSocket::bind_auto("wayland", 1..33)?,
            None => {
                let path =
                    std::env::temp_dir().join(format!("be-compositor-{}", std::process::id()));
                let socket = ListeningSocket::bind_absolute(path.clone())?;
                server.name = Some(path.into_os_string());
                socket
            }
        };
        if server.name.is_none() {
            server.name = socket.socket_name().map(OsStr::to_owned);
        }
        server.socket = Some(socket);
        Ok(server)
    }

    pub fn headless() -> Result<Self, Box<dyn Error>> {
        let display = Display::new()?;
        let state = State::new(&display.handle(), Waiter::new()?);
        Ok(Self {
            display,
            socket: None,
            name: None,
            state,
        })
    }

    pub fn socket_name(&self) -> Option<&OsStr> {
        self.name.as_deref()
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
        let _ = self.display.dispatch_clients(&mut self.state);
        self.state.release_blockers();
        self.state.cleanup();
        self.flush();
    }

    pub fn flush(&mut self) {
        let _ = self.display.flush_clients();
    }

    pub fn watch(&mut self, wake: impl Fn() + Send + 'static) -> std::io::Result<Watch> {
        let mut fds = vec![rustix::io::dup(self.display.backend().poll_fd())?];
        if let Some(socket) = &self.socket {
            fds.push(rustix::io::dup(socket.as_fd())?);
        }
        let waiter = self.state.waiter().clone();
        let thread = waiter.clone();
        std::thread::Builder::new()
            .name("wayland watch".to_owned())
            .spawn(move || watch(&fds, &thread, &wake))?;
        Ok(Watch { waiter })
    }
}

pub struct Watch {
    waiter: Waiter,
}

impl Watch {
    pub fn resume(&self) {
        self.waiter.0.resumed.store(true, Ordering::SeqCst);
        self.waiter.kick();
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.waiter.0.stopped.store(true, Ordering::SeqCst);
        self.waiter.kick();
    }
}

struct Waiting {
    fds: Mutex<Vec<(OwnedFd, Arc<AtomicBool>)>>,
    kick: OwnedFd,
    resumed: AtomicBool,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct Waiter(Arc<Waiting>);

impl Waiter {
    pub fn new() -> std::io::Result<Self> {
        let kick = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
        Ok(Self(Arc::new(Waiting {
            fds: Mutex::new(Vec::new()),
            kick,
            resumed: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        })))
    }

    pub fn wait(&self, fd: OwnedFd, ready: Arc<AtomicBool>) {
        self.0.fds.lock().unwrap().push((fd, ready));
        self.kick();
    }

    fn kick(&self) {
        let _ = rustix::io::write(&self.0.kick, &1u64.to_ne_bytes());
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

fn watch(fds: &[OwnedFd], waiter: &Waiter, wake: &dyn Fn()) {
    let mut paused = false;
    let mut blockers: Vec<(OwnedFd, Arc<AtomicBool>)> = Vec::new();
    loop {
        if waiter.0.stopped.load(Ordering::SeqCst) {
            return;
        }
        blockers.extend(waiter.0.fds.lock().unwrap().drain(..));
        if waiter.0.resumed.swap(false, Ordering::SeqCst) {
            paused = false;
        }
        let display = if paused { 0 } else { fds.len() };
        let mut polled: Vec<PollFd<'_>> = std::iter::once(&waiter.0.kick)
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
            let _ = rustix::io::read(&waiter.0.kick, &mut count);
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
