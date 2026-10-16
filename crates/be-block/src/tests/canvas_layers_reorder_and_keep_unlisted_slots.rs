use crate::canvas::{
    CanvasContent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasLayerMove, CanvasPoint,
    CanvasTransform, InfiniteCanvasOperation,
};
use uuid::Uuid;

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

fn run(content: &mut CanvasContent, operation: InfiniteCanvasOperation) {
    let edit = content.root().edit_for(&operation);
    content.apply(&edit);
}

fn canvas(ids: &[Uuid]) -> CanvasContent {
    let mut content = CanvasContent::default();
    for id in ids {
        run(
            &mut content,
            InfiniteCanvasOperation::Add {
                entity: entity(*id),
            },
        );
    }
    content
}

fn ids(content: &CanvasContent) -> Vec<Uuid> {
    content
        .root()
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect()
}

#[test]
fn canvas_layers_reorder_and_keep_unlisted_slots() {
    let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
    for (movement, expected) in [
        (CanvasLayerMove::ForwardOne, [b, a, d, c]),
        (CanvasLayerMove::BackOne, [a, c, b, d]),
        (CanvasLayerMove::BringToFront, [b, d, a, c]),
        (CanvasLayerMove::SendToBack, [a, c, b, d]),
    ] {
        let mut content = canvas(&[a, b, c, d]);
        run(
            &mut content,
            InfiniteCanvasOperation::Reorder {
                ids: vec![a, c],
                movement,
            },
        );
        assert_eq!(ids(&content), expected, "{movement:?}");
    }

    let mut content = canvas(&[a, d, b, c]);
    run(
        &mut content,
        InfiniteCanvasOperation::ExactOrder { ids: vec![c, a, b] },
    );
    assert_eq!(ids(&content), [c, d, a, b]);

    run(
        &mut content,
        InfiniteCanvasOperation::Add { entity: entity(a) },
    );
    run(
        &mut content,
        InfiniteCanvasOperation::Remove { ids: vec![d] },
    );
    assert_eq!(ids(&content), [c, a, b]);
}
