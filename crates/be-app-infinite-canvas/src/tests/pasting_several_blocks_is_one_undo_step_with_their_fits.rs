use super::*;
use be_sdk_beui::beui::Input;
use be_sdk_beui::beui::clipboard::{Blocks, ClipBlock, ClipFormat as _};
use be_sdk_beui::Vec2;
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_test::HostCall;

use super::a_dropped_card_takes_its_childs_size_in_the_step_that_added_it::report;

fn block(id: u128) -> ClipBlock {
    ClipBlock {
        id: Uuid::from_u128(id),
        organization: None,
        block_type: Uuid::from_u128(77),
        title: format!("Block {id}"),
    }
}

#[test]
fn pasting_several_blocks_is_one_undo_step_with_their_fits() {
    let known = |id: u128| {
        BlockInfo::new(
            Uuid::from_u128(id),
            CanvasContent::CONTENT_TYPE,
            BlockParent::Root,
        )
    };
    let mut editor = open(&Canvas::default(), false, &[known(1), known(2)]);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    editor.click(center);
    let blocks = Blocks {
        blocks: vec![block(1), block(2)],
    };
    editor.step(vec![Input::Paste(blocks.encode())]);
    editor.run();
    let reply = editor
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PasteBlocks { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("the paste asks the host");
    let pasted = |id: u128| be_sdk_beui::PastedBlock {
        block_id: Uuid::from_u128(id).into_bytes(),
        block_type: Uuid::from_u128(77).into_bytes(),
        moved: false,
        cut_from: None,
    };
    editor.answer(reply, Ok(vec![pasted(1), pasted(2)]));
    editor.run();
    let placeholders = entities(&editor);
    assert_eq!(placeholders.len(), 2, "both blocks are placed");

    report(&mut editor, Some(Vec2::new(300.0, 200.0)));
    let fitted = entities(&editor);
    assert!(
        fitted.iter().all(|card| !placeholders.contains(card)),
        "both cards are fitted to their children: {fitted:?}"
    );

    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        [],
        "one undo takes the whole paste away, fits and all"
    );
}
