use super::*;

#[test]
fn modifiers_run_after_props_and_inside_render_slots() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let order = Rc::new(RefCell::new(Vec::new()));
    let record = |event| {
        order.borrow_mut().push(event);
        event
    };
    let page = view! {
        <OrdinaryProps
            @modifiers::suffix={record("first")}
            test_id={record("prop").to_string()}
            @modifiers::suffix={record("second")}
            node_ref="ordinary"
        />
    };
    assert_eq!(*order.borrow(), ["prop", "first", "second"]);
    assert_eq!(page.html(), "<p id=0>prop:ordinaryfirstsecond</p>");
    let page = view! {
        <Element tag="div">
            <Show condition=true>
                <Element tag="p" text="shown" @modifiers::wrap="strong" />
            </Show>
        </Element>
    };
    assert_eq!(
        page.html(),
        "<div id=1><strong id=3><p id=2>shown</p></strong></div>"
    );
}
