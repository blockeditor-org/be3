use super::*;
use accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId};

#[test]
fn a_screen_readers_action_round_trips() {
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

}
