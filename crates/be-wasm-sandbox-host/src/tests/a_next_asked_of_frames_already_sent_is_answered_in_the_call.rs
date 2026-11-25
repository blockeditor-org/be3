use super::*;

/// Frames an instance sent while its guest was not waiting are held for it; the guest's next
/// `next` is answered with them as the server takes the call, which says so, since the guest
/// takes the answer in only once its host steps it again.
#[test]
fn a_next_asked_of_frames_already_sent_is_answered_in_the_call() {
    let mut pump = Pump::new();
    let instance = pump.run(async {
        let module = guest::sandbox::compile(ECHO.to_vec())
            .await
            .unwrap()
            .expect("the echo guest compiles");
        let instance = guest::sandbox::Instance::new(
            Source::Module(FromBytes {
                module,
                seed: [0; 32],
            }),
            config(),
        );
        instance.send(vec![b"hello".to_vec()]).await.unwrap();
        instance
    });
    // The echo answers while the guest asks nothing: what it sent is held for the guest.
    while !pump
        .server
        .instances
        .values()
        .any(|served| served.received.len() == 2)
    {
        pump.woken
            .recv_timeout(Duration::from_secs(60))
            .expect("the echo answers in time");
        assert!(
            !pump.server.poll(&mut pump.host),
            "nothing is answered while the guest asks nothing"
        );
    }
    pump.answered_in_call = false;
    let received = pump.run(async move { instance.next().await.unwrap().unwrap() });
    assert_eq!(
        received,
        vec![be_wasm_guest_echo::STARTED.to_vec(), b"hello".to_vec()]
    );
    assert!(
        pump.answered_in_call,
        "the call that asked was answered as the server took it"
    );
}
