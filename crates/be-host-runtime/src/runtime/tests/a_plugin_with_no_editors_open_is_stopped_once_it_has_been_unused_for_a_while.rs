use super::*;

fn manifest(id: &str, block_type: &str) -> PluginManifest {
    be_sdk_manifest::manifest_from_json(&format!(
        r#"{{
            "id": "{id}",
            "name": "{id}",
            "version": "0.1.0",
            "editors": {{"main": {{
                "id": "00000000-0000-4000-8000-00000000e417",
                "edits": "{block_type}",
                "display_name": "{id}",
                "icon": "x",
                "views": ["Main"]
            }}}},
            "entry_point": "{id}.wasm"
        }}"#
    ))
    .expect("the manifest is valid")
}

fn running() -> Vec<String> {
    HOST.with(|host| {
        let mut running: Vec<String> = host.borrow().runtimes.keys().cloned().collect();
        running.sort();
        running
    })
}

fn stop_idle(now: Duration) -> Option<Duration> {
    HOST.with(|host| host.borrow_mut().stop_idle(now))
}

#[test]
fn a_plugin_with_no_editors_open_is_stopped_once_it_has_been_unused_for_a_while() {
    let open = manifest("test.open", "00000000-0000-4000-8000-0000000000d1");
    let unused = manifest("test.unused", "00000000-0000-4000-8000-0000000000d2");
    let editor = EditorInstanceId(7);
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        for (surface, plugin) in [&open, &unused].into_iter().enumerate() {
            let runtime = Runtime::new(plugin, surface as u32);
            host.runtimes.insert(plugin.identity.id.clone(), runtime);
        }
    });
    with("test.open", |runtime| {
        runtime.instances.mount(
            editor,
            View::MAIN,
            Uuid::nil(),
            InstanceRole::Picker(Uuid::nil()),
            &Arc::default(),
        );
    });
    let start = Duration::from_secs(5);

    assert_eq!(
        stop_idle(start),
        Some(IDLE_TIMEOUT),
        "the host wakes when the unused plugin is due to stop"
    );
    assert_eq!(
        stop_idle(start + IDLE_TIMEOUT - Duration::from_secs(1)),
        Some(Duration::from_secs(1))
    );
    assert_eq!(running(), ["test.open", "test.unused"]);

    assert_eq!(stop_idle(start + IDLE_TIMEOUT), None);
    assert_eq!(
        running(),
        ["test.open"],
        "the unused plugin is stopped and the one with an editor open keeps running"
    );

    let closed = start + IDLE_TIMEOUT * 3;
    close("test.open", editor);
    assert_eq!(
        stop_idle(closed),
        Some(IDLE_TIMEOUT),
        "a plugin is unused from when its last editor closes"
    );
    assert_eq!(stop_idle(closed + IDLE_TIMEOUT), None);
    assert_eq!(running(), Vec::<String>::new());
}
