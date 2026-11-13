use super::*;
use be_block::ChildChange;
use be_type_database::database::{Database, DatabaseContent, DatabaseValue};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == blocks).then_some(())
    });
}

#[test]
fn a_deleted_database_row_is_no_longer_found() {
    let harness = Harness::start();
    harness.connect();
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the workspace's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let title = Uuid::from_u128(1);
    let row = |text: &str| Database::add_row([(title, DatabaseValue::String(text.into()))]);
    let (_, plumber) = row("Call the plumber");
    let (_, roof) = row("Patch the roof");
    let mut database = DatabaseContent::default();
    database.apply(&plumber);
    let before = database.clone();
    database.apply(&roof);
    let block = Uuid::new_v4();
    create(
        block,
        DatabaseContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block::BlockMetadata::named("Chores"),
        Some(database.encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(block)
        ),
        Some(true)
    );
    flush();
    finds("plumber", &[block]);
    finds("roof", &[block]);

    // The roof's row is gone: its document leaves the index, the plumber's stays.
    replace(block, DatabaseContent::CONTENT_TYPE, before.encode());
    flush();
    finds("roof", &[]);
    finds("plumber", &[block]);
}
