use super::*;
use crate::database::RowRef;

#[test]
fn a_database_follows_the_pages_and_databases_its_rows_link_to() {
    let schema = Uuid::new_v4();
    let page = Uuid::new_v4();
    let projects = Uuid::new_v4();
    let field = Uuid::new_v4();
    let link = RowRef {
        database: projects,
        row: Uuid::new_v4(),
    };
    let (row, add) = Database::add_row([(field, DatabaseValue::Relation(link))]);
    let content = edited(
        &DatabaseContent::new(&Database::with_schema(schema)),
        [add, Database::set_page(row, Some(page))],
    );
    assert_eq!(content.root().rows.get(row).unwrap().page(), Some(page));
    let mut references = BlockContent::references(&content);
    references.sort();
    let mut expected = vec![schema, page, projects];
    expected.sort();
    assert_eq!(references, expected);

    let copied_projects = Uuid::new_v4();
    let copied_page = Uuid::new_v4();
    let repointed = edited(
        &content,
        [
            content
                .root()
                .child_edit(ChildChange::Replace {
                    old: projects,
                    new: copied_projects,
                })
                .unwrap(),
            content
                .root()
                .child_edit(ChildChange::Replace {
                    old: page,
                    new: copied_page,
                })
                .unwrap(),
        ],
    );
    let moved = repointed.root();
    let moved = moved.rows.get(row).unwrap();
    assert_eq!(
        moved.value(field),
        Some(&DatabaseValue::Relation(RowRef {
            database: copied_projects,
            row: link.row,
        }))
    );
    assert_eq!(moved.page(), Some(copied_page));

    let cleared = edited(
        &content,
        [
            content
                .root()
                .child_edit(ChildChange::Delete(projects))
                .unwrap(),
            content
                .root()
                .child_edit(ChildChange::Delete(page))
                .unwrap(),
        ],
    );
    let row = cleared.root();
    let row = row.rows.get(row_id(&content)).unwrap();
    assert_eq!(row.value(field), None);
    assert_eq!(row.page(), None);
}

fn row_id(content: &DatabaseContent) -> ObjectId {
    content.root().rows.iter().next().unwrap().id
}
