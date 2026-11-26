use super::*;

#[test]
fn a_press_outside_a_child_taking_the_frame_takes_the_frame_back() {
    let press = |layer| Pressed {
        layer,
        escape: false,
    };
    let mut children = placing(vec![up(1, ChildMode::Live)]);
    children.shown[0].framing = true;
    let taken = |children: &Children| {
        children
            .frame_revoked
            .contains(&(EditorInstanceId(1), ViewId::MAIN, ChildId(1)))
    };

    // A press in it, its chrome or its content, keeps the frame.
    children.revoke(&[press(LayerId(201))]);
    assert!(!taken(&children));

    // A press on its placer, where its content shows outside its place, takes the frame back,
    // and a live child is still live.
    children.revoke(&[press(PLACER)]);
    assert!(taken(&children));
    assert!(children.shown[0].takes_input(&children.revoked));

    // Placed live again, it may take the frame again only once placed otherwise first.
    children.forget_revoked();
    assert!(taken(&children));
    children.shown[0].placement.mode = ChildMode::Passive;
    children.forget_revoked();
    assert!(!taken(&children));
}
