use super::*;

#[test]
fn taking_the_frame_back_takes_it_from_every_child_of_that_view() {
    // Two live children of one view: the first takes its frame over, the second waits.
    let mut children = placing(vec![up(1, ChildMode::Live), up(2, ChildMode::Live)]);
    children.shown[0].framing = true;
    children.revoke(&[Pressed {
        layer: PLACER,
        escape: false,
    }]);
    // A press on the placer gives the frame back to it, not to the second child.
    for id in [1, 2] {
        assert!(
            children
                .frame_revoked
                .contains(&(EditorInstanceId(1), ViewId::MAIN, ChildId(id))),
            "child {id} is taken back from the frame",
        );
    }
}
