use super::*;

#[test]
fn a_pure_guest_may_not_outgrow_its_memory_limit() {
    let host = Host::pure(None).unwrap().with_memory_limit(64 << 20);
    let mut guest = host.load_bytes(ECHO).unwrap();
    guest.start().unwrap();
    assert_eq!(
        guest.take_outbound(),
        vec![block_guest_echo::STARTED.to_vec()]
    );
    guest.deliver(vec![block_guest_echo::GROW.to_vec()]);
    assert!(
        guest.step().is_err(),
        "a guest that grows past its limit traps"
    );
    assert!(guest.take_outbound().is_empty());
}
