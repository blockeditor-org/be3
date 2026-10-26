use super::*;

#[test]
fn two_checklists_merge_every_item_either_side_added() {
    let (milk, add_milk) = Checklist::add("milk");
    let (eggs, add_eggs) = Checklist::add("eggs");
    let base = edited(&ChecklistContent::default(), [add_milk, add_eggs]);
    let ours = edited(
        &base,
        [Checklist::add("bread").1, Checklist::set_done(milk, true)],
    );
    let theirs = edited(
        &base,
        [
            Checklist::add("jam").1,
            Checklist::set_text(milk, "oat milk"),
            Checklist::remove(eggs),
        ],
    );

    let MergeResult::Clean(merged) = ChecklistContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different things");
    };

    let items: Vec<(String, bool)> = merged
        .root()
        .items
        .iter()
        .map(|item| (item.text.clone(), item.done))
        .collect();
    assert_eq!(
        items,
        [
            ("oat milk".to_owned(), true),
            ("bread".to_owned(), false),
            ("jam".to_owned(), false)
        ]
    );
}
