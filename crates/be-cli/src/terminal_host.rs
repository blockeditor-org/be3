use std::cell::RefCell;
use std::future::Future;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use be_block::presence::{ChannelKind, PresenceKind};
use be_block::terminal::{LARGEST_OUTPUT, TerminalContent, TerminalHost, TerminalMessage, TerminalSize};
use be_client::{ClientError, Live, Peer};
use be_protocol::ClientId;
use be_store::ObjectStore;
use ghostty_vt::{Renderer, Terminal};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::{broadcast::error::RecvError, mpsc};
use uuid::Uuid;

use crate::session::explain;

const DEFAULT_SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };
const MAX_SCROLLBACK: usize = 1_000;
const READ_CHUNK: usize = 16 * 1024;
const LONGEST_RETRY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default)]
pub struct HostOptions {
    pub command: Vec<String>,
    pub machine: String,
    pub user: String,
}

impl HostOptions {
    pub fn here(command: Vec<String>) -> Self {
        let machine = ["HOSTNAME", "COMPUTERNAME"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "this machine".to_owned());
        let user = ["USER", "USERNAME"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .unwrap_or_default();
        Self {
            command,
            machine,
            user,
        }
    }
}

enum Event {
    Output(u64, Vec<u8>),
    Ended(u64),
}

struct Shell {
    id: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

pub struct Host {
    options: HostOptions,
    size: TerminalSize,
    shell: Option<Shell>,
    shells: u64,
    sequence: u64,
    mirror: Terminal,
    renderer: Renderer,
    replies: Rc<RefCell<Vec<u8>>>,
    sender: mpsc::UnboundedSender<Event>,
    events: mpsc::UnboundedReceiver<Event>,
}

enum Ended {
    Shutdown,
    Disconnected(String),
}

impl Host {
    pub fn new(options: HostOptions) -> Result<Self, String> {
        let mut mirror = Terminal::new(DEFAULT_SIZE.cols, DEFAULT_SIZE.rows, MAX_SCROLLBACK)
            .map_err(|error| format!("could not make a terminal emulator: {error}"))?;
        let replies = Rc::new(RefCell::new(Vec::new()));
        let held = Rc::clone(&replies);
        mirror
            .on_reply(move |reply| held.borrow_mut().extend_from_slice(reply))
            .map_err(|error| format!("could not answer the terminal's queries: {error}"))?;
        let renderer = Renderer::new()
            .map_err(|error| format!("could not read the terminal's screen: {error}"))?;
        let (sender, events) = mpsc::unbounded_channel();
        Ok(Self {
            options,
            size: DEFAULT_SIZE,
            shell: None,
            shells: 0,
            sequence: 0,
            mirror,
            renderer,
            replies,
            sender,
            events,
        })
    }

    pub async fn run<S: ObjectStore>(
        &mut self,
        connect: impl AsyncFn() -> Result<(Arc<Peer<S>>, Uuid), String>,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), String> {
        let mut shutdown = std::pin::pin!(shutdown);
        let mut retry = Duration::from_secs(1);
        loop {
            let ended = match connect().await {
                Ok((peer, block)) => {
                    retry = Duration::from_secs(1);
                    eprintln!("be: hosting a terminal for block {block}");
                    self.serve(peer, block, shutdown.as_mut()).await
                }
                Err(error) => Ended::Disconnected(error),
            };
            let Ended::Disconnected(error) = ended else {
                return Ok(());
            };
            eprintln!(
                "be: {error}; connecting again in {}s, with the shell kept running",
                retry.as_secs()
            );
            let wait = tokio::time::sleep(retry);
            tokio::pin!(wait);
            loop {
                tokio::select! {
                    () = &mut wait => break,
                    () = shutdown.as_mut() => return Ok(()),
                    Some(event) = self.events.recv() => {
                        let _ = self.take(event);
                    }
                }
            }
            retry = (retry * 2).min(LONGEST_RETRY);
        }
    }

