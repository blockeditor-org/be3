use super::*;

#[test]
fn clearing_done_items_removes_one_someone_reopened_at_once() {
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
    let reopen = Checklist::set_done(milk, false);

    for sequenced in [
        edited(&base, [reopen.clone(), clear.clone()]),
        edited(&base, [clear, reopen]),
    ] {
        assert!(sequenced.root().items.is_empty());
    }
}
