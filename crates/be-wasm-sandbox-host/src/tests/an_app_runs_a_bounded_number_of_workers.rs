use be_sdk_worker::Worker;

use super::*;

/// An app runs at most `WORKERS` workers at once; one that stopped makes room for another.
#[test]
fn an_app_runs_a_bounded_number_of_workers() {
    let mut pump = Pump::echo_app();
    let (refused, replaced) = pump.run(async {
        let mut workers = Vec::new();
        for _ in 0..WORKERS {
            let mut worker = Worker::spawn(be_sdk_worker_echo::MODULE);
            worker.receive().await.expect("the worker starts");
            workers.push(worker);
        }
        let refused = Worker::spawn(be_sdk_worker_echo::MODULE).receive().await;
        drop(workers.pop());
        let replaced = Worker::spawn(be_sdk_worker_echo::MODULE).receive().await;
        (refused, replaced)
    });
    assert_eq!(
        refused,
        Err(format!("be3.echo may run at most {WORKERS} workers at once"))
    );
    assert_eq!(replaced, Ok(be_sdk_worker_echo::STARTED.to_vec()));
}
