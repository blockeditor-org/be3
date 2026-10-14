use super::*;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::reactive::{
    Frame, NodeRef, Overlay, WriteSignal, build, create_signal, view, with_reactive_scope,
};
use crate::{Dismiss, LayerPolicy};

#[test]
fn a_nested_overlay_closes_with_its_parent_whatever_the_parents_policy() {
    let _root = crate::reactive::RootScope::new();
    let (open, set_open) = create_signal(true);
    let (policy, set_policy) = create_signal(LayerPolicy::BlocksAll);
    let (nested_open, set_nested_open) = create_signal(true);
    let (dismissed, set_dismissed) = create_signal(0);
    let (nested_dismissed, set_nested_dismissed) = create_signal(0);
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            view! {
                <Frame>
                    <Overlay
                        @node_ref=&outer
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        policy={policy}
                        open={open}
                        on_dismiss={move |_: Dismiss| set_dismissed.update(|dismissed| *dismissed += 1)}
                    >
                        <List spacing=0.0>
                            <Frame width=40.0 height=30.0 />
                            <Overlay
                                @node_ref=&inner
                                anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                                placement=Placement::At
                                open={nested_open}
                                on_dismiss={move |_: Dismiss| {
                                    set_nested_dismissed.update(|nested_dismissed| *nested_dismissed += 1);
                                    set_nested_open.set(false);
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
        signal: WriteSignal<T>,
        value: T,
    ) {
        with_reactive_scope(harness.document_mut(), move || signal.set(value));
        harness.frame(Vec::new());
    }
    assert!(harness.document().is_overlay_open(inner.id()));

    set(&mut harness, set_policy, LayerPolicy::Pass);
    assert!(harness.document().is_overlay_open(outer.id()));
    assert!(
        harness.document().is_overlay_open(inner.id()),
        "changing the parent's policy leaves the nested overlay open"
    );
    let layers = harness.document().layers();
    let nested = layers
        .index_of(inner)
        .expect("the nested overlay is a layer");
    assert_eq!(
        layers[nested].parent,
        layers.index_of(outer),
        "and its layer stays a child of the parent's"
    );

    set(&mut harness, set_open, false);
    assert!(
        !harness.document().is_overlay_open(inner.id()),
        "set_nested_open a passing parent takes the nested overlay with it"
    );
    assert_eq!(harness.document().layers().len(), 1);
    assert_eq!(nested_dismissed.get_untracked(), 1);
    assert_eq!(dismissed.get_untracked(), 0);
}
