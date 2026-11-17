use super::*;

/// A panel that grows while open moves along its side; its beak moves with it on that
/// same frame, still pointing at the anchor.
#[test]
fn a_beak_follows_its_pointer_frame_when_the_panel_changes_size() {
    let _root = crate::reactive::RootScope::new();
    let width = create_signal(60.0f32);
    let button = NodeRef::new();
    let overlay = NodeRef::new();
    let pointer = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&button width=40.0 height=20.0 />
                <Overlay
                    @node_ref=&overlay
                    anchor={OverlayAnchor::Node(button)}
                    placement={Placement::BELOW_CENTER.gap(10).margin(8)}
                    policy=LayerPolicy::Pass
                    traps_focus=false
                    pointer={pointer}
                    beak=10.0
                    open=true
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame @node_ref=&pointer width={move || beui::Extent::px(width.get())} height=30.0 />
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 200.0));
    harness.frame(Vec::new());
    let beak = |harness: &Harness| {
        let document = harness.document();
        let overlay = document.overlay_of(overlay.get()).expect("an overlay");
        document.overlay_beak(overlay).expect("a beak")
    };
    for grown in [60.0, 200.0, 120.0] {
        width.set(grown);
        harness.frame(Vec::new());
        assert_eq!(
            harness.rect(beak(&harness)),
            harness.rect(pointer.get()),
            "the beak lies along the panel {grown} wide on the frame it changes size"
        );
    }
}
