use super::*;

#[test]
fn reading_its_own_selection_leaves_the_program_able_to_close() {
    let Some(server) = Xvfb::start() else {
        return;
    };
    let _watchdog = Watchdog::start(&server);
    let owner = Owner::connect(Some(&server.display)).expect("the program connects");
    // Larger than one piece, so it goes by INCR to the program's own window.
    let large = ClipItem::new(formats::CANVAS_ITEMS, vec![5; CHUNK + 9]);
    owner.offer(&[ClipItem::text("plain"), large.clone()]);
    let targets = owner.targets();
    assert!(
        targets.iter().any(|target| target == formats::CANVAS_ITEMS),
        "the program lists what it offers: {targets:?}"
    );
    for _ in 0..2 {
        assert_eq!(owner.read(formats::CANVAS_ITEMS).as_ref(), Some(&large));
    }
    // Dropping waits for the serving thread, which ends only once its window's
    // destruction is heard: a transfer to itself must not have taken that.
    drop(owner);
}
