use be_sdk_beui::be_block::{Edit, LiveEdit, ObjectId};
use be_sdk_beui::beui::vec2;
use be_sdk_beui::{EditorHost, OwnedEditor, Signer};
use be_sdk_test::BeuiTest;
use be_type_chat::{ChatContent, Message, Signature};
use uuid::Uuid;

use crate::root::ChatRoot;

mod a_conversation_shows_each_message_with_its_author_and_time;
mod a_draft_stays_in_the_editor_until_it_is_sent;
mod a_message_shows_whether_its_author_signed_it;
mod a_reply_or_edit_ends_when_its_message_is_deleted_or_the_other_starts;
mod a_reply_quotes_the_message_it_answers;
mod a_revision_preview_verifies_what_the_chat_signed;
mod messages_are_sent_edited_reacted_to_and_deleted;
mod reactions_show_under_a_message_with_their_counts;
mod reading_on_one_device_clears_the_count_on_another;

const ME: Uuid = Uuid::from_u128(0x5a1d_0000_0000_4000_8000_0000_0000_0001);
const ROSA: Uuid = Uuid::from_u128(0x7e1a_0000_0000_4000_8000_0000_0000_0002);
const CHAT: Uuid = Uuid::from_u128(0xc4a7_0000_0000_4000_8000_0000_0000_0003);
const MINUTE: i64 = 60_000;
/// 09:00 UTC on the day the test wall clock is pinned to, 41 minutes before it.
const MORNING: i64 = 1_772_442_000_000;

struct Harness {
    editor: BeuiTest<ChatRoot>,
}

/// Sends `message` as `signer` signed it: its author, unless the test forges it.
fn signed_by(content: &ChatContent, signer: Uuid, message: Message) -> Edit {
    let (id, message) = content.draft(CHAT, message);
    let signature = be_sdk_test::signature(signer, &message.signed_bytes(CHAT, id));
    ChatContent::send_as(
        id,
        &Message {
            signature: Signature(signature),
            ..message
        },
    )
}

impl Harness {
    fn new(messages: &[(Uuid, &str)]) -> Self {
        Self::trusting(messages, &[])
    }

    /// A chat whose members' keys `verify` trusts as `trusted` says, and any other's as a
    /// contact's.
    fn trusting(messages: &[(Uuid, &str)], trusted: &[(Uuid, Signer)]) -> Self {
        Self::shown_as(messages, trusted, CHAT)
    }

    /// A revision preview of the chat: the editor shows block `shown`, while what its
    /// messages signed names `CHAT`, which the host says signatures bind to.
    fn previewing(messages: &[(Uuid, &str)]) -> Self {
        Self::shown_as(
            messages,
            &[],
            Uuid::from_u128(0x9e51_0000_0000_4000_8000_0000_0000_0009),
        )
    }

    fn shown_as(messages: &[(Uuid, &str)], trusted: &[(Uuid, Signer)], shown: Uuid) -> Self {
        let mut content = ChatContent::default();
        for (index, (author, body)) in messages.iter().enumerate() {
            let time = MORNING + index as i64 * 3 * MINUTE;
            let send = signed_by(&content, *author, Message::new(*author, time, *body));
            content.apply(&send);
        }
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_account_id(ME);
        let mut harness = Self {
            editor: match shown == CHAT {
                true => BeuiTest::new(OwnedEditor::new(host, shown)),
                false => BeuiTest::in_revision_of(OwnedEditor::new(host, shown), CHAT),
            },
        };
        for (account, signer) in trusted {
            harness.editor.set_signer(*account, *signer);
        }
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

    fn send(&mut self, text: &str) {
        self.write(text);
        self.click("chat.send");
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
