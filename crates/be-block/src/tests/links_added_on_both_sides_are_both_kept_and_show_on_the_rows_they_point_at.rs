use super::*;
use crate::database::{RowRef, inverses};
use crate::database_schema::DatabaseRelationOptions;

#[test]
fn links_added_on_both_sides_are_both_kept_and_show_on_the_rows_they_point_at() {
    let database = Uuid::new_v4();
    let mut schema = DatabaseSchemaContent::default();
    let (title, add_title) = DatabaseSchema::add_field("Title", DatabaseFieldType::String);
    let (blocked_by, add_blocked_by) =
        DatabaseSchema::add_field("Blocked by", DatabaseFieldType::Relation);
    for edit in [
        add_title,
        add_blocked_by,
        DatabaseSchema::set_relation_options(
            blocked_by,
            DatabaseRelationOptions {
                database: Some(database),
                multiple: true,
                inverse: "Blocking".into(),
            },
        ),
    ] {
        schema.apply(&edit);
    }
    let named = |name: &str| Database::add_row([(title, DatabaseValue::String(name.into()))]);
    let (login, add_login) = named("Fix the login");
    let (retry, add_retry) = named("Retry");
    let (banner, add_banner) = named("Banner");
    let base = edited(
        &DatabaseContent::default(),
        [add_login, add_retry, add_banner],
    );
    let link = |row: ObjectId| RowRef {
        database,
        row: row.as_uuid(),
    };

    let ours = edited(&base, [Database::link(banner, blocked_by, link(login))]);
    let theirs = edited(&base, [Database::link(banner, blocked_by, link(retry))]);
    let (merged, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    let root = merged.root();
    let mut links = root.rows.get(banner).unwrap().links_of(blocked_by);
    links.sort();
    let mut expected = vec![link(login), link(retry)];
    expected.sort();
    assert_eq!(links, expected);
    assert_eq!(
        root.rows.get(banner).unwrap().get(blocked_by),
        Some(DatabaseValue::Links(links.clone()))
    );

    let found = inverses(database, database, &root, &schema.root());
    assert_eq!(found.len(), 1);
    let blocking = &found[0];
    assert_eq!(blocking.name, "Blocking");
    assert_eq!(
        blocking.rows.get(&login.as_uuid()),
        Some(&vec![link(banner)])
    );
    let shown = blocking.shown_on(root.rows.get(login).unwrap(), login.as_uuid());
    assert_eq!(
        shown.get(blocking.id),
        Some(DatabaseValue::Links(vec![link(banner)]))
    );

    let unblocked = edited(&merged, [blocking.edits(&root, link(login), &[])]);
    assert_eq!(
        unblocked
            .root()
            .rows
            .get(banner)
            .unwrap()
            .links_of(blocked_by),
        [link(retry)]
    );
    let set = edited(
        &merged,
        [root.set_links(banner, blocked_by, &[link(retry)])],
    );
    assert_eq!(
        set.root().rows.get(banner).unwrap().links_of(blocked_by),
        [link(retry)]
    );
}
