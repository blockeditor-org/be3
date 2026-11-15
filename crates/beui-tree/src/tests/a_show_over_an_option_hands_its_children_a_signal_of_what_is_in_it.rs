use super::*;
use crate::reactive::{ReadSignal, Show};

#[test]
fn a_show_over_an_option_hands_its_children_a_signal_of_what_is_in_it() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let picked = create_signal(None::<String>);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Show
                    value={picked}
                    fallback={|| view! {
                        <Element tag="i" text="none" />
                    }}
                >|name: ReadSignal<String>|
                    <Element tag="p" text={create_memo(move || name.get())} />
                </Show>
            </Element>
        }
    });
    let html = page.html();
    assert!(html.contains(">none</i>"), "{html}");

    picked.set(Some("one".to_owned()));
    let html = page.html();
    assert!(html.contains(">one</p>"), "{html}");
    let built = html.clone();

    picked.set(Some("two".to_owned()));
    let html = page.html();
    assert_eq!(
        html,
        built.replace(">one</p>", ">two</p>"),
        "a new value while it stays `Some` updates the children in place"
    );

    picked.set(None);
    let html = page.html();
    assert!(
        html.contains(">none</i>") && !html.contains("</p>"),
        "{html}"
    );
}
