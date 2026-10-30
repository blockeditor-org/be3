use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery, ChildContent};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn the_new_menu_offers_each_workspace_then_a_new_workspace() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    my_files(&mut fixture, Vec::new());
    let (team, viewed) = (Uuid::from_u128(4), Uuid::from_u128(5));
    let mut shared = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    shared.set_name("Team");
    shared.author = Some(fixture.host.account_id());
    let mut read_only = BlockInfo::new(viewed, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    read_only.set_name("Archive");
    read_only.author = Some(Uuid::from_u128(9));
    read_only.access = AccessLevel::View;
    let own = files_workspace(&fixture.host, &[]);
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![own, shared, read_only]);
    for id in [team, viewed] {
        fixture
            .host
            .set_blocks(BlockQuery::References(id), Vec::new());
    }
    fixture.settle();

    fixture.test.click("file-tree.new");
    fixture.settle();
    assert!(fixture.offers("New in My files…"));
    assert!(fixture.offers("New in Team…"));
    assert!(
        !fixture.offers("New in Archive…"),
        "a workspace the account may only view is not offered"
    );
    assert!(fixture.offers("New workspace"));
    fixture
        .test
        .snapshot("the_new_menu_at_the_top_of_the_files");

    fixture.test.click(format!("file-tree.new-in.{MY_FILES}"));
    fixture.settle();
    let picker = fixture
        .test
        .placed_picker()
        .expect("New in My files opens a picker");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds a picker")
    };
    assert_eq!(filter.place, Some(BlockParent::Block(MY_FILES).encode()));
    let button = fixture.test.rect_of("file-tree.new");
    assert!(
        picker.rect.min.y >= button.max.y - 1.0,
        "the picker hangs from the New button: {:?} under {button:?}",
        picker.rect
    );

    fixture.new_in(team);
    let picker = fixture
        .test
        .placed_picker()
        .expect("New in another workspace opens the picker there instead");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds a picker")
    };
    assert_eq!(filter.place, Some(BlockParent::Block(team).encode()));
}
