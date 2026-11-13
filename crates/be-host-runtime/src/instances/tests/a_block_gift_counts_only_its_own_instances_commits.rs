use super::*;

#[test]
fn a_block_gift_counts_only_its_own_instances_commits() {
    let mut instances = placed();
    instances.next_screens(PASS);
    tell(&mut instances, ShellMessage::FocusBlockChild([3; 16]));
    for _ in 0..gift::GIFT_COMMITS * 4 {
        instances.settle_gifts(&[EditorInstanceId(99)]);
    }
    assert!(
        instances.gift_pending(INSTANCE),
        "another instance of the app drawing is no reason to give up"
    );
    let report = |screen| FrameReport {
        screen,
        content: None,
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        marks: Vec::new(),
        marks_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        actions: None,
        anchored: None,
        work_area: None,
        backdrop: None,
    };
    let own = instances.entries[&INSTANCE].screens[&REGION].request.screen;
    assert_eq!(
        instances.committed(&[report(own)], &[]),
        [INSTANCE],
        "a frame report of one of its views is a commit of its own"
    );
    for _ in 0..gift::GIFT_COMMITS {
        instances.settle_gifts(&[INSTANCE]);
    }
    assert!(
        !instances.gift_pending(INSTANCE),
        "its own commits count, and no child it placed showed the block"
    );
}
