use super::*;
use crate::{CanvasContent, CanvasLayerMove, InfiniteCanvasOperation};

#[test]
fn entities_brought_forward_on_both_sides_merge_to_both_moves() {
    let entities = [rectangle(), rectangle(), rectangle(), rectangle()];
    let mut base = CanvasContent::default();
    for entity in &entities {
        base = canvas_run(
            &base,
            InfiniteCanvasOperation::Add {
                entity: Box::new(entity.clone()),
                pixels: None,
            },
        );
    }
    let ours = canvas_run(
        &base,
        InfiniteCanvasOperation::Reorder {
            ids: vec![entities[0].id],
            movement: CanvasLayerMove::BringToFront,
        },
    );
    let theirs = canvas_run(
        &base,
        InfiniteCanvasOperation::Reorder {
            ids: vec![entities[3].id],
            movement: CanvasLayerMove::SendToBack,
        },
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);

    let order: Vec<Uuid> = merged
        .root()
        .entities()
        .iter()
        .map(|entity| entity.id)
        .collect();
    assert_eq!(
        order,
        [
            entities[3].id,
            entities[1].id,
            entities[2].id,
            entities[0].id
        ]
    );
}
