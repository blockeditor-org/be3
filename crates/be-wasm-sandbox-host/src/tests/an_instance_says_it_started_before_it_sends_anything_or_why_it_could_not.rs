use super::*;

#[test]
fn an_instance_says_it_started_before_it_sends_anything_or_why_it_could_not() {
    let mut pump = Pump::new();
    let (started, first, refused) = pump.run(async {
        let instance = guest::sandbox::Instance::new(echo_app(), app_config());
        let started = instance.started().await.unwrap();
        let first = instance.next().await.unwrap().expect("the echo app runs");
        let nowhere = guest::sandbox::Instance::new(
            Source::App(StagedModule {
                app: "be3.nowhere".to_owned(),
                module: ECHO_ENTRY.to_owned(),
            }),
            app_config(),
        );
        (started, first, nowhere.started().await.unwrap())
    });
    assert_eq!(
        started,
        Ok(()),
        "the echo app's module loaded and it ran its start"
    );
    assert_eq!(
        first,
        vec![be_wasm_guest_echo::STARTED.to_vec()],
        "what it sent as it started comes after"
    );
    assert_eq!(
        refused,
        Err("no staged app has the id be3.nowhere".to_owned()),
        "one that could not start says why"
    );
}
