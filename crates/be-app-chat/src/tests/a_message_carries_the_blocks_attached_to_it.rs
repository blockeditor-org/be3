use be_sdk_beui::{
    BlockCommand, BlockInfo, BlockLocation, BlockParent, BlockTypeDescriptor, ChildOperations,
    ImportedBlock, PickedBlock, Press,
};
use be_sdk_test::HostCall;

use super::*;

const TEXT: Uuid = Uuid::from_u128(0x7e47);

/// Picks `source` to attach, and has the host copy it into the chat as `copy`.
fn attach(chat: &mut Harness, source: Uuid, copy: Uuid, name: &str) {
    chat.click("chat.attach");
    chat.editor.answer_picker(Ok(Some(PickedBlock {
        block_id: source.into_bytes(),
        block_type: TEXT.into_bytes(),
        linked: false,
        named: true,
        placed: false,
    })));
    chat.run();
    let asked: Vec<_> = chat
        .editor
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::CopyBlocks {
                reply,
                source,
                parent,
            } => Some((reply, Uuid::from_bytes(source), parent)),
            _ => None,
        })
        .collect();
    let [(reply, asked, parent)]: [_; 1] = asked
        .try_into()
        .unwrap_or_else(|asked: Vec<_>| panic!("attaching asks for one copy, asked {asked:?}"));
    assert_eq!(asked, source, "the picked block is copied");
    assert_eq!(
        parent,
        BlockLocation::Unfiled,
        "into Unfiled, until the message carrying it is sent"
    );
    let mut info = BlockInfo::new(copy, TEXT, BlockParent::Unfiled);
    info.set_name(name);
    chat.editor.store().add_block(info);
    chat.editor.answer(
        reply,
        Ok(Some(ImportedBlock {
            block_id: copy.into_bytes(),
            block_type: TEXT.into_bytes(),
        })),
    );
    chat.run();
}

#[test]
fn a_message_carries_the_blocks_attached_to_it() {
    let mut chat = Harness::new(&[(ROSA, "Here are the plans?")]);
    chat.editor.block_types(vec![BlockTypeDescriptor {
        block_type: TEXT.into_bytes(),
        display_name: "Text".to_owned(),
        color: None,
        icon_codepoint: "\u{e0cc}".to_owned(),
        children: ChildOperations::default(),
        press: Press::Open,
        plain_file: true,
    }]);
    let plans = Uuid::from_u128(0x91a5);
    let mut info = BlockInfo::new(plans, TEXT, BlockParent::Root);
    info.set_name("Floor plans");
    chat.editor.store().add_block(info);

    // A copy is attached; removing it from the draft deletes the copy.
    let first = Uuid::from_u128(0xc0b1);
    attach(&mut chat, plans, first, "Floor plans");
    chat.editor.take_block_commands();
    chat.click(&format!("chat.attaching.{plans}.remove"));
    let deleted: Vec<Uuid> = chat
        .editor
        .take_block_commands()
        .into_iter()
        .filter_map(|(block, command)| matches!(command, BlockCommand::Delete { .. }).then_some(block))
        .collect();
    assert_eq!(deleted, [first], "a copy no message carries is deleted");

    let copy = Uuid::from_u128(0xc0b2);
    attach(&mut chat, plans, copy, "Floor plans");
    assert_eq!(
        chat.editor.store().block(plans).and_then(|info| info.parent),
        Some(BlockParent::Root),
        "the picked block stays where it was"
    );
    assert!(
        chat.editor
            .label(&format!("chat.attaching.{plans}"))
            .contains("Floor plans"),
        "the draft shows the copy it will carry"
    );
    chat.snapshot("a_draft_shows_the_copy_it_will_attach");

    // A message of attachments alone is sent: the body may be empty.
    chat.click("chat.send");
    chat.editor.advance(std::time::Duration::from_millis(500));
    chat.run();
    let sent = chat.id(1);
    assert_eq!(
        chat.content()
            .message(sent)
            .map(|message| message.attachments),
        Some(vec![be_sdk_beui::be_block::be_block_model::BlockRef(copy)]),
        "the message names the copy, not the picked block"
    );
    assert_eq!(
        chat.editor.store().block(copy).and_then(|info| info.parent),
        Some(BlockParent::Block(CHAT)),
        "the copy moves into the chat with the message"
    );
    assert!(
        !chat.editor.shown(&format!("chat.attaching.{plans}")),
        "the composer lets the attachment go once sent"
    );
    assert!(
        chat.editor
            .label(&format!("chat.message.{sent}.attachment.{copy}"))
            .contains("Floor plans"),
        "the message links what it carries"
    );
    assert!(
        !chat.editor.shown(&format!("chat.message.{sent}.edit")),
        "a message with no text has none to edit"
    );
    chat.snapshot("a_message_carries_the_blocks_attached_to_it");
}
