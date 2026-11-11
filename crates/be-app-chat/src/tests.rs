use be_sdk_beui::be_block::{Edit, LiveEdit, ObjectId};
use be_sdk_beui::beui::vec2;
use be_sdk_beui::{EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_type_chat::{ChatContent, Message};
use uuid::Uuid;

use std::time::Duration;

use crate::root::ChatRoot;

mod a_conversation_shows_each_message_with_its_author_and_time;
mod a_draft_stays_in_the_editor_until_it_is_sent;
mod a_message_kept_offline_is_sent_once_the_drain_publishes_it_while_the_chat_is_closed;
mod a_message_shows_as_sending_until_it_is_delivered;
mod a_message_the_server_refuses_shows_as_not_sent;
mod a_discarded_message_does_not_return_as_not_sent_when_the_chat_opens_again;
mod a_reply_or_edit_ends_when_its_message_is_deleted_or_the_other_starts;
mod a_reply_quotes_the_message_it_answers;
mod messages_are_sent_edited_reacted_to_and_deleted;
mod reactions_show_under_a_message_with_their_counts;
mod reading_on_one_device_clears_the_count_on_another;

const ME: Uuid = Uuid::from_u128(0x5a1d_0000_0000_4000_8000_0000_0000_0001);
const ROSA: Uuid = Uuid::from_u128(0x7e1a_0000_0000_4000_8000_0000_0000_0002);
const CHAT: Uuid = Uuid::from_u128(0xc4a7_0000_0000_4000_8000_0000_0000_0003);
const MINUTE: i64 = 60_000;
/// 09:00 UTC on the day the test wall clock is pinned to, 41 minutes before it.
const MORNING: i64 = 1_772_442_000_000;

fn host() -> EditorHost {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_account_id(ME);
    host
}

struct Harness {
    editor: BeuiTest<ChatRoot>,
}

impl Harness {
    fn new(messages: &[(Uuid, &str)]) -> Self {
        let mut content = ChatContent::default();
        for (index, (author, body)) in messages.iter().enumerate() {
            let time = MORNING + index as i64 * 3 * MINUTE;
            let (_, send) = content.send(CHAT, Message::new(*author, time, *body));
            content.apply(&send);
        }
        let mut harness = Self {
            editor: BeuiTest::new(OwnedEditor::new(host(), CHAT)),
        };
        harness
            .editor
            .store()
            .register(be_type_chat::registrations());
        harness.editor.hold(None, content);
        harness.editor.set_size(vec2(480.0, 560.0));
        harness.editor.presence_visible(true);
        harness.run();
        harness
    }

    /// The same chat opened again in a new editor, as after switching to another chat and
    /// back: the block's replica is the one this editor followed.
    fn reopened(&self) -> Self {
        let mut harness = Self {
            editor: BeuiTest::new_beside(&self.editor, OwnedEditor::new(host(), CHAT)),
        };
        harness.editor.set_size(vec2(480.0, 560.0));
        harness.editor.presence_visible(true);
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn arrive(&mut self, operation: Edit) {
        self.editor.edit::<ChatContent>(None, &operation);
        self.run();
    }

    fn click(&mut self, test_id: &str) {
        self.editor.click(test_id);
        self.run();
    }

    fn type_text(&mut self, text: &str) {
        self.editor.type_text(text);
        self.run();
    }

    fn write(&mut self, text: &str) {
        self.click("chat.draft");
        self.type_text(text);
    }

    /// Writes and sends `text`, and lets the server take it.
    fn send(&mut self, text: &str) {
        self.write(text);
        self.click("chat.send");
        self.editor.advance(std::time::Duration::from_millis(500));
        self.run();
    }

    fn content(&self) -> ChatContent {
        self.editor.content(None)
    }

    fn id(&self, index: usize) -> ObjectId {
        self.content().messages()[index]
    }

    fn bodies(&self) -> Vec<String> {
        let content = self.content();
        content
            .messages()
            .into_iter()
            .filter_map(|id| content.body(id))
            .collect()
    }

    /// The text a message's row shows, without its buttons' icons.
    fn row(&self, message: ObjectId) -> String {
        let label = self.editor.label(&format!("chat.message.{message}"));
        label
            .split(' ')
            .filter(|word| !word.chars().all(|c| ('\u{e000}'..='\u{f8ff}').contains(&c)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn snapshot(&mut self, name: &str) {
        self.editor.snapshot(name);
    }
}
