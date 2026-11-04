use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_node_on_laid_out_removes_is_gone_before_the_next_layout() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let focused = create_signal(false);
    let gone = NodeRef::new();
    let inner = NodeRef::new();
    let document = build_node(move || {
        let tall = create_memo(move || if focused.get() { 80.0 } else { 40.0 });
        view! {
            <List spacing=0.0>
                <Frame width=200.0 height=100.0>
                    <List spacing=0.0>
                        <Frame @node_ref=&inner height={tall} />
                    </List>
                </Frame>
                <Show condition={shown}>
                    <Interactive @node_ref=&gone focusable=true>
                        <Frame height=20.0 />
                    </Interactive>
                </Show>
            </List>
        }
    });
    let watched = document.watch_focused();
    create_binding(move || focused.set(watched.get().is_some()));
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let node = gone.get();
    harness.document_mut().focus_focusable(node);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(inner.get()).height(), 80.0);

    let hiding = Rc::new(std::cell::Cell::new(true));
    harness.document_mut().on_laid_out(move || {
        if hiding.replace(false) {
            shown.set(false);
        }
    });
    harness.frame(Vec::new());

    assert!(!harness.document().contains(node));
    assert_eq!(
        harness.rect(inner.get()).height(),
        40.0,
        "losing the focus with the removed node is laid out before the frame is painted"
    );
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "the node is removed as soon as on_laid_out returns, so the one layout it asks for settles the frame"
    );
}
