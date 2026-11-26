use accesskit::{Action, ActionRequest, Role, TreeId};

use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_screen_reader_steps_a_sheet_handle_between_its_stops() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <Sheet @node_ref=&sheet extent=600.0>
                    <Frame height=1000.0 />
                </Sheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    let tree = harness
        .frame(Vec::new())
        .accessibility_tree("Test", Vec2::new(400.0, 600.0));
    let (handle, slider) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Slider && node.label() == Some("Sheet height"))
        .expect("the handle is a slider named for what it sets");
    assert!(
        slider.supports_action(Action::Increment) && slider.supports_action(Action::Decrement),
        "a screen reader can step it"
    );
    let handle = *handle;
    harness.settle();
    let height = |harness: &Harness| {
        harness
            .document()
            .node_rect(sheet.get())
            .map(|_| sheet_raised(harness, sheet.get()))
    };
    assert_eq!(height(&harness), Some(300.0));

    let act = |harness: &mut Harness, action| {
        harness.context().accessibility_action(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: handle,
            data: None,
        });
        harness.settle();
    };
    let value = |harness: &mut Harness| {
        harness
            .accessible()
            .into_iter()
            .find(|node| node.role() == Role::Slider)
            .and_then(|node| Some((node.numeric_value()?.round(), node.value()?.to_owned())))
    };
    assert_eq!(
        value(&mut harness),
        Some((50.0, "Stop 2 of 3".to_owned())),
        "read as the stop it rests at, its value the share of the extent it stands at"
    );
    act(&mut harness, Action::Increment);
    assert_eq!(
        height(&harness),
        Some(540.0),
        "Increment takes it a stop up"
    );
    assert_eq!(value(&mut harness), Some((90.0, "Stop 3 of 3".to_owned())));
    act(&mut harness, Action::Decrement);
    assert_eq!(height(&harness), Some(300.0), "and Decrement a stop down");
}
