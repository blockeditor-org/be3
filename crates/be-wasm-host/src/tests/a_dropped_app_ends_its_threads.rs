use super::*;

#[test]
fn a_dropped_app_ends_its_threads() {
    let source = threaded_guest("(loop $spin (br $spin))");
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.start().unwrap();
    let threads = Arc::clone(&app.store.data().threads);

    drop(app);

    assert!(
        threads.await_ended(Duration::from_secs(30)),
        "a thread of the dropped app is still running"
    );
}
