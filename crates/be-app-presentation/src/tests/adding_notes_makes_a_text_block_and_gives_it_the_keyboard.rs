use super::*;
use be_sdk_beui::be_block::{ChildChange, Root};
use be_sdk_beui::{BlockParent, ChildContent};

fn notes(test: &Harness) -> Vec<Option<Uuid>> {
    test.content::<PresentationContent>(None)
        .root()
        .slides
        .iter()
        .map(|slide| slide.notes)
        .collect()
}

#[test]
fn adding_notes_makes_a_text_block_and_gives_it_the_keyboard() {
    let (mut test, editor) = editor(2);
    test.run();
    assert!(test.shown("presentation.notes.add"));
    assert_eq!(notes(&test), [None, None]);

    let texts = |test: &Harness| -> Vec<Uuid> {
        test.store()
            .created()
            .into_iter()
            .filter(|id| {
                test.store()
                    .block(*id)
                    .is_some_and(|info| info.block_type == crate::root::TEXT_TYPE)
            })
            .collect()
    };
    assert_eq!(
        texts(&test),
        Vec::<Uuid>::new(),
        "no notes block is made for untouched slides"
    );

    test.click("presentation.notes.add");
    test.settle();
    let [made]: [Uuid; 1] = texts(&test)
        .try_into()
        .unwrap_or_else(|made: Vec<_>| panic!("one notes block is made, made {made:?}"));
    let info = test.store().block(made).expect("the notes block is made");
    assert_eq!(
        info.block_type,
        crate::root::TEXT_TYPE,
        "notes start as text"
    );
    assert_eq!(info.parent, Some(BlockParent::Block(editor.block_id())));
    assert_eq!(
        notes(&test),
        [Some(made), None],
        "only the slide whose notes were added has a notes block"
    );

    let placed = test
        .children()
        .iter()
        .find_map(|placement| match placement.content {
            ChildContent::Block { block_id, .. } if Uuid::from_bytes(block_id) == made => {
                Some(placement.child)
            }
            _ => None,
        })
        .expect("the notes editor is placed");
    test.available_children();
    test.settle();
    assert_eq!(
        test.take_child_focus().last(),
        Some(&placed),
        "the keyboard goes into the notes editor"
    );
    assert!(!test.shown("presentation.notes.add"));

    let deleted = test
        .content::<PresentationContent>(None)
        .root()
        .child_edit(ChildChange::Delete(made))
        .expect("a deck follows its children");
    test.edit::<PresentationContent>(None, &deleted);
    test.settle();
    assert_eq!(
        notes(&test),
        [None, None],
        "deleting the notes block clears the link"
    );
    assert_eq!(slide_ids(&test).len(), 2, "and keeps the slide");
    assert!(test.shown("presentation.notes.add"));
}
