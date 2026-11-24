use super::*;

/// From the frames a guest sends an instance (while it starts, steps on them, and holds its
/// answer) until the guest takes that answer, the instance is unsettled: a host waiting for its
/// guest to settle waits for it. Once the guest took everything, it is settled.
#[test]
fn an_instance_is_unsettled_until_the_guest_takes_what_it_answered() {
    let mut pump = Pump::new();
    let instance = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .unwrap();
        let instance = guest::sandbox::Instance::new(Source::Module(module), config());
        instance.send(vec![b"hello".to_vec()]).await.unwrap();
        instance
    });
    assert!(
        pump.server.unsettled().is_some(),
        "starting or stepping on what it was sent, or holding the echo"
    );
    let op = *pump.server.instances.keys().next().unwrap();
    while pump.server.instances[&op].received.len() < 2 {
        assert!(pump.server.unsettled().is_some(), "the echo is not taken yet");
        pump.woken
            .recv_timeout(Duration::from_secs(60))
            .expect("the instance echoes in time");
        pump.server.poll(&mut pump.host);
    }
    assert_eq!(
        pump.server.unsettled(),
        Some("an instance sent frames the guest has yet to take")
    );

    let received = pump.run(async move {
        let received = instance.next().await.unwrap().unwrap();
        (received, instance)
    });
    assert_eq!(
        received.0,
        vec![be_wasm_guest_echo::STARTED.to_vec(), b"hello".to_vec()]
    );
    assert!(pump.server.take_answered(), "the guest was answered");
    assert_eq!(pump.server.unsettled(), None);
}