    pub async fn serve_until<S: ObjectStore>(
        &mut self,
        peer: Arc<Peer<S>>,
        block: Uuid,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), String> {
        let shutdown = std::pin::pin!(shutdown);
        match self.serve(peer, block, shutdown).await {
            Ended::Shutdown => Ok(()),
            Ended::Disconnected(error) => Err(error),
        }
    }

    async fn serve<S: ObjectStore>(
        &mut self,
        peer: Arc<Peer<S>>,
        block: Uuid,
        mut shutdown: std::pin::Pin<&mut impl Future<Output = ()>>,
    ) -> Ended {
        let mut doorbell = peer.connection().subscribe();
        let mut live = match Live::<S, TerminalContent>::join(Arc::clone(&peer), block).await {
            Ok(live) => live,
            Err(error) => return Ended::Disconnected(explain(error)),
        };
        let shown = TerminalHost {
            machine: self.options.machine.clone(),
            user: self.options.user.clone(),
        };
        let presence = serde_json::to_vec(&shown).unwrap_or_default();
        if let Err(error) = live.set_presence(TerminalHost::ID, Some(presence)).await {
            return Ended::Disconnected(explain(error));
        }
        let outcome = loop {
            tokio::select! {
                () = shutdown.as_mut() => break Ok(true),
                rung = doorbell.recv() => match rung {
                    Ok(_) | Err(RecvError::Lagged(_)) => {
                        if let Err(error) = self.answer(&mut live).await {
                            break Err(error);
                        }
                    }
                    Err(RecvError::Closed) => break Err("the connection to the server closed".to_owned()),
                },
                Some(event) = self.events.recv() => {
                    if let Err(error) = self.forward(&live, event).await {
                        break Err(error);
                    }
                }
            }
        };
        match outcome {
            Ok(_) => {
                let _ = live.set_presence(TerminalHost::ID, None).await;
                let _ = peer.leave_session(block).await;
                Ended::Shutdown
            }
            Err(error) => Ended::Disconnected(error),
        }
    }

    async fn answer<S: ObjectStore>(
        &mut self,
        live: &mut Live<S, TerminalContent>,
    ) -> Result<(), String> {
        live.poll().await.map_err(explain)?;
        live.catch_up().await.map_err(explain)?;
        if live.is_owner() && !live.is_clean() {
            live.seal().await.map_err(explain)?;
        }
        for message in live.take_channel() {
            if message.channel != TerminalMessage::ID {
                continue;
            }
            let Some(decoded) = TerminalMessage::decode(&message.payload) else {
                continue;
            };
            self.receive(live, message.from, decoded).await?;
        }
        Ok(())
    }

    async fn receive<S: ObjectStore>(
        &mut self,
        live: &Live<S, TerminalContent>,
        from: ClientId,
        message: TerminalMessage,
    ) -> Result<(), String> {
        match message {
            TerminalMessage::Attach(size) => {
                let started = self.ensure_shell()?;
                let resized = self.resize(size);
                let to = match started || resized {
                    true => None,
                    false => Some(from),
                };
                send(live, to, &self.snapshot()).await
            }
            TerminalMessage::Resize(size) => {
                if self.resize(size) {
                    send(live, None, &self.snapshot()).await?;
                }
                Ok(())
            }
            TerminalMessage::Input(bytes) => {
                if self.ensure_shell()? {
                    return send(live, None, &self.snapshot()).await;
                }
                if let Some(shell) = &mut self.shell {
                    let _ = shell.writer.write_all(&bytes);
                    let _ = shell.writer.flush();
                }
                Ok(())
            }
            TerminalMessage::Snapshot { .. }
            | TerminalMessage::Output { .. }
            | TerminalMessage::Exited { .. } => Ok(()),
        }
    }

    async fn forward<S: ObjectStore>(
        &mut self,
        live: &Live<S, TerminalContent>,
        event: Event,
    ) -> Result<(), String> {
        let Some(message) = self.take(event) else {
            return Ok(());
        };
        let messages = match message {
            TerminalMessage::Output {
                shell,
                sequence,
                bytes,
            } => {
                let chunks = bytes.chunks(LARGEST_OUTPUT).count() as u64;
                bytes
                    .chunks(LARGEST_OUTPUT)
                    .enumerate()
                    .map(|(index, chunk)| TerminalMessage::Output {
                        shell,
                        sequence: sequence + 1 - chunks + index as u64,
                        bytes: chunk.to_vec(),
                    })
                    .collect()
            }
            other => vec![other],
        };
        for message in &messages {
            send(live, None, message).await?;
        }
        Ok(())
    }

