use accesskit::{Live, Role};

use super::*;
use crate::primitives::Frame;
use crate::styled::*;

#[test]
fn toasts_read_as_a_live_log_with_errors_as_alerts() {
    let document = build_node(move || {
        let area = NodeRef::new();
        let toasts = vec![
            Toast {
                id: 1,
                title: "Saved".to_owned(),
                message: "Notes".to_owned(),
                ..Toast::default()
            },
            Toast {
                id: 2,
                message: "Sync failed".to_owned(),
                danger: true,
                ..Toast::default()
            },
        ];
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts anchor={area} toasts={toasts} />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let nodes = harness
        .frame(Vec::new())
        .accessibility_tree("Test", VIEWPORT)
        .nodes;
    let find = |role: Role| {
        nodes
            .iter()
            .find(|(_, node)| node.role() == role)
            .map(|(_, node)| node.clone())
            .unwrap_or_else(|| panic!("a {role:?} node is in the tree"))
    };
    let log = find(Role::Log);
    assert_eq!(log.live(), Some(Live::Polite), "the stack is a live region");
    let status = find(Role::Status);
    assert_eq!(status.label(), Some("Saved: Notes"));
    let alert = find(Role::Alert);
    assert_eq!(alert.label(), Some("Sync failed"));
    assert_eq!(
        alert.live(),
        Some(Live::Assertive),
        "an error interrupts what is being read"
    );
}
