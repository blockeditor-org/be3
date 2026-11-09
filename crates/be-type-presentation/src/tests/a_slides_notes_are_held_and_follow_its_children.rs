use super::*;
use crate::{Presentation, PresentationContent};
use be_block::{ChildChange, Root};

fn notes(presentation: &PresentationContent) -> Vec<Option<Uuid>> {
    presentation
        .root()
        .slides
        .iter()
        .map(|slide| slide.notes)
        .collect()
}

fn changed(presentation: &mut PresentationContent, change: ChildChange) {
    let edit = presentation
        .root()
        .child_edit(change)
        .expect("a presentation takes children");
    presentation.apply(&edit);
}

#[test]
fn a_slides_notes_are_held_and_follow_its_children() {
    let [first, second, written] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let mut presentation = PresentationContent::default();
    let slides = [ObjectId::new(), ObjectId::new()];
    for (index, (slide, block)) in slides.into_iter().zip([first, second]).enumerate() {
        let edit = presentation.root().insert(slide, index, block);
        presentation.apply(&edit);
    }
    assert_eq!(notes(&presentation), [None, None]);

    presentation.apply(&Presentation::set_notes(slides[1], Some(written)));
    assert_eq!(notes(&presentation), [None, Some(written)]);
    assert_eq!(
        BlockContent::references(&presentation),
        [first, second, written],
        "the notes block is held"
    );

    let replacement = Uuid::new_v4();
    changed(
        &mut presentation,
        ChildChange::Replace {
            old: written,
            new: replacement,
        },
    );
    assert_eq!(notes(&presentation), [None, Some(replacement)]);

    changed(&mut presentation, ChildChange::Delete(replacement));
    assert_eq!(notes(&presentation), [None, None]);
    assert_eq!(
        presentation.root().slides.len(),
        2,
        "deleting the notes keeps the slide"
    );
    assert_eq!(BlockContent::references(&presentation), [first, second]);
}
