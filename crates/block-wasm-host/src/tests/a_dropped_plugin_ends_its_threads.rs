use super::*;

#[test]
fn a_dropped_plugin_ends_its_threads() {
    let source = threaded_guest("(loop $spin (br $spin))");
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    plugin.start().unwrap();
    let threads = Arc::clone(&plugin.store.data().threads);

    drop(plugin);

    assert!(
        threads.await_ended(Duration::from_secs(30)),
        "a thread of the dropped plugin is still running"
    );
}
