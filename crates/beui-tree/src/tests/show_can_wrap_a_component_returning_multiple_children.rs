use super::*;

#[component]
fn Pair(label: Prop<String>, tail: Prop<bool>, disposed: Rc<Cell<usize>>) -> Children<DomNode> {
    on_cleanup(move || disposed.set(disposed.get() + 1));
    view! {
        <Element tag="p" text={label} />
        <Show condition={tail}>
            <Element tag="span" text="tail" />
        </Show>
    }
}

#[test]
fn show_can_wrap_a_component_returning_multiple_children() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let (shown, set_shown) = create_signal(false);
    let (tail, set_tail) = create_signal(false);
    let (label, set_label) = create_signal("one".to_string());
    let disposed = Rc::new(Cell::new(0));
    let counter = disposed.clone();
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Element tag="b" text="before" />
                <Show condition={shown}>
                    <Pair label tail disposed={counter.clone()} />
                </Show>
                <Element tag="b" text="after" />
            </Element>
        }
    });
    assert_eq!(page.0.children.borrow().len(), 2);
    set_shown.set(true);
    assert_eq!(page.0.children.borrow().len(), 3);
    set_tail.set(true);
    set_label.set("two".to_string());
    assert_eq!(page.0.children.borrow().len(), 4);
    let html = page.html();
    assert!(html.find("before").unwrap() < html.find("two").unwrap());
    assert!(html.find("two").unwrap() < html.find("tail").unwrap());
    assert!(html.find("tail").unwrap() < html.find("after").unwrap());
    set_shown.set(false);
    assert_eq!(disposed.get(), 1);
    assert_eq!(page.0.children.borrow().len(), 2);
    set_tail.set(false);
    set_shown.set(true);
    assert_eq!(page.0.children.borrow().len(), 3);
    drop(scope);
    assert_eq!(disposed.get(), 2);
}
