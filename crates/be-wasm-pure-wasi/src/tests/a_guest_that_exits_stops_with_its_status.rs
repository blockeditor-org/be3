use super::*;

#[test]
fn a_guest_that_exits_stops_with_its_status() {
    let stopped = Wasi::new([0; 32])
        .call("proc_exit", &[3], &mut Bytes(Vec::new()), &mut |_| {})
        .unwrap_err();
    assert_eq!(stopped, Stopped::Exited(3));
    assert_eq!(stopped.to_string(), "the guest exited with status 3");
}
