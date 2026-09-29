use block_editor_beui::be_block::WorkspaceUiContent;

use super::*;

#[test]
fn a_shown_block_is_remembered_in_the_recents() {
    let (mut fixture, opened) = editor();
    fixture.test.hold(None, WorkspaceUiContent::default());
    fixture.settle();
    let second = Uuid::new_v4();

    show(&mut fixture, opened, None);
    show(&mut fixture, second, Some(opened));

    let recents = fixture
        .test
        .content::<WorkspaceUiContent>(None)
        .root()
        .recent_blocks();
    assert_eq!(
        recents,
        vec![
            (second, FileTreeContent::CONTENT_TYPE),
            (opened, FileTreeContent::CONTENT_TYPE),
        ],
        "the block shown last comes first"
    );
}
