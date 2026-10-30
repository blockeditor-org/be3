use super::*;
use crate::Objects;
use std::cell::RefCell;

#[test]
fn an_edit_rereads_only_the_objects_it_touched() {
    let fixture = Fixture::new();
    let items = Rc::new(RefCell::new(Objects::default()));
    let _watching = fixture
        .projection
        .on_touched(Touched::Subtree(ObjectId::ROOT), {
            let items = Rc::clone(&items);
            move |content, touched| {
                let next = Objects::reread(
                    content,
                    ObjectId::ROOT,
                    Checklist::ITEMS,
                    touched,
                    &items.borrow(),
                );
                *items.borrow_mut() = next;
            }
        });
    fixture.projection.visible(|content| {
        *items.borrow_mut() = Objects::reread(
            content,
            ObjectId::ROOT,
            Checklist::ITEMS,
            &[Touched::Everything],
            &Objects::default(),
        );
    });
    let before = items.borrow().clone();

    fixture.arrive(&[(Checklist::set_text(fixture.second, "oat milk"), false)]);
    let after = items.borrow().clone();
    let held: Vec<_> = after.iter().map(|(id, _)| *id).collect();
    assert_eq!(held, [fixture.first, fixture.second]);
    let (first, second) = (after.iter().next().unwrap(), after.iter().nth(1).unwrap());
    assert!(Rc::ptr_eq(&first.1, &before.iter().next().unwrap().1));
    assert!(!Rc::ptr_eq(&second.1, &before.iter().nth(1).unwrap().1));
    assert_eq!(second.1.text, "oat milk");

    let (bread, added) = Checklist::add("bread");
    fixture.arrive(&[(added, false)]);
    let grown = items.borrow().clone();
    assert_eq!(grown.len(), 3);
    assert_eq!(grown.iter().nth(2).unwrap().0, bread);
    assert!(Rc::ptr_eq(&grown.iter().next().unwrap().1, &first.1));
}
