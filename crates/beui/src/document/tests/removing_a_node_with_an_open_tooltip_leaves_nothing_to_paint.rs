use std::time::Duration;

use super::*;
use crate::reactive::{Button, ForEach, List, Text, build, create_signal, view};
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn removing_a_node_with_an_open_tooltip_leaves_nothing_to_paint() {
    let document = build(|| {
        let (rows, set_rows) = create_signal(vec![1i64]);
        view! {
            <List spacing=0.0>
                <Button on_click={move || set_rows.set(Vec::new())}>
                    <Text string="remove" />
                </Button>
                <ForEach keys={rows}>
                    {|row: i64| view! {
                        <Tooltip
                            label="Add a root block"
                            delay={Duration::ZERO}
                            content={move |handle: TooltipHandle| {
                                view! {
                                    <Text @test_id={format!("tip.{row}")} string={handle.label} />
                                }
                            }}
                        >
                            <Text @test_id={format!("row.{row}")} string="described" />
                        </Tooltip>
                    }}
                </ForEach>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let described = harness.document().find_test_id("row.1").expect("the row");

    harness.frame(vec![Event::PointerMoved(harness.center(described))]);
    harness.frame(Vec::new());
    let tip = harness
        .document()
        .find_test_id("tip.1")
        .expect("the bubble");
    assert!(
        harness.document().node_rect(tip).is_some(),
        "the dwell must place the bubble"
    );

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().find_test_id("tip.1"),
        None,
        "the removed tooltip must leave nothing behind to paint"
    );
}
