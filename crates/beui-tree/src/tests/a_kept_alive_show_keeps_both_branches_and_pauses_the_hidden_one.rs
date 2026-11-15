use super::*;

#[test]
fn a_kept_alive_show_keeps_both_branches_and_pauses_the_hidden_one() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let editing = create_signal(false);
    let note = create_signal(0);
    let runs = store(Cell::new(0));
    let page = scope.run(|| {
        let text = create_memo(move || {
            runs.with(|runs| runs.set(runs.get() + 1));
            note.get().to_string()
        });
        view! {
            <Holder tag="div">
                <Show
                    keep_alive=true
                    condition={editing}
                    fallback={move || view! {
                        <Element tag="p" text={text} />
                    }}
                >||
                    <Element tag="textarea" text="editing" />
                </Show>
            </Holder>
        }
    });
    assert_eq!(
        page.html(),
        "<div id=2><host id=0><p id=1>0</p></host></div>"
    );

    editing.set(true);
    flush();
    assert_eq!(
        page.html(),
        "<div id=2><host id=0><textarea id=3>editing</textarea></host></div>"
    );
    for next in 1..=5 {
        note.set(next);
    }
    assert_eq!(
        runs.with(Cell::get),
        1,
        "the hidden fallback runs none of its updates"
    );

    editing.set(false);
    flush();
    assert_eq!(
        page.html(),
        "<div id=2><host id=0><p id=1>5</p></host></div>",
        "the fallback comes back as it was built, caught up once"
    );
    assert_eq!(runs.with(Cell::get), 2);

    editing.set(true);
    flush();
    assert_eq!(
        page.html(),
        "<div id=2><host id=0><textarea id=3>editing</textarea></host></div>"
    );
}
