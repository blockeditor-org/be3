use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn the_files_are_shown_by_workspace_then_what_is_shared() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    let notes = Uuid::from_u128(3);
    let team = Uuid::from_u128(4);
    let plan = Uuid::from_u128(5);
    let elsewhere = Uuid::from_u128(6);
    let mut written = BlockInfo::new(notes, text_type, BlockParent::Block(MY_FILES));
    written.set_intrinsic_name("Notes");
    let workspace = files_workspace(&fixture.host, std::slice::from_ref(&written));
    let files = workspace.id;
    let mut shared_space = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    shared_space.set_name("Team");
    shared_space.author = Some(Uuid::from_u128(9));
    shared_space.access = AccessLevel::View;
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![shared_space, workspace]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(files), vec![written]);
    fixture
        .host
        .set_blocks(BlockQuery::References(team), Vec::new());
    let mut given = BlockInfo::new(plan, text_type, BlockParent::Block(elsewhere));
    given.set_intrinsic_name("Plan");
    given.author = Some(Uuid::from_u128(9));
    given.access = AccessLevel::View;
    fixture.host.set_blocks(BlockQuery::Shared, vec![given]);
    fixture.settle();

    let top =
        |fixture: &Fixture, id: &str| fixture.test.rect_of(&format!("file-tree.{id}.row")).top();
    let order = [
        top(&fixture, &files.to_string()),
        top(&fixture, &notes.to_string()),
        top(&fixture, &team.to_string()),
        top(&fixture, "shared"),
        top(&fixture, &plan.to_string()),
        top(&fixture, "orphans"),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "My files comes first, then the other workspaces, then what is shared, then what was deleted: {order:?}"
    );
    assert!(
        fixture.says("Nothing here yet"),
        "an empty workspace says so"
    );
    fixture
        .test
        .snapshot("the_files_are_shown_by_workspace_then_what_is_shared");

    fixture.test.click(format!("file-tree.{files}.chevron"));
    fixture.settle();
    assert!(
        !fixture.test.shown(&format!("file-tree.{notes}.row")),
        "a workspace folds away what is in it"
    );
}
