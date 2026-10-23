use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::be_block::terminal::{
    TerminalContent, TerminalHost, TerminalMessage, TerminalSize,
};
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use block_editor_beui::{EditorHost, OwnedEditor, PeerPresence};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::{TerminalApp, setup_command};

mod a_gap_in_the_output_asks_the_host_for_the_screen_again;
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

    fn host_says(&mut self, message: TerminalMessage) {
        self.editor.receive_from_peer(HOST, &message);
        self.editor.run();
    }

    fn snapshot_of(&mut self, screen: &str) {
        self.host_says(TerminalMessage::Snapshot {
            shell: 1,
            sequence: 4,
            size: TerminalSize { cols: 80, rows: 24 },
            screen: screen.as_bytes().to_vec(),
        });
    }

    fn shown(&mut self, test_id: &str) -> String {
        let node = self
            .editor
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing has the test id {test_id}"));
        self.editor
            .document()
            .node_detail(node)
            .unwrap_or_default()
    }
}

fn attaches(sent: &[TerminalMessage]) -> usize {
    sent.iter()
        .filter(|message| matches!(message, TerminalMessage::Attach(_)))
        .count()
}
