use block_editor_beui::Editor;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::terminal::{
    Terminal, TerminalColor, TerminalContent, TerminalHost, TerminalLine, TerminalMessage,
    TerminalShell, TerminalSize,
};
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::reactive::*;
use terminal_view::{Emulator, Fit, Row, Span, TerminalInput, TerminalScreen};

const FIRST_SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };

#[derive(Clone, Copy)]
pub(crate) struct Session {
    editor: Editor,
    pub(crate) screen: TerminalScreen,
    pub(crate) host: Memo<Option<(u64, TerminalHost)>>,
    pub(crate) shell: ReadSignal<Option<TerminalShell>>,
    wanted: Signal<TerminalSize>,
}

impl Session {
    pub(crate) fn new(editor: Editor) -> Result<Self, String> {
        let screen = TerminalScreen::new(Emulator::new(FIRST_SIZE.cols, FIRST_SIZE.rows, 0)?);
        let content = editor.block_content::<TerminalContent>();
        let writer = content.project(|document| document.field(ObjectId::ROOT, Terminal::HOST));
        let shell = content.project(|document| document.field(ObjectId::ROOT, Terminal::SHELL));
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
        let shown = content.project(shown);
        create_effect(move || {
            let (rows, background) = shown.get();
            screen.show(rows, background);
        });
        let session = Self {
            editor,
            screen,
            host,
            shell,
            wanted: create_signal(FIRST_SIZE),
        };
        let client =
            create_memo(move || host.with(|host| host.as_ref().map(|(client, _)| *client)));
        create_effect(move || {
            if client.get().is_some() {
                untrack(|| session.resized());
            }
        });
        Ok(session)
    }

    fn send(&self, message: TerminalMessage) {
        if let Some((client, _)) = self.host.get_untracked() {
            self.editor.send_to_peers(Some(client), &message);
        }
    }

    fn resized(&self) {
        if self.editor.editable().get_untracked() {
            self.send(TerminalMessage::Resize(self.wanted.get_untracked()));
        }
    }

    pub(crate) fn input(&self, input: TerminalInput) {
        if !self.editor.editable().get_untracked() {
            return;
        }
        self.send(match input {
            TerminalInput::Key(key) => TerminalMessage::Key(key),
            TerminalInput::Paste(text) => TerminalMessage::Paste(text),
        });
    }

    pub(crate) fn fit(&self, fit: Fit) {
        let size = TerminalSize {
            cols: fit.cols,
            rows: fit.rows,
        };
        if self.wanted.get_untracked() == size {
            return;
        }
        self.wanted.set(size);
        self.resized();
    }
}

fn shown(document: &TerminalContent) -> (Vec<Row>, Color32) {
    let colors = document.field(ObjectId::ROOT, Terminal::COLORS);
    let cursor = document.field(ObjectId::ROOT, Terminal::CURSOR);
    let alternate = document.field(ObjectId::ROOT, Terminal::ALTERNATE_ACTIVE);
    let field = match alternate {
        true => Terminal::ALTERNATE,
        false => Terminal::SCREEN,
    };
    let rows = document
        .ids(ObjectId::ROOT, field)
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let line = document.read::<TerminalLine>(id).unwrap_or_default();
            let caret = (cursor.visible && usize::from(cursor.row) == index)
                .then_some(cursor.byte as usize);
            spans(&line, caret, colors.foreground, colors.background)
        })
        .collect();
    (rows, color_of(colors.background))
}

fn spans(
    line: &TerminalLine,
    caret: Option<usize>,
    foreground: TerminalColor,
    background: TerminalColor,
) -> Row {
    let mut text = line.text.clone();
    if let Some(caret) = caret {
        while text.len() <= caret {
            text.push(' ');
        }
    }
    let mut padded = line.clone();
    padded.text = text;
    let mut row = Vec::new();
    let mut at = 0;
    for (piece, style) in padded.spans() {
        let mut span = |text: &str, inverted: bool| {
            let mut color = color_of(style.foreground.unwrap_or(foreground));
            let mut fill = style.background.map(color_of);
            if inverted {
                fill = Some(color);
                color = color_of(style.background.unwrap_or(background));
            }
            row.push(Span {
                text: text.to_owned(),
                color,
                background: fill,
                bold: style.bold,
                italic: style.italic,
                underline: style.underline || style.strikethrough,
            });
        };
        let end = at + piece.len();
        match caret.filter(|caret| (at..end).contains(caret)) {
            Some(caret) => {
                let split = caret - at;
                let after = piece[split..]
                    .char_indices()
                    .nth(1)
                    .map_or(piece.len(), |(index, _)| split + index);
                if split > 0 {
                    span(&piece[..split], false);
                }
                span(&piece[split..after], true);
                if after < piece.len() {
                    span(&piece[after..], false);
                }
            }
            None => span(piece, false),
        }
        at = end;
    }
    row
}

fn color_of(color: TerminalColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}
