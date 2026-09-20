use super::*;
use block_client::blocks::counter::Counter;

#[test]
fn a_direct_editor_entity_draws_the_frame_it_reserves() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let counter = client.create_block(Counter::default());
    let block = client.create_block(InfiniteCanvas::new());
    let mut placed = entity(Uuid::from_u128(1));
    placed.kind = CanvasEntityKind::DirectEditor {
        block_id: BlockRef::Direct(counter.id()),
        scale: 1.0,
    };
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    block.operate(InfiniteCanvasOperation::Add {
        entity: placed.clone(),
    });
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::<CanvasApp>::new(editor).in_viewport();
    editor.run();
    editor.available_children();
    editor.run();

    assert!(
        editor.shown(&format!("infinite-canvas.entity.{}", placed.id)),
        "the direct editor has an item on the canvas"
    );
    editor.snapshot("a_direct_editor_entity_draws_the_frame_it_reserves");
}
