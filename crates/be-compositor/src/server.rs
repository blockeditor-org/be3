use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use rustix::event::{PollFd, PollFlags, poll};
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
                let path = std::env::temp_dir().join(format!("be-compositor-{}", std::process::id()));
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
        let state = State::new(&display.handle());
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
        let _ = self.display.dispatch_clients(&mut self.state);
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
        let (resume, resumed) = sync_channel(1);
        std::thread::Builder::new()
            .name("wayland watch".to_owned())
            .spawn(move || watch(&fds, &resumed, &wake))?;
        Ok(Watch { resume })
    }
}

pub struct Watch {
    resume: SyncSender<()>,
}

impl Watch {
    pub fn resume(&self) {
        let _ = self.resume.try_send(());
    }
}

fn watch(fds: &[OwnedFd], resumed: &Receiver<()>, wake: &dyn Fn()) {
    loop {
        let mut polled: Vec<PollFd<'_>> = fds
            .iter()
            .map(|fd| PollFd::new(fd, PollFlags::IN))
            .collect();
        match poll(&mut polled, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return,
        }
        wake();
        if resumed.recv().is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
