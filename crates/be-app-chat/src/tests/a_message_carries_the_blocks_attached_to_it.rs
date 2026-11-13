use be_sdk_beui::{BlockInfo, BlockParent, PickedBlock};

use super::*;

#[test]
fn a_message_carries_the_blocks_attached_to_it() {
    let mut chat = Harness::new(&[(ROSA, "Here are the plans?")]);
    let text_type = Uuid::from_u128(0x7e47);
    let plans = Uuid::from_u128(0x91a5);
    let mut info = BlockInfo::new(plans, text_type, BlockParent::Root);
    info.set_name("Floor plans");
    chat.editor.store().add_block(info);

    chat.click("chat.attach");
    assert!(
        chat.editor.placed_picker().is_some(),
        "attaching picks a block"
    );
    chat.editor.answer_picker(Ok(Some(PickedBlock {
        block_id: plans.into_bytes(),
        block_type: text_type.into_bytes(),
        linked: false,
        named: true,
        placed: false,
    })));
    chat.run();
    assert_eq!(
        chat.editor.store().block(plans).and_then(|info| info.parent),
        Some(BlockParent::Block(CHAT)),
        "the attached block moves into the chat"
    );
    assert!(
        chat.editor
            .label(&format!("chat.attaching.{plans}"))
            .contains("Floor plans"),
        "the draft shows what it will carry"
    );

    // A message of attachments alone is sent: the body may be empty.
    chat.click("chat.send");
    chat.editor.advance(std::time::Duration::from_millis(500));
    chat.run();
    let sent = chat.id(1);
    assert_eq!(
        chat.content()
            .message(sent)
            .map(|message| message.attachments),
        Some(vec![be_sdk_beui::be_block::be_block_model::BlockRef(plans)])
    );
    assert!(
        !chat.editor.shown(&format!("chat.attaching.{plans}")),
        "the composer lets the attachment go once sent"
    );
    assert!(
        chat.editor
            .label(&format!("chat.message.{sent}.attachment.{plans}"))
            .contains("Floor plans"),
        "the message links what it carries"
    );
    chat.snapshot("a_message_carries_the_blocks_attached_to_it");
}
