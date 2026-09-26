use super::*;
use crate::canvas::{CanvasContent, CanvasPoint, InfiniteCanvasOperation};

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_canvas_entity_removed_on_one_side_and_moved_on_the_other_stays_removed() {
    let original = rectangle();
    let base = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: original.clone(),
        },
    );
    let ours = canvas_run(
        &base,
        InfiniteCanvasOperation::Remove {
            ids: vec![original.id],
        },
    );
    let mut moved = original;
    moved.transform.center = CanvasPoint::new(40.0, 40.0);
    let theirs = canvas_run(
        &base,
        InfiniteCanvasOperation::Update {
            entities: vec![moved],
        },
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);

    assert!(merged.root().entities().is_empty());
}
