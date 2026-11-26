use accesskit::Role;

use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::CommandPaletteApi;
use crate::{KeyEvent, KeyPhase};

#[test]
fn a_slider_offers_no_move_and_no_live_region_for_its_thumb() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(0.5);
    let palette = NodeRef::new();
    let document = build_node(move || {
        let open = create_signal(false);
        on_key(
            KeyAt::Root,
            KeyPhase::Capture,
            KeyReveal::Focus,
            move |event: KeyEvent| {
                let wanted = event.press().is_some_and(|press| {
                    press.pressed
                        && press.key == Key::P
                        && press.modifiers.ctrl
                        && press.modifiers.shift
                });
                if wanted {
                    open.set(true);
                }
                wanted
            },
        );
        view! {
            <List spacing=0.0>
                <Slider @test_id={"slider"} value={value} />
                <CommandPalette @node_ref=&palette open />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let nodes = harness
        .frame(Vec::new())
        .accessibility_tree("Test", VIEWPORT)
        .nodes;
    assert!(
        nodes.iter().all(|(_, node)| node.role() != Role::Status),
        "a thumb's carry says nothing: the slider's value says where it is"
    );

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    let shift_ctrl = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowRight, shift_ctrl);
    harness.frame(Vec::new());
    assert_eq!(
        value.get_untracked(),
        0.5,
        "neither Space nor Ctrl+Shift and an arrow lifts the thumb"
    );

    harness.key(Key::P, shift_ctrl);
    let shown = harness
        .document()
        .api::<CommandPaletteApi>(palette.get())
        .shown();
    assert!(
        !shown.iter().any(|action| action == "drag.move"),
        "the palette offers no Move for it: {shown:?}"
    );
}
