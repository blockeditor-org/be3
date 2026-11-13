use super::*;
use be_sdk_beui::beui::Input;
use be_sdk_beui::beui::clipboard::{CanvasItems, ClipFormat as _};

#[test]
fn a_canvas_paste_refuses_refs_that_do_not_match_its_blocks() {
    let mut shown = card();
    shown.block = Some(Uuid::nil());
    let mut editor = editor(&[]);
    editor.click(editor.rect_of("infinite-canvas.canvas").center());
    editor.run();
    let items = CanvasItems {
        canvas: Uuid::new_v4(),
        blocks: vec![Uuid::from_u128(5), Uuid::from_u128(6)],
        entities: serde_json::to_vec(&[shown]).expect("entities encode"),
    };
    editor.step(vec![Input::Paste(items.encode())]);
    editor.run();
    assert!(entities(&editor).is_empty(), "nothing is pasted");
    assert!(
        !editor
            .take_requests()
            .iter()
            .any(|call| matches!(call, be_sdk_test::HostCall::PasteBlocks { .. })),
        "the host is not asked to paste blocks the clip cannot name"
    );
}
