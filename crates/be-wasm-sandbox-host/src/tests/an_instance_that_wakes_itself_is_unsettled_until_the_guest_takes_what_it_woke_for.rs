use super::*;

/// An instance that wakes itself straight after a step on what it was sent finishes that work
/// in the step it woke for, so it stays unsettled through it, until the guest takes what that
/// step sent (`Worker::unsettled`'s `carrying`). One that kept waking itself so would stay
/// unsettled for as long as it did.
#[test]
fn an_instance_that_wakes_itself_is_unsettled_until_the_guest_takes_what_it_woke_for() {
    let mut pump = Pump::new();
    let instance = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
        let instance = guest::sandbox::Instance::new(Source::Module(module), config());
        instance
            .send(vec![be_wasm_guest_echo::WAKE.to_vec()])
            .await
            .unwrap();
        instance
    });
    let op = *pump.server.instances.keys().next().unwrap();
    let woke = |pump: &Pump| {
        pump.server.instances[&op]
            .received
            .iter()
            .any(|frame| frame == be_wasm_guest_echo::WOKEN)
    };
    while !woke(&pump) {
        assert!(
            pump.server.unsettled().is_some(),
            "it has yet to send what it woke for"
        );
        pump.woken
            .recv_timeout(Duration::from_secs(60))
            .expect("the instance wakes in time");
        pump.server.poll(&mut pump.host);
    }
    assert!(pump.server.unsettled().is_some(), "the guest has yet to take it");

    pump.run(async move {
        while !instance
            .next()
            .await
            .unwrap()
            .unwrap()
            .iter()
            .any(|frame| frame == be_wasm_guest_echo::WOKEN)
        {}
    });
    assert_eq!(pump.server.unsettled(), None);
}
