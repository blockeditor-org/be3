use super::*;
use crate::base::overlay::{OverlayAnchor, OverlayMode, OverlayNode, Placement};
use crate::reactive::{
    Frame, NodeRef, Overlay, WriteSignal, build, create_signal, view, with_reactive_scope,
};

#[test]
fn a_nested_overlay_closes_with_its_parent_whatever_the_parents_mode() {
    let (open, set_open) = create_signal(true);
    let (mode, set_mode) = create_signal(OverlayMode::Modal);
    let (nested_open, set_nested_open) = create_signal(true);
    let dismissed = Rc::new(Cell::new(0));
    let nested_dismissed = Rc::new(Cell::new(0));
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (open, mode, nested_open) = (open.clone(), mode.clone(), nested_open.clone());
        let (reports, nested_reports) = (dismissed.clone(), nested_dismissed.clone());
        let (outer, inner) = (outer.clone(), inner.clone());
        let closing = set_nested_open.clone();
        move || {
            view! {
                <Frame>
                    <Overlay
                        @node_ref=&outer
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        mode={mode}
                        open={open}
                        on_dismiss={move || reports.set(reports.get() + 1)}
                    >
                        <List spacing=0.0>
                            <Frame width=40.0 height=30.0 />
                            <Overlay
                                @node_ref=&inner
                                anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                                placement=Placement::At
                                open={nested_open}
                                on_dismiss={move || {
                                    nested_reports.set(nested_reports.get() + 1);
                                    closing.set(false);
                                }}
                            >
                                <Frame width=40.0 height=30.0 />
                            </Overlay>
                        </List>
                    </Overlay>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let outer = kind_of::<OverlayNode>(harness.document(), outer.get());
    let inner = kind_of::<OverlayNode>(harness.document(), inner.get());
    fn set<T: Clone + PartialEq + 'static>(
        harness: &mut Harness,
        signal: &WriteSignal<T>,
        value: T,
    ) {
        let signal = signal.clone();
        with_reactive_scope(harness.document_mut(), move || signal.set(value));
        harness.frame(Vec::new());
    }
    assert!(harness.document().is_overlay_open(inner.id()));

    set(&mut harness, &set_mode, OverlayMode::Floating);
    assert!(harness.document().is_overlay_open(outer.id()));
    assert!(
        !harness.document().is_overlay_open(inner.id()),
        "changing the parent's mode closes the nested overlay"
    );
    assert_eq!(nested_dismissed.get(), 1);
    assert_eq!(dismissed.get(), 0, "and does not dismiss the parent");

    set(&mut harness, &set_nested_open, true);
    assert!(harness.document().is_overlay_open(inner.id()));
    set(&mut harness, &set_open, false);
    assert!(
        !harness.document().is_overlay_open(inner.id()),
        "closing a floating parent takes the nested overlay with it"
    );
    assert!(!harness.document().modal_open());
    assert_eq!(nested_dismissed.get(), 2);
    assert_eq!(dismissed.get(), 0);
}
