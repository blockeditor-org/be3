use super::*;

#[test]
fn a_popovers_beak_survives_its_reach_going_away_and_back() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let carded = create_signal(false);
    let button = NodeRef::new();
    let (document, [_]) = toolbar_of(move || {
        [view! {
            <List spacing=0.0>
                <Frame @node_ref=&button width=40.0 height=20.0 />
                <Show condition={shown}>|| {
                    let pointer = NodeRef::new();
                    let beak = create_memo(move || if carded.get() { 10.0 } else { 0.0 });
                    let placement = create_memo(move || if carded.get() { Placement::Popover { gap: 4, margin: 8 } } else { Placement::BelowStart });
                    let inner = view! {
                        <Overlay
                            anchor={OverlayAnchor::Node(button)}
                            placement
                            policy=LayerPolicy::Pass
                            traps_focus=false
                            pointer={pointer}
                            beak
                            open=true
                            on_dismiss={|_: Dismiss| {}}
                        >
                            <Frame @node_ref=&pointer width=50.0 height=50.0 />
                        </Overlay>
                    };
                    view! {
                        <Overlay
                            anchor={OverlayAnchor::Point(Pos2::ZERO)}
                            placement=Placement::At
                            policy=LayerPolicy::Inert
                            traps_focus=false
                            dismissable=false
                            open=true
                            on_dismiss={|_: Dismiss| {}}
                        >
                            {inner}
                        </Overlay>
                    }
                }</Show>
            </List>
        }]
    });
    let mut harness = Harness::sized(document, vec2(160.0, 80.0));
    give_layer_space(&mut harness);
    harness.frame(Vec::new());
    assert_eq!(harness.document().escaped_layers().len(), 1);
    for carded_now in [true, false, true, false] {
        carded.set(carded_now);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
    }
    shown.set(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(harness.document().escaped_layers().is_empty());
}
