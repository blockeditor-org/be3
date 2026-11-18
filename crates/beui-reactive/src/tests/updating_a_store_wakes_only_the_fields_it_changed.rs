use super::*;

#[derive(Clone, PartialEq, Store)]
struct Pair {
    left: u32,
    right: u32,
}

#[test]
fn updating_a_store_wakes_only_the_fields_it_changed() {
    let _root = RootScope::new();
    let pair = PairStore::new(Pair { left: 1, right: 2 });
    let runs = Rc::new(RefCell::new(Vec::new()));
    let recorded = runs.clone();
    create_effect(move || {
        recorded
            .borrow_mut()
            .push(format!("left {}", pair.left.get()))
    });
    let recorded = runs.clone();
    create_effect(move || {
        recorded
            .borrow_mut()
            .push(format!("right {}", pair.right.get()))
    });
    flush();
    runs.borrow_mut().clear();
    pair.update(|pair| pair.right = 5);
    flush();
    assert_eq!(*runs.borrow(), ["right 5".to_owned()]);
}
