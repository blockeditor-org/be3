use super::*;

#[test]
fn a_failed_plugin_session_is_reported_as_a_crash() {
    let plugin = block_plugin_manifest::manifest_from_json(
        r#"{
            "id": "test.failing",
            "name": "Failing",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e417",
                "edits": "00000000-0000-4000-8000-0000000000c3",
                "display_name": "Failing",
                "icon": "x",
                "regions": ["Frame"]
            }},
            "entry_point": "test.failing.wasm"
        }"#,
    )
    .expect("the manifest is valid");
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(plugin.identity.id.clone(), Runtime::new(&plugin, 0));
    });
    let _ = take_crashed();

    with("test.failing", |runtime| {
        runtime.session.shut_down();
        runtime.pump();
    });

    assert_eq!(
        take_crashed(),
        ["test.failing"],
        "the host hears that it stopped, as it does of a panic"
    );
    assert!(
        crate::notices::shown()
            .iter()
            .any(|toast| toast.danger && toast.message.starts_with("Failing stopped: ")),
        "and the person is told"
    );
    assert!(take_crashed().is_empty(), "each stop is heard once");

    crash("test.failing", "Crashed again".to_owned());
    assert!(
        take_crashed().is_empty(),
        "a plugin that has already stopped does not stop again"
    );
}
