use super::*;

#[test]
fn a_spawned_guest_thread_wakes_the_host() {
    let source = threaded_guest("(call $host_wake)");
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    let told = Told::listen(&app);

    app.start().unwrap();
    told.wait();

    assert!(
        app.take_wake(),
        "the host was told of a wake the app did not keep"
    );
}
