use block_editor_beui::be_block::{BlockContent, FolderContent};
use block_editor_beui::beui::{BackEdge, BackGesture, Event};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn swiping_back_in_a_folder_shows_the_one_above_it() {
    let mut fixture = phone();
    let text_type = Uuid::from_u128(7);
    let folder = Uuid::from_u128(2);
    let notes = Uuid::from_u128(3);
    let inside = Uuid::from_u128(4);
    let mut projects = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Root);
    projects.name = Some("Projects".to_owned());
    projects.references = vec![inside];
    let mut listed = BlockInfo::new(notes, text_type, BlockParent::Root);
    listed.name = Some("Notes".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![projects, listed]);
    fixture.settle();
    fixture.test.click(&format!("file-tree.{folder}.open"));
    fixture.settle();
    let mut roadmap = BlockInfo::new(inside, text_type, BlockParent::Block(folder));
    roadmap.name = Some("Roadmap".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![roadmap]);
    fixture.settle();
    assert!(!fixture.test.shown(&format!("file-tree.{notes}.open")));

    fixture.test.step(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    fixture.test.step(Vec::new());
    assert!(
        fixture.test.shown(&format!("file-tree.{notes}.open")),
        "the folder above shows behind the one being left"
    );
    fixture
        .test
        .snapshot("swiping_back_in_a_folder_shows_the_one_above_it");

    fixture.test.step(vec![Event::Back(BackGesture::Invoked)]);
    fixture.test.settle();
    assert!(fixture.test.shown(&format!("file-tree.{notes}.open")));
    assert!(!fixture.test.shown("file-tree.back"));
}
