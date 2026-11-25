use super::*;

/// What its host fails an instance for (an app whose frames broke the app protocol, which
/// be-host stops toward its root): the guest hears it stopped, and why, from its `next`, and
/// is handed nothing the instance sent that it had not taken.
#[test]
fn an_instance_its_host_fails_stops_and_says_why() {
    let mut pump = Pump::new();
    let instance = pump.run(async {
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
        instance.send(vec![b"hello".to_vec()]).await.unwrap();
        let mut received = Vec::new();
        while received.len() < 2 {
            received.extend(instance.next().await.unwrap().unwrap());
        }
        instance.send(vec![b"untaken".to_vec()]).await.unwrap();
        instance
    });
    let op = *pump
        .server
        .instances
        .keys()
        .next()
        .expect("the guest spawned an instance");
    // The echo the guest has not asked for yet, held for it.
    while pump.server.instances[&op].received.is_empty() {
        pump.woken
            .recv_timeout(Duration::from_secs(60))
            .expect("the instance echoes in time");
        pump.server.poll(&mut pump.host);
    }

    pump.server.fail(op, "it broke the app protocol");

    let (stopped, again) = pump.run(async move {
        let stopped = instance.next().await.unwrap();
        instance.send(vec![b"unheard".to_vec()]).await.unwrap();
        (stopped, instance.next().await.unwrap())
    });
    let why = Err("it broke the app protocol".to_owned());
    assert_eq!(
        stopped, why,
        "the guest hears why, not what it had not taken"
    );
    assert_eq!(again, why, "what it sends after goes nowhere");
}
