use super::*;

#[test]
fn killing_an_app_stops_it_as_any_stop_does() {
    let app = be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.killed",
            "name": "Killed",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e419",
                "edits": "00000000-0000-4000-8000-0000000000c3",
                "display_name": "Killed",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.killed.wasm"
        }"#,
    )
    .expect("the manifest is valid");
    let editor = EditorInstanceId(3);
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        host.runtimes
            .insert(app.identity.id.clone(), Runtime::new(&app, 0));
    });
    with("test.killed", |runtime| {
        runtime.instances.mount(
            editor,
            View::MAIN,
            Uuid::nil(),
            InstanceRole::Picker(Uuid::nil()),
            &Arc::default(),
        );
    });
    let _ = take_stopped();

    kill("test.killed");

    assert_eq!(
        take_stopped(),
        ["test.killed"],
        "the host hears that it stopped, as it does of a crash"
    );
    let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(300.0, 200.0));
    let placed = EmbedPlacement {
        rect,
        clip: rect,
        radius: 0.0,
        raster: 1.0,
    };
    let view = placed_view(&app, editor, ViewId::MAIN, Some(placed), None);
    assert!(
        view.error.is_some() && !view.loading,
        "its open view shows that it stopped, with Reload, rather than loading forever"
    );

    close("test.killed", editor);
    kill("test.killed");
    assert!(
        take_stopped().is_empty(),
        "each stop is heard once, and dropping a stopped app is not another stop"
    );
    assert!(
        HOST.with(|host| !host.borrow().runtimes.contains_key("test.killed")),
        "once nothing shows it, killing it drops it"
    );
}
