use super::*;

#[test]
fn the_list_is_kept_and_shown_while_offline() {
    let mut boat = Boat::listing(SANDBOXES);
    let kept = boat
        .kept()
        .expect("the list is kept in the viewer's view state");
    assert_eq!(kept.sandboxes.len(), 3);
    assert_eq!(kept.sandboxes[0].name, "ci-runner-1");

    for _ in 0..20 {
        boat.wait(Duration::from_secs(15));
        let poll = boat.fetch();
        boat.offline(&poll);
    }

    assert_eq!(
        boat.label("boat.status"),
        "Offline: the list from 5 minutes ago"
    );
    assert!(boat.test.shown("boat.sandbox.bx_k7m2p9qa.detail"));
    boat.test
        .snapshot("the_list_is_kept_and_shown_while_offline");
}
