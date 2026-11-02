use super::*;

#[test]
fn a_failed_app_session_is_reported_as_a_crash() {
    let app = be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.failing",
            "name": "Failing",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e417",
                "edits": "00000000-0000-4000-8000-0000000000c3",
                "display_name": "Failing",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.failing.wasm"
        }"#,
    )
    .expect("the manifest is valid");
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(app.identity.id.clone(), Runtime::new(&app, 0));
    });
    let _ = take_stopped();

    with("test.failing", |runtime| {
        runtime.session.shut_down();
        runtime.pump();
    });

    assert_eq!(
        take_stopped(),
        ["test.failing"],
        "the host hears that it stopped, as it does of a panic"
    );
    assert!(
        be_host_base::notices::shown()
            .iter()
            .any(|notice| notice.message.starts_with("Failing stopped: ")),
        "and the person is told"
    );
    assert!(take_stopped().is_empty(), "each stop is heard once");

    crash("test.failing", "Crashed again".to_owned());
    assert!(
        take_stopped().is_empty(),
        "an app that has already stopped does not stop again"
    );
}
