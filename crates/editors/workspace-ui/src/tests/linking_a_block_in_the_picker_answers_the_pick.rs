use block_editor_beui::{BlockInfo, BlockLocation, BlockPick};

use super::*;

#[test]
fn linking_a_block_in_the_picker_answers_the_pick() {
    let (mut fixture, _) = editor();
    let linked = Uuid::new_v4();
    let mut info = BlockInfo::new(linked, SHOWN_TYPE, BlockParent::Root);
    info.name = Some("Notes".to_owned());
    fixture.test.store().add_block(info);

    fixture.test.request_pick(7, filter(), BlockLocation::Root);
    fixture.settle();
    assert!(fixture.says("Add block"), "the pick opens the picker");

    fixture.click_text("Link existing");
    fixture.test.click(&format!("picker.link.{linked}"));
    fixture.settle();

    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(
            7,
            BlockPick::Chosen {
                block_id: linked.into_bytes(),
                block_type: SHOWN_TYPE.into_bytes(),
                linked: true,
                placed: false,
            }
        )]
    );
    assert!(!fixture.says("Add block"), "an answered pick closes the picker");
}
