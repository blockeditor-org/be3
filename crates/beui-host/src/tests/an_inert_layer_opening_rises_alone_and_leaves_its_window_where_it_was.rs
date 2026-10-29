use super::*;
use crate::LayerPolicy;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_inert_layer_opening_rises_alone_and_leaves_its_window_where_it_was() {
    let _root = crate::reactive::RootScope::new();
    let tip = create_signal(false);
    let menu = create_signal(false);
    let (below, above, inert, blocking) = (
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
    );
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Overlay
                    @node_ref=&below
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    policy=LayerPolicy::Pass
                    traps_focus=false
                    open=true
                >
                    <List spacing=0.0>
                        <Frame width=40.0 height=30.0 />
                        <Overlay
                            @node_ref=&inert
                            anchor=OverlayAnchor::Point(pos2(20.0, 40.0))
                            placement=Placement::At
                            policy=LayerPolicy::Inert
                            traps_focus=false
                            open={tip}
                        >
                            <Frame width=20.0 height=10.0 />
                        </Overlay>
                        <Overlay
                            @node_ref=&blocking
                            anchor=OverlayAnchor::Point(pos2(20.0, 40.0))
                            placement=Placement::At
                            open={menu}
                        >
                            <Frame width=20.0 height=10.0 />
                        </Overlay>
                    </List>
                </Overlay>
                <Overlay
                    @node_ref=&above
                    anchor=OverlayAnchor::Point(pos2(30.0, 10.0))
                    placement=Placement::At
                    policy=LayerPolicy::Pass
                    traps_focus=false
                    open=true
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let [below, above, inert, blocking] = [below, above, inert, blocking]
        .map(|node| kind_of::<OverlayNode>(harness.document(), node.get()));
    let order = |harness: &Harness| {
        let layers = harness.document().layers();
        let at = |overlay| layers.index_of(overlay).unwrap_or(usize::MAX);
        (at(below), at(above))
    };
    let (start_below, start_above) = order(&harness);
    assert!(start_below < start_above, "the later window starts on top");

    tip.set(true);
    harness.frame(Vec::new());
    let (now_below, now_above) = order(&harness);
    assert!(
        now_below < now_above,
        "a tooltip opening in the covered window leaves it covered"
    );
    let layers = harness.document().layers();
    assert!(
        layers.index_of(inert) > layers.index_of(below),
        "while the tooltip itself rises over the window it hangs in"
    );

    menu.set(true);
    harness.frame(Vec::new());
    let (now_below, now_above) = order(&harness);
    assert!(
        now_below > now_above,
        "a menu opening in the covered window brings the window up with it"
    );
    let layers = harness.document().layers();
    assert!(layers.index_of(blocking) > layers.index_of(below));
}
