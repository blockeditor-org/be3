use super::*;
use be_sdk_beui::DropAction;
use be_sdk_beui::beui::Pos2;
use be_sdk_test::HostCall;

fn dropped(action: DropAction) -> (BeuiTest<CanvasRoot>, Uuid) {
    let block = Uuid::new_v4();
    let mut editor = open(
        &Canvas::default(),
        false,
        &[BlockInfo::new(
            block,
            Uuid::from_u128(77),
            BlockParent::Root,
        )],
    );
    editor.drag_block_as(
        Pos2::new(300.0, 300.0),
        block,
        Uuid::from_u128(77),
        true,
        action,
    );
    editor.run();
    (editor, block)
}

#[test]
fn a_dropped_block_moves_is_referenced_or_copied_as_the_modifiers_pick() {
    let (editor, block) = dropped(DropAction::Move);
    let here = editor.block_id().expect("the canvas has a block");
    assert_eq!(entities(&editor).len(), 1);
    assert_eq!(
        editor.store().block(block).and_then(|info| info.parent),
        Some(BlockParent::Block(here)),
        "a plain drop moves the block into the canvas"
    );

    let (editor, block) = dropped(DropAction::Reference);
    let [placed] = &entities(&editor)[..] else {
        panic!("a reference is placed");
    };
    assert_eq!(placed.block, Some(block));
    assert_eq!(
        editor.store().block(block).and_then(|info| info.parent),
        Some(BlockParent::Root),
        "a reference leaves the block's parent alone"
    );

    let (mut editor, block) = dropped(DropAction::Copy);
    let copies = editor
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::CopyBlocks { source, .. } => Some(Uuid::from_bytes(source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(copies, [block], "a copy asks the host to copy the block");
    assert!(
        entities(&editor).is_empty(),
        "the copy is placed once the host made it"
    );
}
