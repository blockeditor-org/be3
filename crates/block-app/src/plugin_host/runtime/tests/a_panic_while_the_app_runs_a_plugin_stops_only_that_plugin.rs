use super::*;

fn manifest(id: &str, name: &str, block_type: &str) -> PluginManifest {
    block_plugin_manifest::manifest_from_json(&format!(
        r#"{{
            "id": "{id}",
            "name": "{name}",
            "version": "0.1.0",
            "editors": {{"main": {{
                "id": "00000000-0000-4000-8000-00000000e417",
                "edits": "{block_type}",
                "display_name": "{name}",
                "icon": "x",
                "regions": ["Frame"]
            }}}},
            "entry_point": "{id}.wasm"
        }}"#
    ))
    .expect("the manifest is valid")
}

#[test]
fn a_panic_while_the_app_runs_a_plugin_stops_only_that_plugin() {
    let crashing = manifest(
        "test.crashing",
        "Crashing",
        "00000000-0000-4000-8000-0000000000c1",
    );
    let steady = manifest(
        "test.steady",
        "Steady",
        "00000000-0000-4000-8000-0000000000c2",
    );
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability(Ok(()));
        for (surface, plugin) in [&crashing, &steady].into_iter().enumerate() {
            let runtime = Runtime::new(plugin, surface as u32);
            host.runtimes.insert(plugin.identity.id.clone(), runtime);
        }
    });

    let ran = with("test.crashing", |_| -> u32 {
        panic!("the host lost track of a screen")
    });

    assert_eq!(ran, None, "a panic leaves nothing to return");
    let error = with("test.crashing", |runtime| runtime.error.clone()).flatten();
    assert_eq!(
        error.as_deref(),
        Some("The app failed while running it: the host lost track of a screen")
    );
    assert_eq!(
        with("test.steady", |runtime| runtime.error.clone()),
        Some(None),
        "the other plugin keeps running"
    );
    let toast = crate::notices::shown()
        .into_iter()
        .find(|toast| {
            toast.message
                == "Crashing stopped: The app failed while running it: the host lost track of a screen"
        })
        .expect("the crash is reported as a toast");
    assert!(
        toast.danger
            && toast
                .actions
                .iter()
                .any(|action| action.key == crate::notices::COPY_REPORT),
        "the toast is an error, which stays until dismissed, and offers its report to copy"
    );
    assert_eq!(
        crate::notices::report_of(toast.id).as_deref(),
        Some("The app failed while running it: the host lost track of a screen"),
        "the report is the plugin's whole error"
    );
    let view = region_view(
        &crashing,
        EditorInstanceId(1),
        EditorRegion::Frame,
        None,
        None,
    );
    assert_eq!(
        view.error,
        Some((
            "The app failed while running it: the host lost track of a screen".to_owned(),
            true
        )),
        "its editors show it stopped, with a way to reload it"
    );

    with("test.crashing", Runtime::restart);

    assert_eq!(
        with("test.crashing", |runtime| runtime.error.clone()),
        Some(None),
        "reloading it starts it again"
    );
}
