use super::*;
use block_client::blocks::counter::Counter;

#[test]
fn a_direct_editor_entity_draws_the_frame_it_reserves() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let counter = client.create_block(Counter::default());
    let mut placed = entity(Uuid::from_u128(1));
    placed.kind = CanvasEntityKind::DirectEditor {
        block_id: counter.id(),
        scale: 1.0,
    };
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let mut editor = open(
        client,
        &Canvas::with_entities([placed.clone()], None),
        false,
    );
    editor.available_children();
    editor.run();

    assert!(
        editor.shown(&format!("infinite-canvas.entity.{}", placed.id)),
        "the direct editor has an item on the canvas"
    );
    editor.snapshot("a_direct_editor_entity_draws_the_frame_it_reserves");
}
