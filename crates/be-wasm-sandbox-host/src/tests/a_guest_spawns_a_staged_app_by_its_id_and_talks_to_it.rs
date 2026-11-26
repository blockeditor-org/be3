use super::*;

#[test]
fn a_guest_spawns_a_staged_app_by_its_id_and_talks_to_it() {
    let mut pump = Pump::new();
    let received = pump.run(async {
        let instance = guest::sandbox::Instance::new(echo_app(), app_config());
        instance
            .send(vec![b"hello".to_vec(), be_wasm_guest_echo::CLOCK.to_vec()])
            .await
            .unwrap();
        let mut received = Vec::new();
        while received.len() < 3 {
            received.extend(instance.next().await.unwrap().expect("the echo app runs"));
        }
        received
    });
    assert_eq!(
        received,
        vec![
            be_wasm_guest_echo::STARTED.to_vec(),
            b"hello".to_vec(),
            be_wasm_guest_echo::CLOCK_READ.to_vec(),
        ],
        "the app started, echoed, and read the clock through WASI"
    );
}
