use super::*;

#[test]
fn a_hidden_window_child_takes_no_room_and_is_kept() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let mut placements = child(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO), false);
    placements.children[0].content = ChildContent::Window(be_sdk_api::HostWindowId(4));
    placements.children[0].mode = ChildMode::Hidden;
    instances.set_children(placements.clone());
    let region = Rect::from_min_size(Pos2::ZERO, SIZE);
    let (children, _) = instances.host_children(INSTANCE, REGION, region, region, None);
    assert_eq!(
        children.iter().map(|child| child.mode).collect::<Vec<_>>(),
        [ChildMode::Hidden],
        "a child placed hidden is kept though it takes no room"
    );

    placements.children[0].mode = ChildMode::Live;
    instances.set_children(placements);
    let (children, _) = instances.host_children(INSTANCE, REGION, region, region, None);
    assert!(
        children.is_empty(),
        "a shown child with no room is placed nowhere"
    );
}
