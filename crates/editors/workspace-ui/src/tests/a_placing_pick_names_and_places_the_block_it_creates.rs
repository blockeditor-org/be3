use block_editor_beui::{BlockLocation, BlockPick, CreationProgress};

use super::*;

#[test]
fn a_placing_pick_names_and_places_the_block_it_creates() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let mut placing = filter();
    placing.place = Some(BlockLocation::Root);
    fixture.test.request_pick(
        4,
        placing,
        BlockLocation::Block(Uuid::new_v4().into_bytes()),
    );
    fixture.settle();

    fixture.test.click("picker.name");
    fixture.settle();
    fixture.test.text("Minutes");
    fixture.settle();
    fixture
        .test
        .click(&format!("picker.tile.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();

    let commits = fixture.test.take_child_commits();
    assert_eq!(
        commits
            .iter()
            .map(|(_, parent, name)| (*parent, name.clone()))
            .collect::<Vec<_>>(),
        vec![(BlockLocation::Root, Some("Minutes".to_owned()))],
        "the block is created at the top level with the name typed for it"
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
            BlockPick::Chosen {
                block_id: created.into_bytes(),
                block_type: TEMPLATE_TYPE.into_bytes(),
                linked: false,
                placed: true,
            }
        )]
    );
}
