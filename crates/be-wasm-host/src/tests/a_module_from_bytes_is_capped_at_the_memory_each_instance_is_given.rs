use std::{sync::Arc, time::Duration};

use super::*;

#[test]
fn a_module_from_bytes_is_capped_at_the_memory_each_instance_is_given() {
    let wake: be_wasm_guest_host::Wake = Arc::new(|| {});
    let module = sandbox::compile(ECHO.to_vec(), Arc::clone(&wake))
        .wait(Duration::from_secs(60))
        .expect("the compile finishes")
        .unwrap();
    let config = sandbox::Config {
        name: "capped".to_owned(),
        memory_limit: 64 << 20,
        display: None,
    };
    let mut instance =
        sandbox::spawn(sandbox::Source::Module(module.clone()), config, wake).unwrap();
    instance
        .send(vec![be_wasm_guest_echo::GROW.to_vec()])
        .unwrap();
    let deadline = be_wasm_guest_host::Deadline::after(Duration::from_secs(60));
    let failed = loop {
        match instance.wait(deadline) {
            Ok(frames) => assert!(
                !frames.is_empty()
                    && frames
                        .iter()
                        .all(|frame| frame == be_wasm_guest_echo::STARTED),
                "only the start is answered, in time: {frames:?}"
            ),
            Err(failure) => break failure,
        }
    };
    assert!(
        failed.contains("trapped") || failed.contains("memory"),
        "growing past the limit stops the instance: {failed}"
    );
}
