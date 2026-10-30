use accesskit::{Action, ActionRequest, Role, TreeId};

use super::*;
use crate::reactive::*;

#[test]
fn focusing_the_focused_node_again_keeps_a_pointer_focus_unringed() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <styled::List spacing=styled::Space::Md>
                <styled::Button label="First" @test_id="first" />
                <styled::Button label="Second" @test_id="second" />
            </styled::List>
        }
    });
    let mut harness = Harness::new(document);
    let tree = harness
        .frame(Vec::new())
        .accessibility_tree("Test", VIEWPORT);
    let button = |label: &str| {
        tree.nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some(label))
            .map(|(id, _)| *id)
            .expect("the button is in the accessibility tree")
    };
    let buttons = [button("First"), button("Second")];
    let first = harness.find("first");
    let at = harness.center(first);
    harness.click(at);
    harness.frame(Vec::new());
    let visible = harness.document().watch_focus_visible();
    assert!(!visible.get_untracked(), "a click shows no focus ring");

    harness.frame(vec![Input::Focus(false)]);
    harness.frame(vec![Input::Focus(true)]);
    assert_eq!(harness.document().focused_node(), Some(first.into()));
    assert!(
        !visible.get_untracked(),
        "regaining the window's focus shows no ring"
    );
    let focus = |target_node| ActionRequest {
        action: Action::Focus,
        target_tree: TreeId::ROOT,
        target_node,
        data: None,
    };
    harness.context().accessibility_action(focus(buttons[0]));
    harness.frame(Vec::new());
    assert!(
        !visible.get_untracked(),
        "a window that gets the focus back, and is asked to focus what already has it, shows no ring"
    );

    harness.context().accessibility_action(focus(buttons[1]));
    harness.frame(Vec::new());
    assert!(
        visible.get_untracked(),
        "moving the focus from assistive technology shows where it went"
    );
}
