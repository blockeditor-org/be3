use super::*;

#[component]
fn RetainedPair(
    label: Prop<String>,
    tail: Prop<bool>,
    builds: Rc<Cell<usize>>,
    disposed: Rc<Cell<usize>>,
) -> Children<DomNode> {
    builds.set(builds.get() + 1);
    on_cleanup(move || disposed.set(disposed.get() + 1));
    view! {
        <Element tag="p" text={label} />
        <Show condition={tail}>
            <Element tag="span" text="tail" />
        </Show>
    }
}

#[test]
fn show_keep_alive_retains_multiple_children_and_pauses_nested_updates() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let shown = create_signal(false);
    let tail = create_signal(false);
    let label = create_signal("one".to_string());
    let builds = Rc::new(Cell::new(0));
    let disposed = Rc::new(Cell::new(0));
    let built = builds.clone();
    let cleaned = disposed.clone();
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Element tag="b" text="before" />
                <ShowKeepAlive condition={shown}>
                    <RetainedPair label tail builds={built} disposed={cleaned} />
                    <Element tag="i" text="sibling" />
                </ShowKeepAlive>
                <Element tag="b" text="after" />
            </Element>
        }
    });
    assert_eq!(builds.get(), 0);
    shown.set(true);
    assert_eq!(builds.get(), 1);
    assert_eq!(page.0.children.borrow().len(), 4);
    let first = page.html();
    let retained = page.0.children.borrow().iter().nth(1).unwrap().clone();
    shown.set(false);
    assert_eq!(disposed.get(), 0);
    assert_eq!(page.0.children.borrow().len(), 2);
    label.set("two".to_string());
    tail.set(true);
    assert_eq!(page.0.children.borrow().len(), 2);
    shown.set(true);
    assert_eq!(builds.get(), 1);
    assert_eq!(page.0.children.borrow().len(), 5);
    let updated = page.html();
    assert!(updated.contains(">two</p>"));
    assert!(Rc::ptr_eq(
        &retained.0,
        &page.0.children.borrow().iter().nth(1).unwrap().0
    ));
    assert!(updated.find("before").unwrap() < updated.find("two").unwrap());
    assert!(updated.find("two").unwrap() < updated.find("tail").unwrap());
    assert!(updated.find("tail").unwrap() < updated.find("sibling").unwrap());
    assert!(updated.find("sibling").unwrap() < updated.find("after").unwrap());
    tail.set(false);
    label.set("one".to_string());
    assert_eq!(page.html(), first);
    shown.set(false);
    drop(scope);
    assert_eq!(disposed.get(), 1);
}
