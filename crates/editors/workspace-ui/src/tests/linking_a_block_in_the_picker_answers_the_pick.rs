use block_editor_beui::{BlockInfo, BlockLocation, PickedBlock};

use super::*;

#[test]
fn linking_a_block_in_the_picker_answers_the_pick() {
    let (fixture, _) = editor();
    let linked = Uuid::new_v4();
    let space = Uuid::new_v4();
    let mut workspace = BlockInfo::new(space, Uuid::new_v4(), BlockParent::Root);
    workspace.set_name("Team space");
    workspace.references = vec![linked];
    fixture.test.store().add_block(workspace);
    let mut info = BlockInfo::new(linked, SHOWN_TYPE, BlockParent::Block(space));
    info.set_intrinsic_name("Notes".to_owned());
    fixture.test.store().add_block(info);

    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.settle();
    assert!(
        fixture.test.shown("picker.link-existing"),
        "the pick opens the menu"
    );

    fixture.test.click("picker.link-existing");
    fixture.settle();
    fixture
        .test
        .double_click(format!("picker.link.{linked}.row"));
    fixture.settle();

    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(Some(PickedBlock {
                block_id: linked.into_bytes(),
                block_type: SHOWN_TYPE.into_bytes(),
                linked: true,
                placed: false,
            }))]
    );
    assert!(
        !fixture.test.shown("picker.confirm"),
        "an answered pick closes the sheet"
    );
}
