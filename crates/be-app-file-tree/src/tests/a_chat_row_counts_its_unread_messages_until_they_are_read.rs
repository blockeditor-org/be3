use be_sdk_beui::be_block_content::{BlockContent, EditorView};
use be_sdk_beui::{BlockInfo, BlockParent};
use be_type_chat::{ChatContent, ChatEntry, ChatReads, ChatReadsContent};

use super::*;

const ROSA: Uuid = Uuid::from_u128(0x7e1a_0000_0000_4000_8000_0000_0000_0002);
const PHONE: Uuid = Uuid::from_u128(0x9407_0000_0000_4000_8000_0000_0000_0005);

#[test]
fn a_chat_row_counts_its_unread_messages_until_they_are_read() {
    let mut fixture = editor();
    let store = fixture.test.store();
    store.register(be_type_chat::registrations());
    let chat = Uuid::new_v4();
    let lunch = ChatContent::default();
    let mut info = BlockInfo::new(
        chat,
        ChatContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    info.set_name("Lunch");
    let mut notes = BlockInfo::new(
        Uuid::new_v4(),
        Uuid::from_u128(0x6e6f_7465),
        BlockParent::Block(MY_FILES),
    );
    notes.set_name("Notes");
    store.add_block(files_workspace(
        &fixture.host,
        &[info.clone(), notes.clone()],
    ));
    store.add_block(info);
    store.add_block(notes);
    fixture.test.hold(Some(chat), lunch);
    // Rosa's two messages, in its log.
    for (at, body) in [(1, "Who's in?"), (2, "Noon at the usual place")] {
        let message = ChatEntry::Message {
            time: at,
            body: body.to_owned(),
            reply_to: None,
            attachments: Vec::new(),
        };
        store.log_entry(Some(chat), ROSA, message.encode(), Some(at));
    }
    // Where this account read each chat up to: nothing of this one yet.
    let reads = Uuid::new_v4();
    fixture.test.hold(Some(reads), ChatReadsContent::default());
    fixture.test.hold(None, EditorView::document(None));
    fixture.settle();

    let badge = format!("file-tree.{chat}.unread");
    assert!(
        fixture.test.shown(&badge),
        "a chat's row carries a badge while it has messages not read"
    );
    assert!(fixture.says("2"), "the badge counts Rosa's two messages");
    fixture
        .test
        .snapshot("a_chat_row_counts_its_unread_messages_until_they_are_read");

    // Reading the chat on another device moves this account's place to its newest message.
    fixture
        .test
        .edit::<ChatReadsContent>(Some(reads), &ChatReads::set_read(chat, PHONE, 1));
    fixture.settle();
    assert!(
        !fixture.test.shown(&badge),
        "reading the chat clears its row's badge"
    );
}
