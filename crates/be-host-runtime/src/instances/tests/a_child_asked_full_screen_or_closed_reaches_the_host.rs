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

    tell(&mut instances, ShellMessage::CloseChild(ChildId(1)));
    assert_eq!(instances.take_child_closes(INSTANCE), vec![ChildId(1)]);
    assert!(
        instances.take_child_closes(INSTANCE).is_empty(),
        "each close is taken once"
    );
}
