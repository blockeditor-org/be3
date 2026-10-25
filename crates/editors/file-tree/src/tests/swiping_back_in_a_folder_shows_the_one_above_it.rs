use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::{BackEdge, BackGesture, Input};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn swiping_back_in_a_folder_shows_the_one_above_it() {
    let mut fixture = phone();
    containers_hold_blocks(&mut fixture);
    let text_type = Uuid::from_u128(7);
    let folder = Uuid::from_u128(2);
    let notes = Uuid::from_u128(3);
    let inside = Uuid::from_u128(4);
    let files = MY_FILES;
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(files),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    projects.references = vec![inside];
    let mut listed = BlockInfo::new(notes, text_type, BlockParent::Block(files));
    listed.set_intrinsic_name("Notes".to_owned());
    my_files(&mut fixture, vec![projects, listed]);
    fixture.settle();
    fixture.test.click(format!("file-tree.{files}.row"));
    fixture.settle();
    fixture.test.click(format!("file-tree.{folder}.row"));
    fixture.settle();
    let mut roadmap = BlockInfo::new(inside, text_type, BlockParent::Block(folder));
    roadmap.set_intrinsic_name("Roadmap".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![roadmap]);
    fixture.settle();
    assert!(!fixture.test.shown(&format!("file-tree.{notes}.row")));

    fixture.test.step(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    fixture.test.step(vec![Input::Back(BackGesture::Invoked)]);
    fixture.test.settle();
    assert!(
        fixture.test.shown(&format!("file-tree.{notes}.row")),
        "swiping back shows the folder above"
    );
    assert!(fixture.test.shown("file-tree.back"));
}
