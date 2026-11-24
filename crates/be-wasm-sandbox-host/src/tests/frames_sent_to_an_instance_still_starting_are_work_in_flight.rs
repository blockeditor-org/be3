use super::*;

/// Frames sent to an instance that has yet to say it started wait for it to start: work in
/// flight, though the worker leaves saying so to whoever starts it.
#[test]
fn frames_sent_to_an_instance_still_starting_are_work_in_flight() {
    let mut instance = be_wasm_host::sandbox::spawn(
        be_wasm_host::sandbox::Source::App(
            staged(be_wasm_guest_echo::APP_ID, ECHO_ENTRY).expect("the echo app is staged"),
        ),
        be_wasm_host::sandbox::Config {
            name: "echo app".to_owned(),
            memory_limit: 1 << 30,
            display: None,
        },
        Arc::new(|| {}),
    )
    .expect("the echo app starts");
    // Not polled, so it has not heard the instance start.
    instance.send(vec![b"hello".to_vec()]).expect("it takes frames");
    assert!(
        !instance.settled(),
        "frames waiting for the instance to start are not settled"
    );
}
