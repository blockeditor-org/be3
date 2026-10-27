use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

fn shown_order(fixture: &Fixture, blocks: &[Uuid]) -> Vec<Uuid> {
    let mut shown: Vec<(f32, Uuid)> = blocks
        .iter()
        .map(|block| {
            let row = fixture.test.rect_of(&format!("file-tree.{block}.row"));
            (row.min.y, *block)
        })
        .collect();
    shown.sort_by(|a, b| a.0.total_cmp(&b.0));
    shown.into_iter().map(|(_, block)| block).collect()
}

#[test]
fn a_workspace_s_rows_follow_the_order_it_lists_its_children_in() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let named = |id: u128, name: &str| {
        let mut block = BlockInfo::new(
            Uuid::from_u128(id),
            Uuid::from_u128(1),
            BlockParent::Block(MY_FILES),
        );
        block.set_name(name);
        block
    };
    let (notes, plans, todo) = (named(5, "Notes"), named(6, "Plans"), named(7, "Todo"));
    let ids = [notes.id, plans.id, todo.id];
    my_files(
        &mut fixture,
        vec![notes.clone(), plans.clone(), todo.clone()],
    );
    fixture.settle();
    assert_eq!(shown_order(&fixture, &ids), ids);

    fixture
        .host
        .reorder_block(todo.id, MY_FILES, Some(notes.id));
    let asked = fixture.host.take_block_commands();
    assert!(
        asked.iter().any(|(block, command)| *block == todo.id
            && *command
                == block_editor_beui::BlockCommand::Reorder {
                    parent: MY_FILES.into_bytes(),
                    after: Some(notes.id.into_bytes()),
                }),
        "the host is asked to put Todo after Notes: {asked:?}"
    );

    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), vec![notes, todo, plans]);
    fixture.settle();
    assert_eq!(
        shown_order(&fixture, &ids),
        [ids[0], ids[2], ids[1]],
        "the rows move once the workspace lists its children in the new order"
    );
}
