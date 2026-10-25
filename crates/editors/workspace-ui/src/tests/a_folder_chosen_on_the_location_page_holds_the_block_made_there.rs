use block_editor_beui::{BlockCommand, BlockLocation, CreationProgress, PickedBlock};

use super::*;

#[test]
fn a_folder_chosen_on_the_location_page_holds_the_block_made_there() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let places = places(&mut fixture);
    let mut placing = filter();
    placing.place = Some(BlockLocation::Block(places.work.into_bytes()));
    let asked = Uuid::new_v4();
    fixture
        .test
        .request_pick(4, placing, BlockLocation::Block(asked.into_bytes()), None);
    fixture.settle();
    assert!(
        fixture.says("Work"),
        "the menu names the folder it makes in"
    );

    fixture.test.click("picker.location");
    fixture.settle();
    fixture.test.click(format!("picker.place.{}", places.q4));
    fixture.settle();
    assert!(
        fixture.says("Q4"),
        "choosing a folder goes back to the menu, which names it"
    );
    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_child_commits()
            .into_iter()
            .map(|(_, parent, name)| (parent, name))
            .collect::<Vec<_>>(),
        vec![(BlockLocation::Block(asked.into_bytes()), None)],
        "the block is made with the name its type gives it"
    );

    let created = Uuid::new_v4();
    report_creation(
        &mut fixture,
        CreationProgress::Created(created.into_bytes()),
    );
    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(
            4,
            Ok(Some(PickedBlock {
                block_id: created.into_bytes(),
                block_type: TEMPLATE_TYPE.into_bytes(),
                linked: false,
                placed: true,
            }))
        )]
    );
    assert!(
        fixture
            .test
            .take_block_commands()
            .iter()
            .any(|(block, command)| *block == created
                && matches!(command, BlockCommand::Place { parent, .. } if *parent == places.q4.into_bytes())),
        "the new block is placed in the folder chosen"
    );
}
