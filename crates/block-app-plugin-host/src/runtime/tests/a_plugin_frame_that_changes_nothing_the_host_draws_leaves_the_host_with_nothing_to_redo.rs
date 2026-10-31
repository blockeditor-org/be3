use super::*;
use block_plugin_api::SurfaceRect;

fn manifest() -> PluginManifest {
    block_plugin_manifest::manifest_from_json(
        r#"{
            "id": "test.still",
            "name": "Still",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41a",
                "edits": "00000000-0000-4000-8000-0000000000d2",
                "display_name": "Still",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.still.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn animating(serial: u64) -> Answer {
    Answer::Frame(FrameCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: Vec::new(),
        repaint_after_micros: Some(0),
        animating: true,
        presented: None,
        window_requests: Vec::new(),
        docks: Vec::new(),
    })
}

#[test]
fn a_plugin_frame_that_changes_nothing_the_host_draws_leaves_the_host_with_nothing_to_redo() {
    let mut runtime = Runtime::new(&manifest(), 0);
    take_changed();

    runtime.apply(vec![animating(1)]);
    runtime.apply(vec![animating(1)]);
    assert!(
        take_changed().is_empty(),
        "an animating plugin's frame that changes nothing makes the host owe no frame"
    );

    runtime.apply(vec![Answer::FrameNeeded]);
    assert_eq!(take_changed(), vec!["test.still".to_owned()]);

    assert!(!frame_changed(Some(&[])), "a present that damaged nothing");
    assert!(frame_changed(None), "a present whose damage is unknown");
    assert!(frame_changed(Some(&[ScreenDamage {
        screen: ScreenId(1),
        rect: SurfaceRect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
    }])));
}
