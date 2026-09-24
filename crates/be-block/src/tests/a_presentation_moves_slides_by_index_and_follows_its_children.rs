use super::*;
use crate::presentation::PresentationContent;
use crate::{ChildChange, ObjectId, Root};
use uuid::Uuid;

fn shown(presentation: &PresentationContent) -> Vec<Option<Uuid>> {
    presentation
        .root()
        .slides
        .iter()
        .map(|slide| slide.block)
        .collect()
}

#[test]
fn a_presentation_moves_slides_by_index_and_follows_its_children() {
    let [first, second, third] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let mut presentation = PresentationContent::default();
    for block in [first, second] {
        let edit = presentation
            .root()
            .child_edit(ChildChange::Add(block))
            .expect("a presentation takes children");
        presentation.apply(&edit);
    }
    let slide = ObjectId::new();
    let edit = presentation.root().insert(slide, 0, third);
    presentation.apply(&edit);
    assert_eq!(
        shown(&presentation),
        [third, first, second].map(|block| Some(block))
    );

    let edit = presentation.root().move_to(slide, 2);
    presentation.apply(&edit);
    assert_eq!(
        shown(&presentation),
        [first, second, third].map(|block| Some(block))
    );

    let replacement = Uuid::new_v4();
    for change in [
        ChildChange::Delete(first),
        ChildChange::Replace {
            old: third,
            new: replacement,
        },
    ] {
        let edit = presentation
            .root()
            .child_edit(change)
            .expect("a presentation takes children");
        presentation.apply(&edit);
    }
    assert_eq!(
        shown(&presentation),
        [second, replacement].map(|block| Some(block))
    );
    assert_eq!(
        BlockContent::references(&presentation),
        [second, replacement]
    );
}
