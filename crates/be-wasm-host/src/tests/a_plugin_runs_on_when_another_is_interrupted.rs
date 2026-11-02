use super::*;

#[test]
fn a_plugin_runs_on_when_another_is_interrupted() {
    let counting = guest(
        "(local $count i32)
         (loop $count_up
            (local.set $count (i32.add (local.get $count) (i32.const 1)))
            (br_if $count_up (i32.lt_u (local.get $count) (i32.const 100000))))
         (call $host_send (i32.const 0) (i32.const 1))",
    );
    let mut running = host().load_bytes(counting.as_bytes()).unwrap();
    running.start().unwrap();
    let stopped = host();
    let mut interrupted = stopped
        .load_bytes(guest("(loop $spin (br $spin))").as_bytes())
        .unwrap();
    interrupted.start().unwrap();

    stopped.interrupter().interrupt();

    assert_eq!(interrupted.step(), Err(INTERRUPTED.to_owned()));
    for _ in 0..3 {
        running.step().unwrap();
        assert_eq!(running.take_outbound().len(), 1);
    }
}
