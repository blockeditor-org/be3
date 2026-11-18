use std::time::Duration;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn a_hovered_tooltip_leaves_escape_to_the_focused_control_that_takes_it() {
    let _root = crate::reactive::RootScope::new();
    let escapes = create_signal(0u32);
    let hinted = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=20.0>
                <Button
                    on_key={move |press: KeyPress| {
                        let escape = press.key == Key::Escape;
                        if escape && press.pressed {
                            escapes.update(|escapes| *escapes += 1);
                        }
                        escape
                    }}
                >
                    <Text>"takes Escape"</Text>
                </Button>
                <Tooltip
                    label="Says more"
                    delay={Duration::ZERO}
                    content={move |handle: TooltipHandle| view! {
                        <Text @test_id="tip">{handle.label}</Text>
                    }}
                >
                    <Button @node_ref=&hinted>
                        <Text>"hinted"</Text>
                    </Button>
                </Tooltip>
            </List>
        }
    });
    let hinted = hinted.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(harness.center(hinted))]);
    harness.frame(Vec::new());
    let tip = |harness: &Harness| harness.document().find_test_id("tip").is_some();
    assert!(tip(&harness), "hovering shows the tooltip");

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        escapes.get_untracked(),
        1,
        "the focused control that answers Escape keeps it"
    );
    assert!(tip(&harness), "so the tooltip stays shown");
}
