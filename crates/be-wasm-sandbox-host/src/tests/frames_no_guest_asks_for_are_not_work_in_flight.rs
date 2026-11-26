use super::*;

/// Frames an instance sent that its guest has not asked for are the guest's to take when it
/// asks, not work in flight: a server holding them is settled. Once the guest waits for them,
/// they are owed it until `poll` hands them over.
#[test]
fn frames_no_guest_asks_for_are_not_work_in_flight() {
    let mut server = idle_server();
    server
        .instances
        .insert(1, stopped_served(vec![b"hello".to_vec()], false));
    assert!(server.settled(), "frames no one asked for wait for no one");
    server
        .instances
        .insert(1, stopped_served(vec![b"hello".to_vec()], true));
    assert!(!server.settled(), "frames the guest waits for are owed it");
    server.instances.insert(1, stopped_served(Vec::new(), true));
    assert!(
        server.settled(),
        "a guest waiting with nothing sent waits on no one"
    );
}
