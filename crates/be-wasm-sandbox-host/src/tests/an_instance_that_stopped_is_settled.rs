use super::*;

/// An instance that stopped is settled, though what it sent was never taken: it works on
/// nothing more.
#[test]
fn an_instance_that_stopped_is_settled() {
    let mut pump = Pump::new();
    let _instance = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
        let instance = guest::sandbox::Instance::new(Source::Module(module), config());
        instance.send(vec![b"hello".to_vec()]).await.unwrap();
        instance
    });
    let op = *pump.server.instances.keys().next().unwrap();
    assert!(pump.server.unsettled().is_some());

    pump.server.fail(op, "it broke the app protocol");

    assert_eq!(pump.server.unsettled(), None);
}
