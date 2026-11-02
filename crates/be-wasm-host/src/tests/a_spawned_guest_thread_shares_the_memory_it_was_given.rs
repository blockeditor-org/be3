use super::*;

#[test]
fn a_spawned_guest_thread_shares_the_memory_it_was_given() {
    let source = threaded_guest(
        "(i32.atomic.store8 (i32.const 8) (i32.const 42))
         (call $host_wake)",
    );
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();
    assert_eq!(settled(&mut app, 42), Ok(()));
}
