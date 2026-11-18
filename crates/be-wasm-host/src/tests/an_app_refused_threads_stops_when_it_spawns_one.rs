use super::*;

#[test]
fn an_app_refused_threads_stops_when_it_spawns_one() {
    let source = threaded_guest("(unreachable)");
    let mut app = host().load_bytes(source.as_bytes()).unwrap();
    app.refuse_threads();

    let started = app.start();

    let failure = started.expect_err("an app that may start no threads stops when it spawns one");
    assert!(
        failure.contains("may start no threads"),
        "it says why it stopped: {failure}"
    );
}
