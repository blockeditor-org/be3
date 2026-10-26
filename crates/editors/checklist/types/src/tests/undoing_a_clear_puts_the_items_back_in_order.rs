use super::*;

#[test]
fn undoing_a_clear_puts_the_items_back_in_order() {
    let (milk, add_milk) = Checklist::add("milk");
    let (eggs, add_eggs) = Checklist::add("eggs");
    let (jam, add_jam) = Checklist::add("jam");
    let mut content = edited(
        &ChecklistContent::default(),
        [
            add_milk,
            add_eggs,
            add_jam,
            Checklist::set_done(milk, true),
            Checklist::set_done(jam, true),
        ],
    );
    let before = content.clone();

    let clear = content.root().clear_done();
    let step = undone(&mut content, clear);
    assert_eq!(content.root().items.len(), 1);
    assert_eq!(content.root().items[0].id, eggs);

    reverted(&mut content, &step);
    assert_eq!(content, before);
}
