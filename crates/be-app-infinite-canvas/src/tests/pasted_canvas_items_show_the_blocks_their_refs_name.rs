use super::*;
use be_sdk_beui::beui::Input;
use be_sdk_beui::beui::clipboard::{CanvasItems, ClipFormat as _};
use be_sdk_test::HostCall;

#[test]
fn pasted_canvas_items_show_the_blocks_their_refs_name() {
    let mut shown = card();
    shown.shape = CanvasShape::Editor;
    shown.block = Some(Uuid::from_u128(1));
    let mut editor = editor(&[]);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    editor.click(center);
    editor.run();
    let translated = Uuid::from_u128(5);
    let items = CanvasItems {
        canvas: Uuid::new_v4(),
        blocks: vec![translated],
        entities: serde_json::to_vec(&[shown]).expect("entities encode"),
    };
    editor.step(vec![Input::Paste(items.encode())]);
    editor.run();

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
        [translated.into_bytes()],
        "the block comes from the refs, which a host translates, not the entities' own ids"
    );
    editor.answer(
        reply,
        Ok(vec![be_sdk_beui::PastedBlock {
            block_id: translated.into_bytes(),
            block_type: Uuid::from_u128(77).into_bytes(),
            moved: false,
            cut_from: None,
        }]),
    );
    editor.run();
    let placed = entities(&editor);
    let [placed] = &placed[..] else {
        panic!("the entity is placed, placed {placed:?}");
    };
    assert_eq!(placed.block, Some(translated));
}
