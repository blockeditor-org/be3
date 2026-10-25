use std::future::Future;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use be_block::ObjectId;
use be_block::presence::{ChannelKind, PresenceKind};
use be_client::Peer;
use be_store::ObjectStore;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use terminal_types::{
    Terminal, TerminalContent, TerminalHost, TerminalMessage, TerminalShell, TerminalSize,
};
use tokio::sync::mpsc;
use tokio::time::Instant;
use uuid::Uuid;

use crate::replica::Session;
use crate::tracker::Sources;

mod document;

use terminal_emulator::Emulator;

const DEFAULT_SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };
const READ_CHUNK: usize = 16 * 1024;
const LONGEST_RETRY: Duration = Duration::from_secs(30);
const CLAIM_INTERVAL: Duration = Duration::from_secs(3);

type Hosted<S> = Session<S, TerminalContent>;

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
    record: Option<TerminalShell>,
    emulator: Emulator,
    writing: bool,
    dirty: bool,
    sender: mpsc::UnboundedSender<Event>,
    events: mpsc::UnboundedReceiver<Event>,
}

enum Ended {
    Shutdown,
    Refused(String),
    Disconnected(String),
}

impl Host {
    pub fn new(options: HostOptions) -> Result<Self, String> {
        let (sender, events) = mpsc::unbounded_channel();
        Ok(Self {
            options,
            size: DEFAULT_SIZE,
            shell: None,
            record: None,
            emulator: Emulator::new(DEFAULT_SIZE)?,
            writing: false,
            dirty: false,
            sender,
            events,
        })
    }

