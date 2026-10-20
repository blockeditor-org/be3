use super::*;

#[test]
fn a_mock_dom_show_keep_alive_hides_the_element_it_built_and_brings_it_back() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let open = create_signal(true);
    let note = create_signal("draft".to_string());
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Show condition={open}>
                    <Element tag="p" text="rebuilt" />
                </Show>
                <ShowKeepAlive condition={open}>
                    <Element tag="textarea" text={note} />
                </ShowKeepAlive>
            </Element>
        }
    });
    assert_eq!(
        page.html(),
        "<div id=0><p id=1>rebuilt</p><textarea id=2>draft</textarea></div>"
    );

    open.set(false);
    assert_eq!(page.html(), "<div id=0></div>");

    note.set("edited while hidden".to_string());
    open.set(true);
    assert_eq!(
        page.html(),
        "<div id=0><p id=3>rebuilt</p><textarea id=2>edited while hidden</textarea></div>",
        "a `ShowKeepAlive` brings back the element it built, brought up to date as it is shown, \
         where a `Show` builds a new one"
    );
}
