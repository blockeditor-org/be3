use block_editor_beui::Editor;
use terminal_types::{TerminalHost, TerminalMessage, TerminalSize};
use block_editor_beui::beui::reactive::*;
use terminal_view::{Emulator, Fit, TerminalScreen};

const SCROLLBACK: usize = 1_000;
const FIRST_SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };

#[derive(Clone, Copy)]
pub(crate) struct Session {
    editor: Editor,
    pub(crate) screen: TerminalScreen,
    pub(crate) host: Memo<Option<(u64, TerminalHost)>>,
    pub(crate) running: Signal<Option<(u64, u64)>>,
    pub(crate) exited: Signal<Option<Option<u32>>>,
    asked: Signal<bool>,
    wanted: Signal<TerminalSize>,
    cell: Signal<(u32, u32)>,
}

impl Session {
    pub(crate) fn new(editor: Editor) -> Result<Self, String> {
        let screen = TerminalScreen::new(
            Emulator::new(FIRST_SIZE.cols, FIRST_SIZE.rows, SCROLLBACK)?.with_cursor(),
        );
        let peers = editor.peers::<TerminalHost>();
        let host = create_memo(move || peers.with(|hosts| hosts.first().cloned()));
        let session = Self {
            editor,
            screen,
            host,
            running: create_signal(None),
            exited: create_signal(None),
            asked: create_signal(false),
            wanted: create_signal(FIRST_SIZE),
            cell: create_signal((1, 1)),
        };
        let client =
            create_memo(move || host.with(|host| host.as_ref().map(|(client, _)| *client)));
        create_effect(move || {
            let client = client.get();
            untrack(|| {
                session.running.set(None);
                session.exited.set(None);
                session.asked.set(false);
                if client.is_some() {
                    session.attach();
                }
            });
        });
        editor.on_peer_message(move |from, message: TerminalMessage| {
            session.receive(from, message);
        });
        Ok(session)
    }

    fn send(&self, message: TerminalMessage) {
        if let Some((client, _)) = self.host.get_untracked() {
            self.editor.send_to_peers(Some(client), &message);
        }
    }

    fn attach(&self) {
        self.asked.set(true);
        self.send(TerminalMessage::Attach(self.wanted.get_untracked()));
    }

    fn receive(&self, from: u64, message: TerminalMessage) {
        if self
            .host
            .with_untracked(|host| host.as_ref().map(|(client, _)| *client))
            != Some(from)
        {
            return;
        }
        match message {
            TerminalMessage::Snapshot {
                shell,
                sequence,
                size,
                screen,
            } => {
                let (width, height) = self.cell.get_untracked();
                self.screen.reset();
                self.screen.resize(size.cols, size.rows, width, height);
                self.screen.write(&screen);
                self.screen.refresh();
                self.asked.set(false);
                self.exited.set(None);
                self.running.set(Some((shell, sequence)));
            }
            TerminalMessage::Output {
                shell,
                sequence,
                bytes,
            } => match self.running.get_untracked() {
                Some((running, last)) if running == shell && sequence == last + 1 => {
                    self.screen.write(&bytes);
                    self.screen.refresh();
                    self.running.set(Some((shell, sequence)));
                }
                Some((running, last)) if running == shell && sequence <= last => {}
                _ if self.asked.get_untracked() => {}
                _ => self.attach(),
            },
            TerminalMessage::Exited { shell, status } => {
                if self.running.get_untracked().map(|(running, _)| running) == Some(shell) {
                    self.exited.set(Some(status));
                }
            }
            TerminalMessage::Attach(_) | TerminalMessage::Resize(_) | TerminalMessage::Input(_) => {
            }
        }
    }

    pub(crate) fn input(&self, bytes: Vec<u8>) {
        if self.editor.editable().get_untracked() {
            self.send(TerminalMessage::Input(bytes));
        }
    }

    pub(crate) fn fit(&self, fit: Fit) {
        self.cell.set((fit.cell_width, fit.cell_height));
        let size = TerminalSize {
            cols: fit.cols,
            rows: fit.rows,
        };
        if self.wanted.get_untracked() == size {
            return;
        }
        self.wanted.set(size);
        if self.editor.editable().get_untracked() {
            self.send(TerminalMessage::Resize(size));
        }
    }
}
