use super::*;

#[test]
fn clearing_a_checklist_keeps_the_open_items() {
    let (milk, add_milk) = Checklist::add("milk");
    let (_, add_eggs) = Checklist::add("eggs");
    let checklist = edited(
        &ChecklistContent::default(),
        [add_milk, add_eggs, Checklist::set_done(milk, true)],
    );
    assert_eq!(checklist.root().done_count(), 1);

    let checklist = edited(&checklist, [checklist.root().clear_done()]);

    let texts: Vec<String> = checklist
        .root()
        .items
        .iter()
        .map(|item| item.text.clone())
        .collect();
    assert_eq!(texts, ["eggs"]);
}
