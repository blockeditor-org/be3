use super::*;

use std::sync::mpsc;

const STUCK: &str = r#"(module
    (import "env" "memory" (memory 1 4 shared))
    (import "wasi" "thread-spawn" (func $spawn (param i32) (result i32)))
    (import "be3_host" "host_wake" (func $host_wake))
    (func (export "plugin_start")
        (drop (call $spawn (i32.const 0)))
        (drop (call $spawn (i32.const 1))))
    (func (export "plugin_shutdown"))
    (func (export "plugin_step")
        (call $host_wake)
        (loop $spin (br $spin)))
    (func (export "wasi_thread_start") (param i32 i32)
        (call $host_wake)
        (if (local.get 1)
            (then (loop $wait
                (drop (memory.atomic.wait32 (i32.const 64) (i32.const 0) (i64.const -1)))
                (br $wait))))
        (loop $spin (br $spin)))
)"#;

#[test]
fn an_interrupted_plugin_stops_on_every_thread() {
    let host = host();
    let interrupter = host.interrupter();
    let (woken, wakes) = mpsc::channel();
    let (shared, threads) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut plugin = host.load_bytes(STUCK.as_bytes()).unwrap();
        shared
            .send(Arc::clone(&plugin.store.data().threads))
            .unwrap();
        plugin.on_wake(move || {
            let _ = woken.send(());
        });
        plugin.start().unwrap();
        plugin.step()
    });
    let threads = threads.recv().unwrap();
    for _ in 0..3 {
        wakes
            .recv()
            .expect("the plugin and both its threads should have started");
    }

    interrupter.interrupt();

    assert_eq!(worker.join().unwrap(), Err(INTERRUPTED.to_owned()));
    assert!(
        threads.await_ended(Duration::from_secs(30)),
        "a thread of the interrupted plugin is still running"
    );
}
