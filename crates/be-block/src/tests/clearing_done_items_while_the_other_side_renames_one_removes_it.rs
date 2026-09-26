use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn clearing_done_items_while_the_other_side_renames_one_removes_it() {
    let (milk, add_milk) = Checklist::add("milk");
    let (eggs, add_eggs) = Checklist::add("eggs");
    let (jam, add_jam) = Checklist::add("jam");
    let base = edited(
        &ChecklistContent::default(),
        [
            add_milk,
            add_eggs,
            add_jam,
            Checklist::set_done(milk, true),
            Checklist::set_done(eggs, true),
        ],
    );
    let ours = edited(&base, [base.root().clear_done()]);
    let theirs = edited(&base, [Checklist::set_text(milk, "oat milk")]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);

    let items: Vec<ObjectId> = merged.root().items.iter().map(|item| item.id).collect();
    assert_eq!(items, [jam]);
}
