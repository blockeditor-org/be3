use super::*;

#[test]
fn a_mock_dom_show_keep_alive_holds_its_childs_updates_while_hidden() {
    fresh_ids();
    let scope = Scope::new();
    let (open, set_open) = create_signal(true);
    let (note, set_note) = create_signal(0);
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let page = scope.run(|| {
        let text = create_memo(move || {
            counted.set(counted.get() + 1);
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
    assert_eq!(runs.get(), 1);

    set_open.set(false);
    for next in 1..=5 {
        set_note.set(next);
    }
    assert_eq!(
        runs.get(),
        1,
        "a hidden `ShowKeepAlive` runs none of its child's updates"
    );

    set_open.set(true);
    assert_eq!(runs.get(), 2, "showing it again catches up once");
    assert_eq!(page.html(), "<div id=0><p id=1>5</p></div>");
}
