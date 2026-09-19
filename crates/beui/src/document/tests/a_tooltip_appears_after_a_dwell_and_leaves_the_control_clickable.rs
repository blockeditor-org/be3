use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::*;
use crate::reactive::{Button, List, Text, build, view};
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn a_tooltip_appears_after_a_dwell_and_leaves_the_control_clickable() {
    let clicked = Rc::new(Cell::new(0u32));
    let button = NodeRef::new();
    let document = build({
        let clicked = Rc::clone(&clicked);
        let button = button.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Tooltip
                        label="Add a root block"
                        delay={Duration::ZERO}
                        content={move |handle: TooltipHandle| {
                            view! {
                                <Text @test_id="tip" string={handle.label} />
                            }
                        }}
                    >
                        <Button
                            @node_ref=&button
                            on_click={move || clicked.set(clicked.get() + 1)}
                        >
                            <Text string="add" />
                        </Button>
                    </Tooltip>
                </List>
            }
        }
    });

    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tip = harness.document().find_test_id("tip").expect("the bubble");
    assert_eq!(
        harness.document().node_rect(tip),
        None,
        "a tooltip nobody is hovering must not be placed"
    );

    let center = harness.center(button);
    harness.frame(vec![Event::PointerMoved(center)]);
    harness.frame(Vec::new());
    assert!(
        harness.document().node_rect(tip).is_some(),
        "the dwell must place the bubble"
    );

    harness.click(center);
    assert_eq!(
        clicked.get(),
        1,
        "an open tooltip must not take the press away from what it describes"
    );
}
