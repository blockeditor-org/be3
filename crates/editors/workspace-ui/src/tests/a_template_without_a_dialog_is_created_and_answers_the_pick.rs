use block_editor_beui::{BlockLocation, BlockPick, ChildContent, CreationProgress};

use super::*;

#[test]
fn a_template_without_a_dialog_is_created_and_answers_the_pick() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let parent = Uuid::new_v4();
    fixture
        .test
        .request_pick(3, filter(), BlockLocation::Block(parent.into_bytes()));
    fixture.settle();

    fixture
        .test
        .click(&format!("picker.tile.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();

    let child = fixture
        .test
        .children()
        .iter()
        .find(|placement| {
            placement.content
                == ChildContent::Creation {
                    editor: TEMPLATE_EDITOR.into_bytes(),
                    template: "main".to_owned(),
                }
        })
        .map(|placement| placement.child)
        .expect("the template is placed for the host to create");
    assert_eq!(
        fixture.test.take_child_commits(),
        vec![(child, BlockLocation::Block(parent.into_bytes()), None)],
        "a template without a dialog is committed at once, under the block that asked"
    );

    let created = Uuid::new_v4();
    report_creation(
        &mut fixture,
        CreationProgress::Created(created.into_bytes()),
    );

    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(
            3,
            BlockPick::Chosen {
                block_id: created.into_bytes(),
                block_type: TEMPLATE_TYPE.into_bytes(),
                linked: false,
                placed: false,
            }
        )]
    );
}
