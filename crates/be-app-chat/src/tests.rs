use be_sdk_beui::be_block_content::{BlockContent, Edit, LiveEdit, ObjectId};
use be_sdk_beui::beui::vec2;
use be_sdk_beui::{
    BlockCommand, BlockInfo, BlockParent, BlockTypeDescriptor, ChildOperations, EditorHost,
    OwnedEditor, PickedBlock, Press, Signer,
};
use be_sdk_test::BeuiTest;
use be_type_chat::{ChatContent, ChatOp, Message, Signature};
use uuid::Uuid;

use std::time::Duration;

use crate::root::ChatRoot;

mod a_conversation_shows_each_message_with_its_author_and_time;
mod a_discarded_message_does_not_return_as_not_sent_when_the_chat_opens_again;
mod a_draft_stays_in_the_editor_until_it_is_sent;
mod a_drafts_made_blocks_come_back_as_chips_when_the_chat_opens_again;
mod a_forged_deletion_of_a_closed_message_shows_where_it_was;
mod a_long_chat_closes_its_oldest_messages_into_a_segment_and_reads_them_back_on_request;
mod a_made_block_moved_out_of_the_chat_survives_the_composer;
mod a_message_carries_the_blocks_attached_to_it;
mod a_message_in_a_closed_segment_shows_an_edit_made_since_it_closed;
mod a_message_kept_offline_is_sent_once_the_drain_publishes_it_while_the_chat_is_closed;
mod a_message_kept_offline_tells_the_other_members_even_when_the_chat_is_closed_as_it_drains;
mod a_message_kept_offline_tells_the_other_members_only_once_it_reaches_the_server;
mod a_message_sent_after_a_discarded_one_does_not_show_as_not_sent_when_the_chat_opens_again;
mod a_message_sent_after_a_discarded_one_is_sent_once_its_commit_drains_while_the_chat_is_closed;
mod a_message_sent_after_a_discarded_one_shows_as_not_sent_when_it_is_refused_while_the_chat_is_closed;
mod a_message_sent_with_two_editors_of_the_chat_open_tells_once;
mod a_message_shows_as_sending_until_it_is_delivered;
mod a_message_shows_whether_its_author_signed_it;
mod a_message_the_server_refuses_shows_as_not_sent;
mod a_refused_message_tells_nobody_and_a_reply_mentions_its_author;
mod a_replayed_edit_a_forged_deletion_and_a_reaction_taken_back_by_someone_else_show_as_forged;
mod a_reply_or_edit_ends_when_its_message_is_deleted_or_the_other_starts;
mod a_reply_quotes_the_message_it_answers;
mod a_revision_preview_verifies_what_the_chat_signed;
mod a_sent_message_tells_the_chats_other_members_once_the_server_holds_it;
mod an_edited_message_tells_nobody_again_and_a_sent_again_one_tells_once;
mod an_image_pasted_into_the_composer_is_made_in_the_chat_and_attached;
mod messages_are_sent_edited_reacted_to_and_deleted;
mod reactions_show_under_a_message_with_their_counts;
mod reading_on_one_device_clears_the_count_on_another;
mod two_closed_segments_that_overlap_show_each_message_once;

const TEXT: Uuid = Uuid::from_u128(0x7e47);

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

/// `message` deleted by `author`, who signed the deletion.
fn deleted_by(content: &ChatContent, message: ObjectId, author: Uuid, time: i64) -> Edit {
    let deletion = content.deletion(message, author, time);
    let signature = be_sdk_test::signature(author, &deletion.signed_bytes(CHAT, message));
    content.put_deletion(
        message,
        &be_type_chat::Deletion {
            signature: Signature(signature),
            ..deletion
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
            content.apply(&send.into());
        }
        Self::open(content, trusted, shown, false)
    }

    /// The editor of a chat whose closed segments are saved as parts, which it reads by range.
    fn holding_parts(content: ChatContent) -> Self {
        Self::open(content, &[], CHAT, true)
    }

    fn open(content: ChatContent, trusted: &[(Uuid, Signer)], shown: Uuid, parts: bool) -> Self {
        let mut harness = Self {
            editor: match shown == CHAT {
                true => BeuiTest::new(OwnedEditor::new(host(), shown)),
                false => BeuiTest::in_revision_of(OwnedEditor::new(host(), shown), CHAT),
            },
        };
        for (account, signer) in trusted {
            harness.editor.set_signer(*account, *signer);
        }
        harness
            .editor
            .store()
            .register(be_type_chat::registrations());
        match parts {
            true => harness.editor.hold_parts(None, content),
            false => harness.editor.hold(None, content),
        }
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
        self.editor
            .edit::<ChatContent>(None, &ChatOp::Edit(operation));
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

/// Picks `block` to attach: an existing block (`linked`), or one the picker made in the chat.
fn attach(chat: &mut Harness, block: Uuid, linked: bool) {
    chat.click("chat.attach");
    chat.editor.answer_picker(Ok(Some(PickedBlock {
        block_id: block.into_bytes(),
        block_type: TEXT.into_bytes(),
        linked,
        named: true,
        placed: false,
    })));
    chat.run();
}

fn deleted(chat: &mut Harness) -> Vec<Uuid> {
    chat.editor
        .take_block_commands()
        .into_iter()
        .filter_map(|(block, command)| {
            matches!(command, BlockCommand::Delete { .. }).then_some(block)
        })
        .collect()
}

/// A text block named `name` made in the chat, as the picker makes one, with the type the
/// editor names it by.
fn made_in_chat(chat: &mut Harness, block: Uuid, name: &str) {
    chat.editor.block_types(vec![BlockTypeDescriptor {
        block_type: TEXT.into_bytes(),
        display_name: "Text".to_owned(),
        color: None,
        icon_codepoint: "\u{e0cc}".to_owned(),
        children: ChildOperations::default(),
        press: Press::Open,
        plain_file: true,
    }]);
    let mut info = BlockInfo::new(block, TEXT, BlockParent::Block(CHAT));
    info.set_name(name);
    chat.editor.store().add_block(info);
}
