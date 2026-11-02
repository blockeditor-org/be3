use super::*;

#[test]
fn checks_of_different_items_at_once_both_stick() {
    let (milk, add_milk) = Checklist::add("milk");
    let (eggs, add_eggs) = Checklist::add("eggs");
    let base = edited(&ChecklistContent::default(), [add_milk, add_eggs]);
    let ours = Checklist::set_done(milk, true);
    let theirs = Checklist::set_done(eggs, true);

    let ours_first = edited(&base, [ours.clone(), theirs.clone()]);
    let theirs_first = edited(&base, [theirs, ours]);

    assert_eq!(ours_first, theirs_first);
    assert_eq!(ours_first.root().done_count(), 2);
}
