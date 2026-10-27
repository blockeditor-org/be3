use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::be_model::{Anchor, FieldRef, List};
use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::beui::native::Document;
use block_editor_beui::beui::{Key, Modifiers, NodeId, PointerButton, Pos2, Rect, Vec2};
use block_editor_beui::{EditorHost, OwnedEditor, PeerPresence};
use block_ui_test::BeuiTest;
use terminal_types::{
    Terminal, TerminalColor, TerminalColors, TerminalContent, TerminalCursor, TerminalHost,
    TerminalLine, TerminalMessage, TerminalShell, TerminalSize, TerminalStyle,
};
use uuid::Uuid;

use crate::app::{TerminalApp, setup_command};

mod a_selection_copies_a_wrapped_line_as_one_and_select_all_copies_unbuilt_history;
mod keys_and_text_reach_the_host_as_input;
mod the_alternate_screen_sends_the_wheel_and_peeks_at_the_main_screen;
mod the_terminal_keeps_its_scrollback_and_says_so_when_the_host_leaves;
mod the_terminal_says_its_machine_is_offline_and_how_to_start_one;
mod the_terminal_shows_history_and_a_wrapped_line_and_scrolls_back;

const HOST: u64 = 41;

struct Harness {
    editor: BeuiTest<TerminalApp>,
    block: Uuid,
}

#[derive(Clone, Default)]
struct Screen {
    history: Vec<TerminalLine>,
    rows: Vec<TerminalLine>,
    alternate: Option<Vec<TerminalLine>>,
}

impl Screen {
    fn rows(rows: Vec<TerminalLine>) -> Self {
        Self {
            rows,
            ..Self::default()
        }
    }

    fn history(mut self, history: Vec<TerminalLine>) -> Self {
        self.history = history;
        self
    }

    fn alternate(mut self, rows: Vec<TerminalLine>) -> Self {
        self.alternate = Some(rows);
        self
    }
}

fn insert(
    content: &mut TerminalContent,
    field: FieldRef<Terminal, List<TerminalLine>>,
    lines: &[TerminalLine],
) {
    for line in lines {
        content.apply(&field.insert(ObjectId::ROOT, Anchor::End, line).1.into());
    }
}

impl Harness {
    fn new(editable: bool) -> Self {
        let block = Uuid::from_u128(0x0192_7a3c_5e10_7b2d_9c4e_61f0_a8d3_2b57);
        let host = EditorHost::default();
        host.set_editable(editable);
        let editor = OwnedEditor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            block,
        };
        harness.editor.set_size(Vec2::new(720.0, 420.0));
        harness.editor.hold(None, TerminalContent::default());
        harness.editor.run();
        harness
    }

    fn show(&mut self, rows: &[TerminalLine]) {
        self.hold(Screen::rows(rows.to_vec()));
    }

    fn hold(&mut self, screen: Screen) {
        let mut content = TerminalContent::default();
        content.apply(&Terminal::HOST.set(ObjectId::ROOT, &Some(HOST)).into());
        content.apply(
            &Terminal::SHELL
                .set(
                    ObjectId::ROOT,
                    &Some(TerminalShell {
                        id: 1,
                        running: true,
                        status: None,
                    }),
                )
                .into(),
        );
        insert(&mut content, Terminal::HISTORY, &screen.history);
        insert(&mut content, Terminal::SCREEN, &screen.rows);
        if let Some(alternate) = &screen.alternate {
            insert(&mut content, Terminal::ALTERNATE, alternate);
            content.apply(&Terminal::ALTERNATE_ACTIVE.set(ObjectId::ROOT, &true).into());
        }
        content.apply(
            &Terminal::COLORS
                .set(
                    ObjectId::ROOT,
                    &TerminalColors {
                        foreground: TerminalColor {
                            r: 220,
                            g: 220,
                            b: 220,
                        },
                        background: TerminalColor {
                            r: 16,
                            g: 18,
                            b: 24,
                        },
                    },
                )
                .into(),
        );
        let shown = screen.alternate.as_ref().unwrap_or(&screen.rows);
        let last = shown.len().saturating_sub(1);
        content.apply(
            &Terminal::CURSOR
                .set(
                    ObjectId::ROOT,
                    &TerminalCursor {
                        row: last as u16,
                        byte: shown.last().map_or(0, |row| row.text.len() as u32),
                        visible: true,
                        ..TerminalCursor::default()
                    },
                )
                .into(),
        );
        self.editor.hold(None, content);
        self.editor.run();
    }

    fn connect_host(&mut self) {
        let shown = TerminalHost {
            machine: "build-box".to_owned(),
            user: "ada".to_owned(),
        };
        self.editor.set_peers(
            None,
            vec![PeerPresence {
                client: HOST,
                kind: TerminalHost::ID,
                value: serde_json::to_vec(&shown).unwrap(),
            }],
        );
        self.editor.run();
    }

    fn disconnect_host(&mut self) {
        self.editor.set_peers(None, Vec::new());
        self.editor.run();
    }

    fn sent(&mut self) -> Vec<TerminalMessage> {
        self.editor
            .take_sent_to_peers::<TerminalMessage>()
            .into_iter()
            .map(|(to, message)| {
                assert_eq!(to, Some(HOST), "everything goes to the host");
                message
            })
            .collect()
    }

    fn size(&mut self) -> TerminalSize {
        self.sent()
            .into_iter()
            .filter_map(|message| match message {
                TerminalMessage::Resize(size) => Some(size),
                _ => None,
            })
            .next_back()
            .expect("the terminal told the host its size")
    }

    fn shown(&mut self, test_id: &str) -> String {
        let node = self
            .editor
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing has the test id {test_id}"));
        self.editor.document().node_detail(node).unwrap_or_default()
    }

    fn says(&self, words: &str) -> bool {
        let document = self.editor.document();
        document
            .root()
            .and_then(|root| text_within(&document, root, words))
            .is_some()
    }

    fn text_rect(&self, words: &str) -> Rect {
        let document = self.editor.document();
        let node = document
            .root()
            .and_then(|root| text_within(&document, root, words))
            .unwrap_or_else(|| panic!("{words:?} is on screen"));
        document.node_rect(node).expect("the text is laid out")
    }

    fn click_text(&mut self, words: &str) {
        let center = self.text_rect(words).center();
        self.editor.click(center);
        self.editor.settle();
    }

    fn right_click(&mut self, at: Pos2) {
        self.editor.click_with(at, PointerButton::Secondary);
        self.editor.settle();
    }

    fn copy(&mut self) -> Vec<String> {
        self.editor.key(Key::C, ctrl_shift());
        self.editor.run();
        self.editor.take_copied_text()
    }
}

