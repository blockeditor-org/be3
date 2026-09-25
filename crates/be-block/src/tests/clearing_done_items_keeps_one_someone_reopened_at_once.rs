use super::*;

#[test]
#[ignore = "clear_done removes the ids it saw as done, so an item reopened in the meantime is removed anyway"]
fn clearing_done_items_keeps_one_someone_reopened_at_once() {
    let (milk, add_milk) = Checklist::add("milk");
    let (eggs, add_eggs) = Checklist::add("eggs");
    let base = edited(
        &ChecklistContent::default(),
        [
            add_milk,
            add_eggs,
            Checklist::set_done(milk, true),
            Checklist::set_done(eggs, true),
        ],
    );
    let clear = base.root().clear_done();

    let sequenced = edited(&base, [Checklist::set_done(milk, false), clear]);

    let items: Vec<(String, bool)> = sequenced
        .root()
        .items
        .iter()
        .map(|item| (item.text.clone(), item.done))
        .collect();
    assert_eq!(items, [("milk".to_owned(), false)]);
}
