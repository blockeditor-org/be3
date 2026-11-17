use super::*;

#[test]
fn an_instance_the_guest_drops_stops() {
    let mut pump = Pump::new();
    pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
        let instance = guest::sandbox::Instance::new(module, config());
        instance
            .send(vec![be_wasm_guest_echo::HANG.to_vec()])
            .await
            .unwrap();
        drop(instance);
        // A call made after the drop reaches the host after it.
        guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
    });
    assert!(
        pump.server.instances.is_empty(),
        "the hung instance was stopped"
    );
}
