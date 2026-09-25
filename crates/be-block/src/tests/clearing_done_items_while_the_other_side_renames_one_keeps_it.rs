use super::*;

#[test]
fn clearing_done_items_while_the_other_side_renames_one_keeps_it() {
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
    let ours = edited(&base, [base.root().clear_done()]);
    let theirs = edited(&base, [Checklist::set_text(milk, "oat milk")]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    let items: Vec<String> = merged
        .root()
        .items
        .iter()
        .map(|item| item.text.clone())
        .collect();
    assert_eq!(items, ["oat milk"]);
}
