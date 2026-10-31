use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn what_a_node_removed_during_layout_changes_is_laid_out_in_the_same_frame() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let focused = create_signal(false);
    let grown = create_signal(10.0_f32);
    let gone = NodeRef::new();
    let inner = NodeRef::new();
    let trigger = NodeRef::new();
    let mut document = build_node(move || {
        let tall = create_memo(move || if focused.get() { 80.0 } else { 40.0 });
        view! {
            <List spacing=0.0>
                <Frame width=200.0 height=100.0>
                    <List spacing=0.0>
                        <Frame @node_ref=&inner height={tall} />
                    </List>
                </Frame>
                <Frame width=200.0 height=40.0>
                    <List spacing=0.0>
                        <Show condition={shown}>
                            <Interactive @node_ref=&gone focusable=true>
                                <Frame height=20.0 />
                            </Interactive>
                        </Show>
                    </List>
                </Frame>
                <Frame @node_ref=&trigger height={grown} />
            </List>
        }
    });
    let (trigger_layouts, _) = counted(&mut document, trigger.get());
    let hiding = Rc::new(std::cell::Cell::new(false));
    let armed = Rc::clone(&hiding);
    crate::native::with_reactive_scope(&mut document, move || {
        create_binding(move || {
            trigger_layouts.get();
            if armed.replace(false) {
                shown.set(false);
            }
        });
    });
    let watched = document.watch_focused();
    create_binding(move || focused.set(watched.get().is_some()));
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let node = gone.get();
    harness.document_mut().focus_focusable(node);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(inner.get()).height(), 80.0);

    hiding.set(true);
    grown.set(20.0);
    harness.frame(Vec::new());

    assert!(
        !harness.document().contains(node),
        "laying out the last frame took the focused node away"
    );
    assert_eq!(
        harness.rect(inner.get()).height(),
        40.0,
        "losing the focus with the removed node is laid out before the frame is painted"
    );
}
