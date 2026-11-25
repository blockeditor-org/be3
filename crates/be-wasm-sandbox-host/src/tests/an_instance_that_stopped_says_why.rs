use super::*;

#[test]
fn an_instance_that_stopped_says_why() {
    let mut pump = Pump::new();
    let (panicked, unknown) = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
        let instance = guest::sandbox::Instance::new(
            Source::Module(FromBytes {
                module: module,
                seed: [0; 32],
            }),
            config(),
        );
        instance
            .send(vec![be_wasm_guest_echo::PANIC.to_vec()])
            .await
            .unwrap();
        let panicked = loop {
            match instance.next().await.unwrap() {
                Ok(frames) => assert_eq!(frames, vec![be_wasm_guest_echo::STARTED.to_vec()]),
                Err(failure) => break failure,
            }
        };
        let never = guest::sandbox::Instance::new(
            Source::Module(FromBytes {
                module: [0; 32],
                seed: [0; 32],
            }),
            config(),
        );
        let unknown = never.next().await.unwrap();
        (panicked, unknown)
    });
    assert!(
        panicked.contains(be_wasm_guest_echo::PANIC_MESSAGE),
        "{panicked}"
    );
    assert_eq!(
        unknown,
        Err("this guest compiled no module with that id".to_owned())
    );
}
