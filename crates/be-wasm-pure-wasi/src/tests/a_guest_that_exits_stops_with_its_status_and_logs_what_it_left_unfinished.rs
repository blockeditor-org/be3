use super::*;

#[test]
fn a_guest_that_exits_stops_with_its_status_and_logs_what_it_left_unfinished() {
    let mut wasi = Wasi::new([0; 32]);
    let mut logged = Vec::new();
    write(&mut wasi, 2, b"almost", &mut |line| logged.push(line.to_owned()));
    assert!(logged.is_empty(), "half a line waits for the rest");
    let stopped = wasi
        .call("proc_exit", &[3], &mut Bytes(Vec::new()), &mut |line| {
            logged.push(line.to_owned())
        })
        .unwrap_err();
    assert_eq!(stopped, Stopped::Exited(3));
    assert_eq!(stopped.to_string(), "the guest exited with status 3");
    assert_eq!(logged, ["almost"]);
}
