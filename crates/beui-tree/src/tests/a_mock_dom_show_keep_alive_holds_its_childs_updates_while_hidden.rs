use super::*;

#[test]
fn a_mock_dom_show_keep_alive_holds_its_childs_updates_while_hidden() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let open = create_signal(true);
    let note = create_signal(0);
    let runs = store(Cell::new(0));
    let page = scope.run(|| {
        let text = create_memo(move || {
            runs.with(|runs| runs.set(runs.get() + 1));
            note.get().to_string()
        });
        view! {
            <Element tag="div">
                <ShowKeepAlive condition={open}>
                    <Element tag="p" text={text} />
                </ShowKeepAlive>
            </Element>
        }
    });
    assert_eq!(runs.with(Cell::get), 1);

    open.set(false);
    for next in 1..=5 {
        note.set(next);
    }
    assert_eq!(
        runs.with(Cell::get),
        1,
        "a hidden `ShowKeepAlive` runs none of its child's updates"
    );

    open.set(true);
    assert_eq!(runs.with(Cell::get), 2, "showing it again catches up once");
    assert_eq!(page.html(), "<div id=0><p id=1>5</p></div>");
}
