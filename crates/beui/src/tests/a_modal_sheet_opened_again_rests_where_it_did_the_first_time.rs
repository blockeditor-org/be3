use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_modal_sheet_opened_again_rests_where_it_did_the_first_time() {
    let _root = crate::reactive::RootScope::new();
    let content = NodeRef::new();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <ModalSheet open={open} on_close={move || open.set(false)}>
                    <Frame @node_ref=&content height=120.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let top = |harness: &Harness| {
        harness
            .document()
            .node_rect(content.get())
            .map(|rect| rect.top())
    };
    open.set(true);
    harness.settle();
    let first = top(&harness).expect("the opened sheet is laid out");
    assert!(first < 600.0, "it rests on screen, at {first}");

    open.set(false);
    harness.settle();
    assert_eq!(top(&harness), None, "it leaves once closed");

    open.set(true);
    harness.settle();
    assert_eq!(
        top(&harness),
        Some(first),
        "opened again, it rests where it did the first time"
    );
}
