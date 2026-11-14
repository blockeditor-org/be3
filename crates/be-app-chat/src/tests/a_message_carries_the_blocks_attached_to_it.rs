use be_sdk_beui::be_block_content::be_block_model::BlockRef;
use be_sdk_test::HostCall;

use super::*;

#[test]
fn a_message_carries_the_blocks_attached_to_it() {
    // Rosa sends a block this account cannot read: its link says so and does not open.
    let hidden = Uuid::from_u128(0x41dd);
    let mut chat = Harness::new(&[]);
    let send = signed_by(
        &chat.content(),
        ROSA,
        Message {
            attachments: vec![BlockRef(hidden)],
            ..Message::new(ROSA, MORNING, "Here are the plans")
        },
    );
    chat.arrive(send);
    chat.editor.block_types(vec![BlockTypeDescriptor {
        block_type: TEXT.into_bytes(),
        display_name: "Text".to_owned(),
        color: None,
        icon_codepoint: "\u{e0cc}".to_owned(),
        children: ChildOperations::default(),
        press: Press::Open,
        plain_file: true,
    }]);
    let rosas = chat.id(0);
    assert!(
        chat.editor
            .label(&format!("chat.message.{rosas}.attachment.{hidden}"))
            .contains("No access"),
        "a block this account cannot read is marked so, not linked"
    );

    let plans = Uuid::from_u128(0x91a5);
    let mut info = BlockInfo::new(plans, TEXT, BlockParent::Root);
    info.set_name("Floor plans");
    chat.editor.store().add_block(info);
    let sketch = Uuid::from_u128(0x5c7c);
    let mut info = BlockInfo::new(sketch, TEXT, BlockParent::Block(CHAT));
    info.set_name("Sketch");
    chat.editor.store().add_block(info);

    // An existing block is a reference: nothing is copied or moved, and removing it from the
    // draft deletes nothing.
    attach(&mut chat, plans, true);
    assert!(
        chat.editor
            .take_requests()
            .iter()
            .all(|call| !matches!(call, HostCall::CopyBlocks { .. })),
        "an existing block is not copied"
    );
    chat.editor.take_block_commands();
    chat.click(&format!("chat.attaching.{plans}.remove"));
    assert_eq!(deleted(&mut chat), Vec::<Uuid>::new());

    // A block made in the picker is the chat's child, listed as a draft; removing it from the
    // draft takes it out of the chat, which deletes it.
    attach(&mut chat, sketch, false);
    assert_eq!(chat.content().drafts_of(ME), [sketch]);
    assert!(chat.content().references().contains(&sketch));
    chat.click(&format!("chat.attaching.{sketch}.remove"));
    assert!(
        !chat.content().references().contains(&sketch),
        "a block made for the draft goes with it"
    );
    assert!(chat.content().drafts_of(ME).is_empty());

    attach(&mut chat, plans, true);
    attach(&mut chat, sketch, false);
    assert!(
        chat.editor
            .label(&format!("chat.attaching.{plans}"))
            .contains("Floor plans")
    );
    chat.snapshot("a_draft_shows_the_blocks_it_will_carry");

    // A message of attachments alone is sent: the body may be empty.
    chat.click("chat.send");
    chat.editor.advance(std::time::Duration::from_millis(500));
    chat.run();
    let sent = chat.id(1);
    assert_eq!(
        chat.content()
            .message(sent)
            .map(|message| message.attachments),
        Some(vec![BlockRef(plans), BlockRef(sketch)])
    );
    assert_eq!(
        chat.editor
            .store()
            .block(plans)
            .and_then(|info| info.parent),
        Some(BlockParent::Root),
        "the referenced block stays where it was"
    );
    assert!(
        !chat.editor.shown(&format!("chat.attaching.{plans}")),
        "the composer lets the attachments go once sent"
    );
    assert!(
        chat.content().drafts_of(ME).is_empty(),
        "what a message carries is no longer a draft"
    );
    assert!(chat.content().references().contains(&sketch));
    assert!(
        chat.editor
            .label(&format!("chat.message.{sent}.attachment.{plans}"))
            .contains("Floor plans"),
        "the message links what it carries"
    );
    assert!(
        !chat.editor.shown(&format!("chat.message.{sent}.edit")),
        "a message with no text has none to edit"
    );
    chat.snapshot("a_message_carries_the_blocks_attached_to_it");
}
