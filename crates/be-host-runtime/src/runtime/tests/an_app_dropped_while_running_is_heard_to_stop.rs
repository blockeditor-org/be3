use super::*;

#[test]
fn an_app_dropped_while_running_is_heard_to_stop() {
    let app = be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.dropped",
            "name": "Dropped",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41a",
                "edits": "00000000-0000-4000-8000-0000000000c3",
                "display_name": "Dropped",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.dropped.wasm"
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

    HOST.with(|host| host.borrow_mut().shutdown("test.dropped"));

    assert_eq!(take_stopped(), ["test.dropped"]);
}
