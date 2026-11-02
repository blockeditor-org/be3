use super::*;

#[test]
fn a_thread_that_trapped_wakes_the_host_to_be_told() {
    let source = threaded_guest("(drop (call $create_buffer (i32.const 0) (i32.const 0)))");
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    let told = Told::listen(&app);

    let started = app.start();
    told.wait();
    let failure = started
        .and_then(|()| app.step())
        .expect_err("the host should be told what stopped the thread");

    assert!(failure.contains("create-buffer"), "{failure}");
}
