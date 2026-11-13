use super::*;

#[test]
fn a_child_asked_full_screen_or_closed_reaches_the_host() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let mut placements = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
    placements.children[0].fullscreen = true;
    instances.set_children(placements);
    let region = Rect::from_min_size(Pos2::ZERO, SIZE);
    let (children, _) = instances.host_children(INSTANCE, REGION, region, region, None);
    assert!(
        children[0].fullscreen,
        "the host reads that its placer shows it full screen"
    );

    let (_mailbox, heard) = hearing();
    tell(&mut instances, ShellMessage::CloseChild(ChildId(1)));
    host::deliver();
    let closed: Vec<ChildId> = heard
        .borrow()
        .iter()
        .filter_map(|event| match event {
            InstanceEvent::ChildClosed(child) => Some(*child),
            _ => None,
        })
        .collect();
    assert_eq!(closed, vec![ChildId(1)], "the close reaches the host once");
}
