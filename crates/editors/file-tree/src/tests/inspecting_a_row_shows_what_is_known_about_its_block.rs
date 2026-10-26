use block_editor_beui::{BlockInfo, BlockParent};

use super::*;

#[test]
fn inspecting_a_row_shows_what_is_known_about_its_block() {
    let mut fixture = editor();
    let block_type = Uuid::from_u128(1);
    let block = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(block, block_type, BlockParent::Block(MY_FILES));
    listed.set_name("Notes".to_owned());
    listed.references = vec![Uuid::from_u128(3), Uuid::from_u128(4)];
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.inspect.id"),
        "nothing is inspected until asked"
    );

    fixture.choose(block, "Inspect");

    fixture
        .test
        .snapshot("inspecting_a_row_shows_what_is_known_about_its_block");
    assert_eq!(
        fixture.test.label("file-tree.inspect.id"),
        block.to_string()
    );
    assert_eq!(fixture.test.label("file-tree.inspect.name"), "Notes");
    assert_eq!(
        fixture.test.label("file-tree.inspect.parent"),
        format!("My files ({MY_FILES})")
    );
    assert_eq!(fixture.test.label("file-tree.inspect.references"), "2");
    assert!(
        fixture.opened().is_empty(),
        "inspecting a block must not open it"
    );

    fixture.test.click("file-tree.inspect.close");
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.inspect.id"),
        "closing the inspector hides it"
    );
}
