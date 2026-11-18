use be_sdk_worker::Worker;
use be_sdk_worker_echo::{ECHOED, MODULE, STARTED};

use super::*;

#[test]
fn an_app_spawns_its_own_worker_and_exchanges_messages_with_it() {
    let mut pump = Pump::echo_app();
    let received = pump.run(async {
        let mut worker = Worker::spawn(MODULE);
        worker.send(b"one".to_vec());
        worker.sender().send(b"two".to_vec());
        let mut received = Vec::new();
        for _ in 0..3 {
            received.push(worker.receive().await.expect("the worker runs"));
        }
        let page = vec![7u8; 4 << 20];
        worker.send(page.clone());
        let echoed = worker.receive().await.expect("the worker runs");
        assert_eq!(
            echoed,
            [ECHOED, &page].concat(),
            "a large buffer arrives whole"
        );
        received
    });
    assert_eq!(
        received,
        vec![
            STARTED.to_vec(),
            [ECHOED, b"one"].concat(),
            [ECHOED, b"two"].concat(),
        ],
        "the worker started, then answered each message in the order it was sent"
    );
}
