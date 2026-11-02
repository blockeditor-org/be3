use accesskit::{Action, ActionRequest, NodeId as AccessNodeId, TreeId};
use beui_view::reactive::Get;
use beui_view::reactive::Set;

use super::*;
use crate::LayerLevel;
use crate::reactive::*;

fn named(harness: &Harness, id: &str) -> Option<(AccessNodeId, bool)> {
    ::beui_reactive::flush();
    let document = harness.document();
    let view = document.accessibility_view()?;
    let mut pending = vec![(view.root(), false)];
    while let Some((at, within_hidden)) = pending.pop() {
        let Some(node) = view.get(&at) else {
            continue;
        };
        let hidden = within_hidden || node.is_hidden();
        if node.author_id().is_some_and(|names| {
            names
                .split(beui_core::accessibility::TEST_ID_SEPARATOR)
                .any(|name| name == id)
        }) {
            return Some((at, hidden));
        }
        pending.extend(node.children().iter().map(|child| (*child, hidden)));
    }
    None
}

fn shown(harness: &Harness, id: &str) -> bool {
    named(harness, id).is_some_and(|(_, hidden)| !hidden)
}

#[test]
fn a_modal_hides_what_lies_beneath_it_from_assistive_technology_and_its_actions() {
    for level in [LayerLevel::Normal, LayerLevel::Lock] {
        let _root = crate::reactive::RootScope::new();
        let open = create_signal(false);
        let clicks = create_signal(0);
        let placed = create_signal(None::<EmbedPlacement>);
        let slot = EmbedSlot::new();
        slot.on_place(move |placement| placed.set(placement));
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Button
                        label="Behind"
                        @test_id={"behind"}
                        on_click={move || clicks.update(|clicks| *clicks += 1)}
                    />
                    <Frame width=80.0 height=40.0>
                        <Embed slot={slot} />
                    </Frame>
                    <Modal open={open} level={level}>|| {
                        view! { <Button label="Inside" @test_id={"inside"} on_click={|| ()} /> }
                    }</Modal>
                </List>
            }
        });
        let mut harness = Harness::new(document);
        harness.frame(Vec::new());
        let (behind, _) = named(&harness, "behind").expect("the button is described");
        assert!(shown(&harness, "behind"), "{level:?}: nothing hides the button yet");
        assert_eq!(placed.get_untracked().map(|placed| placed.inert), Some(false));

        open.set(true);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        assert!(
            !shown(&harness, "behind"),
            "{level:?}: what lies beneath a modal is hidden from assistive technology"
        );
        assert!(shown(&harness, "inside"), "{level:?}: while the modal itself is not");
        assert_eq!(
            placed.get_untracked().map(|placed| placed.inert),
            Some(true),
            "{level:?}: an embed beneath the modal hears it is inert, so its app can be hidden too"
        );
        harness.context().accessibility_action(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: behind,
            data: None,
        });
        harness.frame(Vec::new());
        assert_eq!(
            clicks.get_untracked(),
            0,
            "{level:?}: assistive technology cannot press what the modal covers"
        );

        open.set(false);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        assert!(shown(&harness, "behind"), "{level:?}: closing the modal gives it back");
        assert_eq!(placed.get_untracked().map(|placed| placed.inert), Some(false));
        harness.context().accessibility_action(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: behind,
            data: None,
        });
        harness.frame(Vec::new());
        assert_eq!(clicks.get_untracked(), 1, "{level:?}: and its actions");
    }
}
