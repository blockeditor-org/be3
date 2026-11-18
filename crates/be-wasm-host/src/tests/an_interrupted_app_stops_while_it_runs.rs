use super::*;

use std::sync::mpsc;

#[test]
fn an_interrupted_app_stops_while_it_runs() {
    let host = host();
    let interrupter = host.interrupter();
    let (woken, wakes) = mpsc::channel();
    let spinning = guest("(call $host_wake) (loop $spin (br $spin))")
        .replace(
            r#"(import "be3_host" "host_send""#,
            r#"(import "be3_host" "host_wake" (func $host_wake))
            (import "be3_host" "host_send""#,
        );
    let worker = std::thread::spawn(move || {
        let mut app = host.load_bytes(spinning.as_bytes()).unwrap();
        app.on_wake(move || {
            let _ = woken.send(());
        });
        app.start().unwrap();
        app.step()
    });
    wakes.recv().expect("the app should have started its step");

    interrupter.interrupt();

    assert_eq!(worker.join().unwrap(), Err(INTERRUPTED.to_owned()));
}
