use super::*;
use accesskit::{Action, ActionData, ActionRequest, Node, NodeId, Role, TreeId, TreeUpdate};

#[test]
fn a_screen_readers_action_round_trips_and_a_huge_tree_is_refused() {
    let action = ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: NodeId(1 << 41 | 7),
        data: Some(ActionData::Value("Buy bread".into())),
    };
    let carried = crate::ops::decode::<crate::accessibility::ActionRequest>(&crate::ops::encode(
        &crate::accessibility::ActionRequest::from(action.clone()),
    ))
    .expect("the request decodes");
    assert_eq!(ActionRequest::from(carried), action);

    let huge = Message::Frame(FrameCommit {
        reports: vec![FrameReport {
            screen: ScreenId(4),
            content: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            painted: Vec::new(),
            floating: Vec::new(),
            drawing: Vec::new(),
            every_sample: false,
            claims: Vec::new(),
            description: Some(Description {
                tree: Some(TreeUpdate {
                    nodes: (0..=MAX_ACCESSIBILITY_NODES as u64)
                        .map(|id| (NodeId(id), Node::new(Role::Label)))
                        .collect(),
                    tree: None,
                    tree_id: TreeId::ROOT,
                    focus: NodeId(0),
                }),
                actions: Vec::new(),
            }),
        }],
        ..frame(1)
    });
    assert_eq!(
        encode_frame(&huge),
        Err(DecodeError::LimitExceeded("accessibility nodes"))
    );
}
