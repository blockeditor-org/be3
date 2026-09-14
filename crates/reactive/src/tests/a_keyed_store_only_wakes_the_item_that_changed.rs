use super::*;

#[test]
fn a_keyed_store_only_wakes_the_item_that_changed() {
    let scope = Scope::new();
    let store: KeyedStore<u32, String> = KeyedStore::new();
    store.reconcile_owned((0..4).map(|index| (index, format!("item {index}"))));
    let runs = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        for index in 0..4 {
            let item = store.get(&index);
            let runs = runs.clone();
            create_effect(move || {
                let text = item.get();
                runs.borrow_mut().push((index, text));
            });
        }
    });
    assert_eq!(runs.borrow().len(), 4);
    runs.borrow_mut().clear();
    store.reconcile_owned((0..4).map(|index| {
        let text = if index == 2 { "edited" } else { "item" };
        (index, format!("{text} {index}"))
    }));
    assert_eq!(*runs.borrow(), [(2, "edited 2".to_owned())]);
}
