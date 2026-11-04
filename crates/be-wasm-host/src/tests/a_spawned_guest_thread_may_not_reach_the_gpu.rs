use super::*;

#[test]
fn a_spawned_guest_thread_may_not_reach_the_gpu() {
    let source = threaded_guest(
        "(drop (call $create_buffer (i32.const 0) (i32.const 0)))
         (i32.atomic.store8 (i32.const 8) (i32.const 42))",
    );
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    let started = app.start();
    let failure = started
        .and_then(|()| settled(&mut app, 42))
        .expect_err("the thread should have been refused");
    assert!(failure.contains("create-buffer"), "{failure}");
}
