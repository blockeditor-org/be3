use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_folder_that_could_not_be_listed_says_why() {
    let mut fixture = editor();
    let folder = Uuid::new_v4();
    let mut listed = BlockInfo::new(folder, Uuid::nil(), BlockParent::Block(MY_FILES));
    listed.references = vec![Uuid::new_v4()];
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture.test.click(format!("file-tree.{folder}.chevron"));
    fixture.settle();
    fixture.host.fail_blocks(
        BlockQuery::References(folder),
        "The server could not list this folder".to_owned(),
    );
    fixture.settle();

    assert!(
        fixture.says("The server could not list this folder"),
        "a folder whose listing failed shows the error"
    );
    assert!(
        !fixture.says("No references"),
        "a failed listing is not an empty folder"
    );
}
