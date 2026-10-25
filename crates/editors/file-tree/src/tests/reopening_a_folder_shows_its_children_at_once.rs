use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn reopening_a_folder_shows_its_children_at_once() {
    let mut fixture = editor();
    let folder = Uuid::new_v4();
    let child = Uuid::new_v4();
    let mut listed = BlockInfo::new(folder, Uuid::nil(), BlockParent::Block(MY_FILES));
    listed.references = vec![child];
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let chevron = format!("file-tree.{folder}.chevron");
    let row = format!("file-tree.{child}.row");

    fixture.test.click(&chevron);
    fixture.settle();
    fixture.host.set_blocks(
        BlockQuery::References(folder),
        vec![BlockInfo::new(
            child,
            Uuid::nil(),
            BlockParent::Block(folder),
        )],
    );
    fixture.settle();
    assert!(fixture.test.shown(&row));

    fixture.test.click(&chevron);
    fixture.settle();
    assert!(!fixture.test.shown(&row));
    assert!(
        !fixture
            .host
            .watched_blocks()
            .contains(&BlockQuery::References(folder)),
        "a closed folder stops asking for its children"
    );

    fixture.test.click(&chevron);
    fixture.settle();
    assert!(
        fixture.test.shown(&row),
        "reopening a folder shows the children it last had without waiting for the host"
    );
}
