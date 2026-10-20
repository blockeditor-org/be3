use super::*;
use crate::reactive::Binding;

#[component]
fn Switch(
    #[prop(default = true)] on: Binding<bool>,
    grab: Rc<Cell<Option<Binding<bool>>>>,
) -> DomNode {
    grab.set(Some(on));
    view! {
        <Element tag="p" text={Prop::dynamic(move || on.get().to_string())} />
    }
}

#[test]
fn a_binding_prop_reads_and_writes_the_signal_it_was_given_or_its_own() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let given = create_signal(false);
    let (shared, owned) = (Rc::new(Cell::new(None)), Rc::new(Cell::new(None)));
    let (controlled, uncontrolled) = scope.run(|| {
        (
            view! { <Switch on={given} grab={shared.clone()} /> },
            view! { <Switch grab={owned.clone()} /> },
        )
    });
    assert!(controlled.html().contains(">false</p>"));
    assert!(uncontrolled.html().contains(">true</p>"));
    given.set(true);
    assert!(controlled.html().contains(">true</p>"));
    shared.get().unwrap().set(false);
    assert!(!given.get_untracked());
    assert!(controlled.html().contains(">false</p>"));
    let owned: Binding<bool> = owned.get().unwrap();
    assert!(owned.signal().is_some());
    owned.set(false);
    assert!(uncontrolled.html().contains(">false</p>"));
}
