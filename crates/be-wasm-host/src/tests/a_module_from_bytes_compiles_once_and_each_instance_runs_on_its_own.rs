use std::{sync::Arc, time::Duration};

use super::*;

#[test]
fn a_module_from_bytes_compiles_once_and_each_instance_runs_on_its_own() {
    let wake: be_wasm_guest_host::Wake = Arc::new(|| {});
    let compile = || {
        sandbox::compile(ECHO.to_vec(), Arc::clone(&wake))
            .wait(Duration::from_secs(60))
            .expect("the compile finishes")
            .expect("the echo guest is a pure guest's module")
    };
    let first = compile();
    let second = compile();
    assert_eq!(first.id(), be_wasm_dylink::sha256(ECHO));
    assert!(
        Arc::ptr_eq(&first.compiled, &second.compiled),
        "the same bytes compile once"
    );

    let config = |name: &str| sandbox::Config {
        name: name.to_owned(),
        memory_limit: 16 << 20,
    };
    let mut one = sandbox::spawn(sandbox::Source::Module(first.clone()), config("one"), Arc::clone(&wake)).unwrap();
    let mut two = sandbox::spawn(sandbox::Source::Module(second.clone()), config("two"), Arc::clone(&wake)).unwrap();
    one.send(vec![b"to one".to_vec()]).unwrap();
    two.send(vec![b"to two".to_vec()]).unwrap();
    for (instance, sent) in [(&mut one, b"to one"), (&mut two, b"to two")] {
        let mut received = Vec::new();
        while received.len() < 2 {
            let frames = instance
                .wait(be_wasm_guest_host::Deadline::after(Duration::from_secs(60)))
                .unwrap();
            assert!(!frames.is_empty(), "the instance answers in time");
            received.extend(frames);
        }
        assert_eq!(
            received,
            vec![be_wasm_guest_echo::STARTED.to_vec(), sent.to_vec()]
        );
    }
}
