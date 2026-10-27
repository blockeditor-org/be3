use super::*;

#[test]
fn event_listeners_are_ignored_in_html_strings() {
    let clicks = Rc::new(std::cell::Cell::new(0));
    let counted = clicks.clone();
    let html = render_fragment(move || {
        view! {
            <elements::Button
                on_click={move |_: Event| counted.set(counted.get() + 1)}
                @on={("dblclick", |_: Event| {})}
            >
                "Go"
            </elements::Button>
        }
    });
    assert_eq!(html, "<button>Go</button>");
    assert_eq!(clicks.get(), 0);
}
