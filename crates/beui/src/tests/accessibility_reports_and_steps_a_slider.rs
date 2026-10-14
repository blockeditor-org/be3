use accesskit::{Action, ActionRequest, Role, TreeId};

use super::{Harness, VIEWPORT, build, create_signal, styled, view};

#[test]
fn accessibility_reports_and_steps_a_slider() {
    let _root = crate::reactive::RootScope::new();
    let (changed, set_changed) = create_signal(None);
    let document = build(move || {
        view! {
            <styled::Slider
                value=0.5
                label="Volume"
                on_change={move |value| set_changed.set(Some(value))}
            />
        }
    });
    let mut harness = Harness::new(document);

    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let (slider_id, slider) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Slider)
        .expect("slider is absent from the accessibility tree");
    assert_eq!(slider.label(), Some("Volume"));
    assert_eq!(slider.numeric_value(), Some(0.5));
    assert!(slider.supports_action(Action::Increment));
    let slider_id = *slider_id;

    harness.context.accessibility_action(ActionRequest {
        action: Action::Increment,
        target_tree: TreeId::ROOT,
        target_node: slider_id,
        data: None,
    });
    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let slider = tree
        .nodes
        .iter()
        .find_map(|(id, node)| (*id == slider_id).then_some(node))
        .expect("slider disappeared from the accessibility tree");

    assert_eq!(changed.get_untracked(), Some(0.55));
    let value = slider
        .numeric_value()
        .expect("slider lost its numeric value");
    assert!((value - 0.55).abs() < f32::EPSILON.into());
}
