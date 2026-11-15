use super::*;

#[test]
fn a_show_with_a_fallback_fills_a_single_child_slot_and_swaps_it() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let editing = create_signal(false);
    let disposed = Rc::new(Cell::new(0));
    let counter = disposed.clone();
    let page = scope.run(|| {
        let reading = move || {
            let counter = counter.clone();
            on_cleanup(move || counter.set(counter.get() + 1));
            view! {
                <Element tag="p" text="reading" />
            }
        };
        view! {
            <Holder tag="div">
                <Show condition={editing} fallback={reading}>||
                    <Element tag="textarea" text="editing" />
                </Show>
            </Holder>
        }
    });
    assert_eq!(page.html(), "<div id=0><p id=1>reading</p></div>");

    editing.set(true);
    flush();
    assert_eq!(page.html(), "<div id=0><textarea id=2>editing</textarea></div>");
    assert_eq!(disposed.get(), 1, "the branch it left is dropped");

    editing.set(false);
    flush();
    assert_eq!(
        page.html(),
        "<div id=0><p id=3>reading</p></div>",
        "without `keep_alive` a branch is built again each time it shows"
    );
    drop(scope);
    assert_eq!(disposed.get(), 2);
}
