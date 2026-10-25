use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent};
use database_types::database::RowRef;
use database_types::database_schema::DatabaseRelationOptions;

use super::*;

#[test]
fn a_blocked_row_lists_its_blockers_and_they_list_it_as_blocking() {
    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Blocked by", DatabaseFieldType::Relation),
    ]);
    let (title, blocked_by) = (fixture.fields[0], fixture.fields[1]);
    let (database, schema) = (fixture.database, fixture.schema);
    fixture.harness.edit::<DatabaseSchemaContent>(
        Some(schema),
        &DatabaseSchema::set_relation_options(
            blocked_by,
            DatabaseRelationOptions {
                database: Some(database),
                multiple: true,
                inverse: "Blocking".into(),
            },
        ),
    );
    let mut info = BlockInfo::new(
        schema,
        DatabaseSchemaContent::CONTENT_TYPE,
        BlockParent::Block(database),
    );
    info.references = vec![database];
    fixture.harness.store().add_block(info);
    fixture.set(0, title, DatabaseValue::String("Fix the login".into()));
    fixture.set(
        1,
        title,
        DatabaseValue::String("Retry on slow networks".into()),
    );
    fixture.set(
        2,
        title,
        DatabaseValue::String("Show a retry banner".into()),
    );
    let rows = fixture.database();
    let id = |index: usize| rows.rows.get_index(index).unwrap().id;
    let link = |index: usize| RowRef {
        database,
        row: id(index).as_uuid(),
    };
    for blocker in [0, 1] {
        fixture.harness.edit::<DatabaseContent>(
            Some(database),
            &Database::link(id(2), blocked_by, link(blocker)),
        );
    }
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    fixture
        .harness
        .click(format!("database-view.list.row.{}", id(0).as_uuid()));
    fixture
        .harness
        .settle_until("the blocking row to show", |harness| {
            harness.shown(&format!(
                "database-view.detail.links.{}.{}.remove",
                database_types::database::inverse_id(database, blocked_by),
                id(2).as_uuid()
            ))
        });
    fixture
        .harness
        .snapshot("a_blocked_row_lists_its_blockers_and_they_list_it_as_blocking");

    fixture.harness.click(format!(
        "database-view.detail.links.{}.{}.remove",
        database_types::database::inverse_id(database, blocked_by),
        id(2).as_uuid()
    ));
    fixture.settle();
    assert_eq!(
        fixture
            .database()
            .rows
            .get(id(2))
            .unwrap()
            .links_of(blocked_by),
        [link(1)]
    );
}
