use super::*;

#[test]
fn a_guest_compiles_a_module_and_talks_to_an_instance_of_it() {
    let mut pump = Pump::new();
    let received = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .expect("the echo guest compiles");
        let instance = guest::sandbox::Instance::new(
            Source::Module(FromBytes {
                module: module,
                seed: [0; 32],
            }),
            config(),
        );
        instance.send(vec![b"hello".to_vec()]).await.unwrap();
        let mut received = Vec::new();
        while received.len() < 2 {
            received.extend(instance.next().await.unwrap().unwrap());
        }
        received
    });
    assert_eq!(
        received,
        vec![be_wasm_guest_echo::STARTED.to_vec(), b"hello".to_vec()]
    );
}