fn text_within(document: &Document, id: NodeId, words: &str) -> Option<NodeId> {
    if document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
    {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_within(&document, child, words))
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    }
}

fn ctrl_shift() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    }
}

fn resizes(sent: &[TerminalMessage]) -> usize {
    sent.iter()
        .filter(|message| matches!(message, TerminalMessage::Resize(_)))
        .count()
}

fn plain(text: &str) -> TerminalLine {
    TerminalLine::styled(text, TerminalStyle::default())
}

fn color(r: u8, g: u8, b: u8) -> TerminalStyle {
    TerminalStyle {
        foreground: Some(TerminalColor { r, g, b }),
        ..TerminalStyle::default()
    }
}

fn faint() -> TerminalStyle {
    TerminalStyle {
        faint: true,
        ..TerminalStyle::default()
    }
}

fn prompt() -> TerminalLine {
    let green = TerminalStyle {
        bold: true,
        ..color(80, 200, 120)
    };
    let mut line = TerminalLine::styled("ada@build-box", green);
    line.push(":~$ ", TerminalStyle::default());
    line
}

fn command(text: &str) -> TerminalLine {
    let mut line = prompt();
    line.push(text, TerminalStyle::default());
    line
}

fn listing() -> TerminalLine {
    let blue = TerminalStyle {
        bold: true,
        ..color(90, 140, 255)
    };
    let mut files = plain("Cargo.toml  README.md  ");
    files.push("src", blue);
    files.push("  ", TerminalStyle::default());
    files.push("target", faint());
    files
}

fn log_line(index: usize) -> TerminalLine {
    let mut line = TerminalLine::styled(format!("[{index:04}] "), faint());
    let (word, style) = match index % 5 {
        0 => ("warning", color(230, 180, 60)),
        3 => ("error", color(240, 90, 90)),
        _ => ("compiled", color(120, 200, 140)),
    };
    line.push(word, style);
    line.push(
        &format!(" crate number {index} of the workspace"),
        TerminalStyle::default(),
    );
    line
}

const LONG: &str = "echo the quick brown fox jumps over the lazy dog and keeps on running past the right edge of the terminal until it wraps onto the next row";

fn long_command(cols: u16) -> Vec<TerminalLine> {
    let mut first = prompt();
    let room = usize::from(cols) - first.text.len();
    first.push(&LONG[..room], TerminalStyle::default());
    first.wrapped = true;
    vec![first, plain(&LONG[room..])]
}
