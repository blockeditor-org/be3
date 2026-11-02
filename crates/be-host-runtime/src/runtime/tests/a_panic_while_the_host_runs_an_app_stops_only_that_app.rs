use super::*;

fn manifest(id: &str, name: &str, block_type: &str) -> AppManifest {
    be_sdk_manifest::manifest_from_json(&format!(
        r#"{{
            "id": "{id}",
            "name": "{name}",
            "version": "0.1.0",
            "editors": {{"main": {{
                "id": "00000000-0000-4000-8000-00000000e417",
                "edits": "{block_type}",
                "display_name": "{name}",
                "icon": "x",
                "views": ["Main"]
            }}}},
            "entry_point": "{id}.wasm"
        }}"#
    ))
    .expect("the manifest is valid")
}

#[test]
fn a_panic_while_the_host_runs_an_app_stops_only_that_app() {
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
        for (surface, app) in [&crashing, &steady].into_iter().enumerate() {
            let runtime = Runtime::new(app, surface as u32);
            host.runtimes.insert(app.identity.id.clone(), runtime);
        }
    });

    let ran = with("test.crashing", |_| -> u32 {
        panic!("the host lost track of a screen")
    });

    assert_eq!(ran, None, "a panic leaves nothing to return");
    let error = with("test.crashing", |runtime| runtime.error.clone()).flatten();
    assert_eq!(
        error.as_deref(),
        Some("be-host failed while running it: the host lost track of a screen")
    );
    assert_eq!(
        with("test.steady", |runtime| runtime.error.clone()),
        Some(None),
        "the other app keeps running"
    );
    let notice = be_host_base::notices::shown()
        .into_iter()
        .find(|notice| {
            notice.message
                == "Crashing stopped: be-host failed while running it: the host lost track of a screen"
        })
        .expect("the crash is reported as a notice");
    assert!(
        notice.report.is_some(),
        "the notice offers its report to copy"
    );
    assert_eq!(
        be_host_base::notices::report_of(notice.id).as_deref(),
        Some("be-host failed while running it: the host lost track of a screen"),
        "the report is the app's whole error"
    );
    let view = placed_view(&crashing, EditorInstanceId(1), ViewId::MAIN, None, None);
    assert_eq!(
        view.error,
        Some((
            "be-host failed while running it: the host lost track of a screen".to_owned(),
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
