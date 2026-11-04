use std::time::Duration;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn a_tooltip_appears_after_a_dwell_and_leaves_the_control_clickable() {
    let _root = crate::reactive::RootScope::new();
    let clicked = create_signal(0u32);
    let button = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Tooltip
                    label="Add a root block"
                    delay={Duration::ZERO}
                    content={move |handle: TooltipHandle| {
                        view! {
                            <Text @test_id="tip">{handle.label}</Text>
                        }
                    }}
                >
                    <Button
                        @node_ref=&button
                        on_click={move || clicked.update(|clicked| *clicked += 1)}
                    >
                        <Text>"add"</Text>
                    </Button>
                </Tooltip>
            </List>
        }
    });

    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("tip"),
        None,
        "a tooltip nobody is hovering is not on screen"
    );

    let center = harness.center(button);
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.frame(Vec::new());
    let tip = harness.document().find_test_id("tip").expect("the bubble");
    assert!(
        harness.document().node_rect(tip).is_some(),
        "the dwell must place the bubble"
    );

    harness.click(center);
    assert_eq!(
        clicked.get_untracked(),
        1,
        "an open tooltip must not take the press away from what it describes"
    );
}
