use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::be_model::Anchor;
use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::be_block::terminal::{
    Terminal, TerminalColor, TerminalColors, TerminalContent, TerminalCursor, TerminalHost,
    TerminalLine, TerminalMessage, TerminalShell, TerminalStyle,
};
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use block_editor_beui::{EditorHost, OwnedEditor, PeerPresence};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::{TerminalApp, setup_command};

mod keys_and_text_reach_the_host_as_input;
mod the_terminal_says_its_machine_is_offline_and_how_to_start_one;
mod the_terminal_shows_the_hosts_screen_and_follows_its_output;
mod the_terminal_switches_to_its_setup_when_the_host_leaves;

const HOST: u64 = 41;

struct Harness {
    editor: BeuiTest<TerminalApp>,
    block: Uuid,
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
        for row in rows {
            content.apply(
                &Terminal::SCREEN
                    .insert(ObjectId::ROOT, Anchor::End, row)
                    .1
                    .into(),
            );
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
        let last = rows.len().saturating_sub(1);
        content.apply(
            &Terminal::CURSOR
                .set(
                    ObjectId::ROOT,
                    &TerminalCursor {
                        row: last as u16,
                        byte: rows.last().map_or(0, |row| row.text.len() as u32),
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

    fn shown(&mut self, test_id: &str) -> String {
        let node = self
            .editor
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing has the test id {test_id}"));
        self.editor.document().node_detail(node).unwrap_or_default()
    }
}

fn resizes(sent: &[TerminalMessage]) -> usize {
    sent.iter()
        .filter(|message| matches!(message, TerminalMessage::Resize(_)))
        .count()
}

fn prompt() -> TerminalLine {
    let green = TerminalStyle {
        foreground: Some(TerminalColor {
            r: 80,
            g: 200,
            b: 120,
        }),
        bold: true,
        ..TerminalStyle::default()
    };
    let mut line = TerminalLine::styled("ada@build-box", green);
    line.push(":~$ ", TerminalStyle::default());
    line
}
