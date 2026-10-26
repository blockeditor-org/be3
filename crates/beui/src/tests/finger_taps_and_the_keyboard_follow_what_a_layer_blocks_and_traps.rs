use super::*;
use crate::LayerPolicy;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn finger_taps_and_the_keyboard_follow_what_a_layer_blocks_and_traps() {
    let _root = crate::reactive::RootScope::new();
    let taps = create_signal(0);
    let policy = create_signal(LayerPolicy::Pass);
    let traps = create_signal(false);
    let document = build_node(move || {
        on_finger_tap(move |_: usize| {
            taps.update(|taps| *taps += 1);
            true
        });
        view! {
            <Frame width=400.0 height=300.0>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(300.0, 200.0))
                    placement=Placement::At
                    policy={policy}
                    traps_focus={traps}
                    open=true
                >
                    <Frame width=40.0 height=30.0 />
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let tap = |harness: &mut Harness| {
        let before = taps.get_untracked();
        harness.finger(1, TouchPhase::Start, pos2(50.0, 50.0));
        harness.finger(2, TouchPhase::Start, pos2(120.0, 50.0));
        harness.finger(2, TouchPhase::End, pos2(120.0, 50.0));
        harness.finger(1, TouchPhase::End, pos2(50.0, 50.0));
        let wants = harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .wants_keyboard;
        (taps.get_untracked() > before, wants)
    };

    assert_eq!(
        tap(&mut harness),
        (true, false),
        "a layer that passes and traps nothing changes neither"
    );

    traps.set(true);
    assert_eq!(
        tap(&mut harness),
        (true, true),
        "one that traps the focus wants the keyboard, and the fingers still tap the document"
    );

    traps.set(false);
    policy.set(LayerPolicy::BlocksAll);
    assert_eq!(
        tap(&mut harness),
        (false, false),
        "one that blocks everything shuts out the tap, and leaves the keyboard where it was"
    );
}
