use super::*;

#[test]
fn the_first_participant_names_the_build_that_may_speak() {
    let mut lease = Lease::new();
    lease.join(1, BUILD, 0);
    lease.join(2, [2; 32], 0);
    assert_eq!(lease.state().tag, Some(BUILD));
    assert!(lease.speaks(1));
    assert!(!lease.speaks(2), "another build spoke in the session");
    assert!(!lease.speaks(3), "a client outside the session spoke in it");

    lease.leave(1, 10);
    lease.leave(2, 10);
    lease.join(2, [2; 32], 20);
    assert_eq!(lease.state().tag, Some([2; 32]));
    assert!(lease.speaks(2));
}