    pub async fn run<S: ObjectStore + 'static>(
        &mut self,
        connect: impl AsyncFn() -> Result<(Arc<Peer<S>>, Uuid), String>,
        sources: &Sources,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), String> {
        let mut shutdown = std::pin::pin!(shutdown);
        let mut retry = Duration::from_secs(1);
        let mut connected = false;
        loop {
            let ended = match connect().await {
                Ok((peer, block)) => {
                    connected = true;
                    self.serve(peer, block, sources(), shutdown.as_mut()).await
                }
                Err(error) if !connected => return Err(error),
                Err(error) => Ended::Refused(error),
            };
            let error = match ended {
                Ended::Shutdown => return Ok(()),
                Ended::Refused(error) => error,
                Ended::Disconnected(error) => {
                    retry = Duration::from_secs(1);
                    error
                }
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
                    Some(event) = self.events.recv() => self.take(event),
                }
            }
            retry = (retry * 2).min(LONGEST_RETRY);
        }
    }

    pub async fn serve_until<S: ObjectStore + 'static>(
        &mut self,
        peer: Arc<Peer<S>>,
        block: Uuid,
        sources: Vec<be_types_host::Source>,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), String> {
        let shutdown = std::pin::pin!(shutdown);
        match self.serve(peer, block, sources, shutdown).await {
            Ended::Shutdown => Ok(()),
            Ended::Refused(error) | Ended::Disconnected(error) => Err(error),
        }
    }

    async fn serve<S: ObjectStore + 'static>(
        &mut self,
        peer: Arc<Peer<S>>,
        block: Uuid,
        sources: Vec<be_types_host::Source>,
        mut shutdown: std::pin::Pin<&mut impl Future<Output = ()>>,
    ) -> Ended {
        let mut session = match Hosted::open(peer, block, sources).await {
            Ok(session) => session,
            Err(error) => return Ended::Refused(error),
        };
        let shown = TerminalHost {
            machine: self.options.machine.clone(),
            user: self.options.user.clone(),
        };
        let presence = serde_json::to_vec(&shown).unwrap_or_default();
        session.show(TerminalHost::ID, Some(presence));
        session.settle().await;
        eprintln!("be: hosting a terminal for block {block}");
        self.writing = false;
        let mut claim = Instant::now();
        let outcome = loop {
            if let Err(error) = self.step(&mut session, &mut claim).await {
                break Err(error);
            }
            let claiming = !session.is_owner() && !standing_by(&session);
            let next = claim.max(Instant::now());
            tokio::select! {
                () = shutdown.as_mut() => break Ok(()),
                woken = session.next() => {
                    if let Err(error) = session.handle(woken).await {
                        break Err(error);
                    }
                    if let Err(error) = self.answer(&mut session) {
                        break Err(error);
                    }
                }
                Some(event) = self.events.recv() => {
                    self.take(event);
                    while let Ok(event) = self.events.try_recv() {
                        self.take(event);
                    }
                }
                () = tokio::time::sleep_until(next), if claiming => {}
            }
        };
        match outcome {
            Ok(()) => {
                if session.is_owner() && self.writing {
                    self.write(&mut session);
                }
                session.show(TerminalHost::ID, None);
                session.settle().await;
                session.close().await;
                Ended::Shutdown
            }
            Err(error) => Ended::Disconnected(error),
        }
    }

    async fn step<S: ObjectStore + 'static>(
        &mut self,
        session: &mut Hosted<S>,
        claim: &mut Instant,
    ) -> Result<(), String> {
        if !session.is_owner() {
            if self.writing {
                eprintln!("be: another client took over the terminal; standing by");
                self.writing = false;
            }
            let now = Instant::now();
            if !standing_by(session) && now >= *claim {
                *claim = now + CLAIM_INTERVAL;
                session.claim();
                session.settle().await;
            }
            if !session.is_owner() {
                return Ok(());
            }
        }
        if !self.writing {
            self.take_over(session)?;
        }
        if self.dirty {
            self.write(session);
        }
        session.settle().await;
        Ok(())
    }

    fn take_over<S: ObjectStore + 'static>(
        &mut self,
        session: &mut Hosted<S>,
    ) -> Result<(), String> {
        let client = session.client();
        let Some(document) = session.content() else {
            return Ok(());
        };
        let recorded = document.field(ObjectId::ROOT, Terminal::SHELL);
        let ours = self.shell.is_some()
            && recorded.map(|shell| shell.id) == self.record.map(|shell| shell.id);
        let mut changes = Vec::new();
        if !ours {
            let empty = document.ids(ObjectId::ROOT, Terminal::HISTORY).is_empty()
                && document.ids(ObjectId::ROOT, Terminal::SCREEN).is_empty();
            let marker = (!empty).then(|| format!("Restored session: {}", now_utc()));
            changes = document::retire(document, marker);
        }
        if document.field(ObjectId::ROOT, Terminal::HOST) != client {
            changes.push(Terminal::HOST.set(ObjectId::ROOT, &client));
        }
        if self.shell.is_none() {
            let size = document.field(ObjectId::ROOT, Terminal::SIZE);
            if size.cols > 0 && size.rows > 0 {
                self.size = size;
            }
        }
        for edit in document::edits(changes) {
            session.edit(edit);
        }
        self.writing = true;
        self.dirty = true;
        if self.shell.is_none() {
            self.emulator.reset(self.size);
            self.start_shell()?;
        }
        Ok(())
    }

    fn write<S: ObjectStore + 'static>(&mut self, session: &mut Hosted<S>) {
        let Some(document) = session.content() else {
            return;
        };
        self.dirty = false;
        let (returned, scrolled) = self.emulator.take_scrolled();
        let shown = self.emulator.shown();
        let changes = document::sync(document, &returned, scrolled, &shown, self.record);
        for edit in document::edits(changes) {
            session.edit(edit);
        }
    }

    fn answer<S: ObjectStore + 'static>(&mut self, session: &mut Hosted<S>) -> Result<(), String> {
        for message in session.take_messages() {
            if message.channel != TerminalMessage::ID || !self.writing {
                continue;
            }
            let Some(decoded) = TerminalMessage::decode(&message.payload) else {
                continue;
            };
            self.receive(session, decoded)?;
        }
        Ok(())
    }

    fn receive<S: ObjectStore + 'static>(
        &mut self,
        session: &mut Hosted<S>,
        message: TerminalMessage,
    ) -> Result<(), String> {
        if let TerminalMessage::Resize(size) = message {
            self.resize(size);
            return Ok(());
        }
        if self.shell.is_none() {
            self.write(session);
            if let Some(document) = session.content() {
                let changes = document::retire(document, None);
                for edit in document::edits(changes) {
                    session.edit(edit);
                }
            }
            self.emulator.reset(self.size);
            return self.start_shell();
        }
        let bytes = match &message {
            TerminalMessage::Key(key) => self.emulator.key(key),
            TerminalMessage::Paste(text) => self.emulator.paste(text),
            TerminalMessage::Wheel(wheel) => self.emulator.wheel(wheel),
            TerminalMessage::Resize(_) => Vec::new(),
        };
        self.type_into(&bytes);
        Ok(())
    }

    fn type_into(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(shell) = &mut self.shell {
            let _ = shell.writer.write_all(bytes);
            let _ = shell.writer.flush();
        }
    }

    fn take(&mut self, event: Event) {
        match event {
            Event::Output(shell, bytes) => {
                if self.shell.as_ref().map(|running| running.id) != Some(shell) {
                    return;
                }
                self.emulator.feed(&bytes);
                let replies = self.emulator.take_replies();
                self.type_into(&replies);
                self.dirty = true;
            }
            Event::Ended(shell) => {
                let Some(running) = self.shell.take_if(|running| running.id == shell) else {
                    return;
                };
                let mut child = running.child;
                let status = child.wait().ok().map(|status| status.exit_code());
                self.record = Some(TerminalShell {
                    id: shell,
                    running: false,
                    status,
                });
                self.dirty = true;
            }
        }
    }

    fn resize(&mut self, size: TerminalSize) {
        let size = TerminalSize {
            cols: size.cols.max(1),
            rows: size.rows.max(1),
        };
        if size == self.size {
            return;
        }
        self.size = size;
        self.emulator.resize(size);
        if let Some(shell) = &self.shell {
            let _ = shell.master.resize(pty_size(size));
        }
        self.dirty = true;
    }

    fn start_shell(&mut self) -> Result<(), String> {
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
        let id = Uuid::new_v4().as_u64_pair().0;
        read(reader, id, self.sender.clone());
        self.shell = Some(Shell {
            id,
            master: pair.master,
            writer,
            child,
        });
        self.record = Some(TerminalShell {
            id,
            running: true,
            status: None,
        });
        self.dirty = true;
        Ok(())
    }
}

fn standing_by<S: ObjectStore + 'static>(session: &Hosted<S>) -> bool {
    let Some(owner) = session.state().and_then(|state| state.owner) else {
        return false;
    };
    session
        .presence()
        .iter()
        .any(|peer| peer.client == owner && peer.kind == TerminalHost::ID)
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
            if read == 0
                || events
                    .send(Event::Output(shell, buffer[..read].to_vec()))
                    .is_err()
            {
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

fn now_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as i64;
    let days = seconds.div_euclid(86_400);
    let minutes = seconds.rem_euclid(86_400) / 60;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        minutes / 60,
        minutes % 60
    )
}
