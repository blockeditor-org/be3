use std::rc::Rc;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::be_model::{FieldRef, List};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{ContentProjection, Editor};
use terminal_types::{
    Terminal, TerminalColors, TerminalContent, TerminalCursor, TerminalHost, TerminalLine,
    TerminalMessage, TerminalShell, TerminalSize, TerminalWheel,
};
use terminal_view::{Fit, Lines, TerminalInput};

#[derive(Clone, Copy)]
pub(crate) struct Session {
    editor: Editor,
    pub(crate) host: Memo<Option<(u64, TerminalHost)>>,
    pub(crate) shell: ReadSignal<Option<TerminalShell>>,
    pub(crate) history: Memo<Lines>,
    pub(crate) screen: Memo<Lines>,
    pub(crate) alternate: Memo<Lines>,
    pub(crate) alternate_active: ReadSignal<bool>,
    pub(crate) cursor: ReadSignal<TerminalCursor>,
    pub(crate) colors: ReadSignal<TerminalColors>,
    wanted: Signal<Option<TerminalSize>>,
}

fn lines(
    content: ContentProjection<TerminalContent>,
    field: FieldRef<Terminal, List<TerminalLine>>,
) -> Memo<Lines> {
    let objects = content.objects(ObjectId::ROOT, field);
    create_memo(move || {
        objects.with(|objects| {
            objects
                .iter()
                .map(|(id, line)| (*id, Rc::clone(line)))
                .collect()
        })
    })
}

impl Session {
    pub(crate) fn new(editor: Editor) -> Self {
        let content = editor.block_content::<TerminalContent>();
        let writer = content.field(ObjectId::ROOT, Terminal::HOST);
        let peers = editor.peers::<TerminalHost>();
        let host = create_memo(move || {
            let writer = writer.get();
            peers.with(|hosts| {
                hosts
                    .iter()
                    .find(|(client, _)| Some(*client) == writer)
                    .cloned()
            })
        });
        let session = Self {
            editor,
            host,
            shell: content.field(ObjectId::ROOT, Terminal::SHELL),
            history: lines(content, Terminal::HISTORY),
            screen: lines(content, Terminal::SCREEN),
            alternate: lines(content, Terminal::ALTERNATE),
            alternate_active: content.field(ObjectId::ROOT, Terminal::ALTERNATE_ACTIVE),
            cursor: content.field(ObjectId::ROOT, Terminal::CURSOR),
            colors: content.field(ObjectId::ROOT, Terminal::COLORS),
            wanted: create_signal(None),
        };
        let client =
            create_memo(move || host.with(|host| host.as_ref().map(|(client, _)| *client)));
        create_effect(move || {
            if client.get().is_some() {
                untrack(|| session.resized());
            }
        });
        session
    }

    pub(crate) fn empty(&self) -> bool {
        self.history.with(Lines::is_empty)
            && self.screen.with(Lines::is_empty)
            && self.alternate.with(Lines::is_empty)
    }

    fn send(&self, message: TerminalMessage) {
        if !self.editor.editable().get_untracked() {
            return;
        }
        if let Some((client, _)) = self.host.get_untracked() {
            self.editor.send_to_peers(Some(client), &message);
        }
    }

    fn resized(&self) {
        if let Some(size) = self.wanted.get_untracked() {
            self.send(TerminalMessage::Resize(size));
        }
    }

    pub(crate) fn input(&self, input: TerminalInput) {
        self.send(match input {
            TerminalInput::Key(key) => TerminalMessage::Key(key),
            TerminalInput::Paste(text) => TerminalMessage::Paste(text),
        });
    }

    pub(crate) fn wheel(&self, wheel: TerminalWheel) {
        self.send(TerminalMessage::Wheel(wheel));
    }

    pub(crate) fn fit(&self, fit: Fit) {
        let size = TerminalSize {
            cols: fit.cols,
            rows: fit.rows,
        };
        if self.wanted.get_untracked() == Some(size) {
            return;
        }
        self.wanted.set(Some(size));
        self.resized();
    }
}
