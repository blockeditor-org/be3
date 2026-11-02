use super::*;
use crate::database_view::FilterCondition;

#[test]
fn a_view_shows_only_the_rows_every_filter_matches() {
    let (title, status) = (Uuid::new_v4(), Uuid::new_v4());
    let (open, done) = (Uuid::new_v4(), Uuid::new_v4());
    let rows = [
        ("Fix the login", Some(open)),
        ("Write the guide", Some(done)),
        ("Fix the docs", Some(done)),
        ("Fix the logout", None),
    ];
    let database = edited(
        &DatabaseContent::default(),
        rows.iter().map(|(name, state)| {
            let mut values = vec![(title, DatabaseValue::String((*name).into()))];
            values.extend(state.map(|state| (status, DatabaseValue::Enum(state))));
            Database::add_row(values).1
        }),
    );
    let (status_filter, not_done) = DatabaseView::add_filter(
        Some(status),
        FilterCondition::IsNot(DatabaseValue::Enum(done)),
    );
    let (contains, fixes) =
        DatabaseView::add_filter(Some(title), FilterCondition::Contains("FIX".into()));
    let (_, unfinished) = DatabaseView::add_filter(None, FilterCondition::IsEmpty);
    let view = edited(
        &DatabaseViewContent::default(),
        [not_done, fixes, unfinished],
    );

    let shown = |view: &DatabaseViewContent| {
        let view = view.root();
        let database = database.root();
        database
            .rows
            .iter()
            .filter(|row| view.shows(row, &[]))
            .map(|row| database.title(row.id, Some(title)))
            .collect::<Vec<_>>()
    };
    assert_eq!(shown(&view), ["Fix the login", "Fix the logout"]);

    let set = edited(
        &view,
        [DatabaseView::set_filter_condition(
            contains,
            &FilterCondition::IsEmpty,
        )],
    );
    assert!(shown(&set).is_empty());
    let removed = edited(&view, [DatabaseView::remove_filter(status_filter)]);
    assert_eq!(
        shown(&removed),
        ["Fix the login", "Fix the docs", "Fix the logout"]
    );
}
