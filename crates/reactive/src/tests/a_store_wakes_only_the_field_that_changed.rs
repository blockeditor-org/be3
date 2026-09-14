use super::*;

#[derive(Clone, PartialEq, Store)]
struct Settings {
    width: u32,
    label: String,
}

#[test]
fn a_store_wakes_only_the_field_that_changed() {
    let scope = Scope::new();
    let settings = SettingsStore::new(Settings {
        width: 4,
        label: "left".to_owned(),
    });
    let runs = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let width = settings.width.clone();
        let recorded = runs.clone();
        create_effect(move || recorded.borrow_mut().push(format!("width {}", width.get())));
        let label = settings.label.clone();
        let recorded = runs.clone();
        create_effect(move || recorded.borrow_mut().push(format!("label {}", label.get())));
    });
    runs.borrow_mut().clear();
    settings.set(Settings {
        width: 4,
        label: "right".to_owned(),
    });
    assert_eq!(*runs.borrow(), ["label right".to_owned()]);
    runs.borrow_mut().clear();
    settings.set_width(9);
    assert_eq!(*runs.borrow(), ["width 9".to_owned()]);
    assert_eq!(settings.get().label, "right");
}
