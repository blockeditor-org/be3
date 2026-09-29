use super::*;
use crate::workspace_ui::{MAX_RECENT, WorkspaceUi, WorkspaceUiContent};

fn visited(content: &WorkspaceUiContent, block: Uuid) -> WorkspaceUiContent {
    match content.root().visit(block, Uuid::nil()) {
        Some(edit) => edited(content, [edit]),
        None => content.clone(),
    }
}

fn blocks(content: &WorkspaceUiContent) -> Vec<Uuid> {
    content
        .root()
        .recent_blocks()
        .into_iter()
        .map(|(block, _)| block)
        .collect()
}

#[test]
fn visiting_a_block_puts_it_first_in_the_recents_once() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let content = visited(&WorkspaceUiContent::default(), first);
    let content = visited(&content, second);
    assert_eq!(blocks(&content), [second, first]);

    assert!(
        content.root().visit(second, Uuid::nil()).is_none(),
        "visiting the first block again changes nothing"
    );

    let content = visited(&content, first);
    assert_eq!(blocks(&content), [first, second], "a block is listed once");

    let mut content = content;
    for index in 0..MAX_RECENT as u128 + 5 {
        content = visited(&content, Uuid::from_u128(100 + index));
    }
    assert_eq!(blocks(&content).len(), MAX_RECENT, "the list keeps its cap");

    let newest = blocks(&content)[0];
    let content = edited(&content, [content.root().forget(newest)]);
    assert!(!blocks(&content).contains(&newest));
    assert!(WorkspaceUi::default().recent_blocks().is_empty());
}
