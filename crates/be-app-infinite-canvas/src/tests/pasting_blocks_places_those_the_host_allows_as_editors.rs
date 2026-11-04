use super::*;
use be_sdk_beui::beui::Input;
use be_sdk_beui::beui::clipboard::{Blocks, ClipBlock, ClipFormat as _};
use be_sdk_test::HostCall;

fn block(id: u128) -> ClipBlock {
    ClipBlock {
        id: Uuid::from_u128(id),
        organization: None,
        block_type: Uuid::from_u128(77),
        title: format!("Block {id}"),
    }
}

#[test]
fn pasting_blocks_places_those_the_host_allows_as_editors() {
    let mut editor = editor(&[]);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    editor.click(center);
    editor.run();
    let blocks = Blocks {
        blocks: vec![block(1), block(2)],
    };
    editor.step(vec![Input::Paste(blocks.encode())]);
    editor.run();
    assert!(
        entities(&editor).is_empty(),
        "nothing is placed before the host answers"
    );

    let asked = editor
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::PasteBlocks { reply, blocks, .. } => Some((reply, blocks)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(reply, asked)]: [_; 1] = asked
        .try_into()
        .unwrap_or_else(|asked: Vec<_>| panic!("the paste asks the host once, asked {asked:?}"));
    assert_eq!(
        asked,
        [
            Uuid::from_u128(1).into_bytes(),
            Uuid::from_u128(2).into_bytes()
        ]
    );
    editor.answer(
        reply,
        Ok(vec![be_sdk_beui::PastedBlock {
            block_id: Uuid::from_u128(2).into_bytes(),
            block_type: Uuid::from_u128(77).into_bytes(),
            moved: false,
        }]),
    );
    editor.run();

    let placed = entities(&editor);
    let [placed] = &placed[..] else {
        panic!("only the block the host allowed is placed, placed {placed:?}");
    };
    assert_eq!(placed.block, Some(Uuid::from_u128(2)));
    assert_eq!(placed.shape, CanvasShape::Editor);
}
