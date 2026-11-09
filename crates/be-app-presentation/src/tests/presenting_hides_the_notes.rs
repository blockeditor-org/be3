use super::*;
use be_sdk_beui::be_block::ObjectId as Slide;
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery};
use be_type_presentation::Presentation;

#[test]
fn presenting_hides_the_notes() {
    let (mut test, editor) = editor(1);
    let [slide] = slide_ids(&test)[..] else {
        panic!("the deck has one slide");
    };
    let notes = Uuid::new_v4();
    test.edit::<PresentationContent>(
        None,
        &Presentation::set_notes(Slide::from_uuid(slide), Some(notes)),
    );
    editor.host().set_blocks(
        BlockQuery::References(editor.block_id()),
        vec![BlockInfo::new(
            notes,
            crate::root::TEXT_TYPE,
            BlockParent::Block(editor.block_id()),
        )],
    );
    test.settle();
    assert!(test.shown("presentation.notes"));

    editor.host().set_presenting(true);
    test.settle();
    assert!(
        !test.shown("presentation.notes") && !test.shown("presentation.notes.add"),
        "the notes stay hidden while presenting"
    );
}
