use super::*;
use block_editor_plugin::be_block::{BlockContent, CounterContent};

#[test]
fn a_direct_editor_entity_draws_the_frame_it_reserves() {
    let counter = Uuid::new_v4();
    let mut placed = entity(Uuid::from_u128(1));
    placed.kind = CanvasEntityKind::DirectEditor {
        block_id: counter,
        scale: 1.0,
    };
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let mut editor = open(
        &Canvas::with_entities([placed.clone()], None),
        false,
        &[BlockInfo::new(
            counter,
            CounterContent::CONTENT_TYPE,
            BlockParent::Detached,
        )],
    );
    editor.available_children();
    editor.run();

    assert!(
        editor.shown(&format!("infinite-canvas.entity.{}", placed.id)),
        "the direct editor has an item on the canvas"
    );
    editor.snapshot("a_direct_editor_entity_draws_the_frame_it_reserves");
}
