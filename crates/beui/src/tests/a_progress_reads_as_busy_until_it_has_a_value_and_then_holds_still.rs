use accesskit::{Node, Role};

use super::*;
use crate::reactive::*;
use crate::styled::*;

fn progress_node(output: &crate::FrameOutput) -> Node {
    output
        .accessibility_tree("Test", VIEWPORT)
        .nodes
        .into_iter()
        .find(|(_, node)| node.role() == Role::ProgressIndicator)
        .expect("the progress is in the accessibility tree")
        .1
}

#[test]
fn a_progress_reads_as_busy_until_it_has_a_value_and_then_holds_still() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(None::<f32>);
    let document = build_node(move || {
        view! {
            <Progress value={value} label="Copying" />
        }
    });
    let mut harness = Harness::new(document);
    let progress = progress_node(&harness.frame(Vec::new()));
    assert!(progress.is_busy());
    assert_eq!(progress.label(), Some("Copying"));
    assert_eq!(progress.numeric_value(), None);
    let moving = harness.frame(Vec::new()).repaint_after;
    assert!(
        moving > Duration::ZERO && moving < Duration::MAX,
        "an indeterminate progress moves a frame's time apart, not as soon as it can"
    );

    value.set(Some(1.5));
    let progress = progress_node(&harness.frame(Vec::new()));
    assert!(!progress.is_busy());
    assert_eq!(progress.numeric_value(), Some(1.0), "the value is clamped");
    assert_eq!(
        harness.frame(Vec::new()).repaint_after,
        Duration::MAX,
        "a progress with a value holds still"
    );
}