    fn take(&mut self, event: Event) -> Option<TerminalMessage> {
        match event {
            Event::Output(shell, bytes) => {
                if self.shell.as_ref().map(|running| running.id) != Some(shell) {
                    return None;
                }
                self.mirror.write(&bytes);
                let replies = std::mem::take(&mut *self.replies.borrow_mut());
                if !replies.is_empty()
                    && let Some(running) = &mut self.shell
                {
                    let _ = running.writer.write_all(&replies);
                    let _ = running.writer.flush();
                }
                self.sequence += bytes.len().div_ceil(LARGEST_OUTPUT) as u64;
                Some(TerminalMessage::Output {
                    shell,
                    sequence: self.sequence,
                    bytes,
                })
            }
            Event::Ended(shell) => {
                let running = self.shell.take_if(|running| running.id == shell)?;
                let mut child = running.child;
                let status = child.wait().ok().map(|status| status.exit_code());
                Some(TerminalMessage::Exited { shell, status })
            }
        }
    }

    fn snapshot(&mut self) -> TerminalMessage {
        let screen = match self.renderer.update(&mut self.mirror) {
            Ok(screen) => screen.repaint(),
            Err(_) => Vec::new(),
        };
        TerminalMessage::Snapshot {
            shell: self.shell.as_ref().map_or(self.shells, |shell| shell.id),
            sequence: self.sequence,
            size: self.size,
            screen,
        }
    }

    fn resize(&mut self, size: TerminalSize) -> bool {
        let size = TerminalSize {
            cols: size.cols.max(1),
            rows: size.rows.max(1),
        };
        if size == self.size {
            return false;
        }
        self.size = size;
        let _ = self.mirror.resize(size.cols, size.rows, 1, 1);
        if let Some(shell) = &self.shell {
            let _ = shell.master.resize(pty_size(size));
        }
        true
    }

    fn ensure_shell(&mut self) -> Result<bool, String> {
        if self.shell.is_some() {
            return Ok(false);
        }
        let pair = native_pty_system()
            .openpty(pty_size(self.size))
            .map_err(|error| format!("could not open a terminal: {error}"))?;
        let mut command = match self.options.command.split_first() {
            Some((program, arguments)) => {
                let mut command = CommandBuilder::new(program);
                command.args(arguments);
                command
            }
            None => CommandBuilder::new_default_prog(),
        };
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            command.cwd(home);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("could not start the shell: {error}"))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("could not read the terminal: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("could not write to the terminal: {error}"))?;
        self.shells += 1;
        let id = self.shells;
        self.sequence = 0;
        self.mirror.reset();
        let _ = self.mirror.resize(self.size.cols, self.size.rows, 1, 1);
        read(reader, id, self.sender.clone());
        self.shell = Some(Shell {
            id,
            master: pair.master,
            writer,
            child,
        });
        Ok(true)
    }

}

async fn send<S: ObjectStore>(
    live: &Live<S, TerminalContent>,
    to: Option<ClientId>,
    message: &TerminalMessage,
) -> Result<(), String> {
    match live
        .send_channel(to, TerminalMessage::ID, message.encode())
        .await
    {
        Ok(()) => Ok(()),
        Err(ClientError::Refused(..)) if to.is_some() => Ok(()),
        Err(error) => Err(explain(error)),
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(shell) = &mut self.shell {
            let _ = shell.child.kill();
        }
    }
}

fn read(mut reader: Box<dyn Read + Send>, shell: u64, events: mpsc::UnboundedSender<Event>) {
    std::thread::spawn(move || {
        let mut buffer = vec![0u8; READ_CHUNK];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 || events.send(Event::Output(shell, buffer[..read].to_vec())).is_err() {
                break;
            }
        }
        let _ = events.send(Event::Ended(shell));
    });
}

fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
