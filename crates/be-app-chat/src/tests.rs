use be_sdk_beui::be_block_content::{BlockContent, LogItem, LogSigner};
use be_sdk_beui::beui::vec2;
use be_sdk_beui::{
    BlockCommand, BlockInfo, BlockParent, BlockTypeDescriptor, ChildOperations, EditorHost,
    OwnedEditor, PickedBlock, Press,
};
use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use be_type_chat::{ChatContent, ChatEntry, ChatLog};
use uuid::Uuid;

use std::time::Duration;

use crate::root::ChatRoot;

mod a_conversation_shows_each_message_with_its_author_and_time;
mod a_deletion_or_reaction_the_server_refuses_says_so_on_its_message;
mod a_discarded_message_does_not_return_as_not_sent_when_the_chat_opens_again;
mod a_draft_stays_in_the_editor_until_it_is_sent;
mod a_drafts_made_blocks_come_back_as_chips_when_the_chat_opens_again;
mod a_made_block_moved_out_of_the_chat_survives_the_composer;
mod a_message_carries_the_blocks_attached_to_it;
mod a_message_deleted_with_an_old_key_says_so_in_its_place;
mod a_message_kept_offline_tells_the_other_members_only_once_it_reaches_the_server;
mod a_message_refused_while_the_chat_is_closed_shows_as_not_sent_when_it_opens_again;
mod a_message_removed_by_someone_else_says_so_in_its_place;
mod a_message_sent_after_a_discarded_one_does_not_show_as_not_sent_when_the_chat_opens_again;
mod a_message_sent_as_its_editor_closed_still_carries_its_blocks;
mod a_message_sent_offline_shows_when_it_arrived;
mod a_message_shows_as_sending_until_it_is_delivered;
mod a_message_shows_who_signed_it;
mod a_message_the_server_refuses_shows_as_not_sent;
mod a_reply_or_edit_ends_when_its_message_is_deleted_or_the_other_starts;
mod a_reply_quotes_the_message_it_answers;
mod a_sent_message_tells_the_chats_other_members_once_the_server_holds_it;
mod an_image_pasted_into_the_composer_is_made_in_the_chat_and_attached;
mod earlier_messages_are_read_a_page_at_a_time;
mod imported_history_shows_apart_and_read_only;
mod members_show_by_their_names_and_a_name_the_server_gave_is_marked;
mod messages_are_sent_edited_reacted_to_and_deleted;
mod reactions_show_under_a_message_with_their_counts;
mod reading_on_one_device_clears_the_count_on_another;

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

/// A message written at `time`.
fn message(time: i64, body: &str) -> ChatEntry {
    ChatEntry::Message {
        time,
        body: body.to_owned(),
        reply_to: None,
        attachments: Vec::new(),
    }
}

struct Harness {
    editor: BeuiTest<ChatRoot>,
}

impl Harness {
    fn new(messages: &[(Uuid, &str)]) -> Self {
        Self::trusting(messages, &[])
    }

    /// A chat whose messages were each written three minutes after the last, with who signed
    /// what each member appends as `signers` says, and a contact's key for anyone else.
    fn trusting(messages: &[(Uuid, &str)], signers: &[(Uuid, LogSigner)]) -> Self {
        let mut harness = Self {
            editor: BeuiTest::new(OwnedEditor::new(host(), CHAT)),
        };
        let store = harness.editor.store();
        store.register(be_type_chat::registrations());
        store.hold(None, ChatContent::default());
        for (account, signer) in signers {
            store.set_log_signer(*account, *signer);
        }
        for (index, (author, body)) in messages.iter().enumerate() {
            let time = MORNING + index as i64 * 3 * MINUTE;
            store.log_entry(None, *author, message(time, body).encode(), Some(time));
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

    /// An entry `author` appended on another device, which the server took now.
    fn arrive(&mut self, author: Uuid, entry: &ChatEntry) -> u64 {
        let seq = self
            .editor
            .store()
            .log_entry(None, author, entry.encode(), None);
        self.run();
        seq
    }

    /// An edit of the chat's content made on another device.
    fn arrive_edit(&mut self, edit: be_sdk_beui::be_block_content::Edit) {
        self.editor.edit::<ChatContent>(None, &edit);
        self.run();
    }

    fn content(&self) -> ChatContent {
        self.editor.content(None)
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
        self.editor.advance(Duration::from_millis(500));
        self.editor.run();
        self.run();
    }

    /// The chat's log as the server holds it, folded.
    fn log(&self) -> ChatLog {
        let items: Vec<LogItem> = self
            .editor
            .store()
            .log(None)
            .into_iter()
            .map(|item| LogItem {
                seq: item.seq,
                id: item.id,
                author: item.author,
                arrival: item.arrival,
                signer: match item.payload {
                    Some(_) => LogSigner::Verified,
                    None => LogSigner::Redacted,
                },
                payload: item.payload,
                imported: item
                    .imported
                    .map(|from| be_sdk_beui::be_block_content::LogImport {
                        block: from.block,
                        seq: from.seq,
                        importer: from.importer,
                    }),
            })
            .collect();
        ChatLog::of(&items)
    }

    /// The place of the `index`th message in the log.
    fn seq(&self, index: usize) -> u64 {
        self.log()
            .messages()
            .nth(index)
            .expect("the chat has that many messages")
            .seq
    }

    /// What each message the chat shows says, deleted ones left out.
    fn bodies(&self) -> Vec<String> {
        self.log()
            .messages()
            .filter(|shown| shown.deleted.is_none())
            .filter_map(|shown| shown.body.clone())
            .collect()
    }

    /// The row's key of the `index`th message this device sent that the server holds back.
    fn kept(&self, index: usize) -> String {
        let (id, _) = self.editor.store().held_appends(None)[index].clone();
        format!("kept.{id}")
    }

    /// The text a message's row shows, without its buttons' icons.
    fn row(&self, key: impl std::fmt::Display) -> String {
        let label = self.editor.label(&format!("chat.message.{key}"));
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
