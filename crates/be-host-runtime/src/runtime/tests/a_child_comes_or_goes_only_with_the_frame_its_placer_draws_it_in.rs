use super::*;

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.placer",
            "name": "Placer",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41b",
                "edits": "00000000-0000-4000-8000-0000000000d2",
                "display_name": "Placer",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.placer.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn closing_a_child(serial: u64) -> Heard {
    Heard::LaidOut(Box::new(LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: vec![ChildPlacements {
            instance: EditorInstanceId(1),
            view: ViewId::MAIN,
            children: Vec::new(),
            occluders: Vec::new(),
        }],
        window_requests: Vec::new(),
        docks: Vec::new(),
    }))
}

fn presented(serial: u64) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        presented: None,
    })
}

#[test]
fn a_child_comes_or_goes_only_with_the_frame_its_placer_draws_it_in() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.taking_in = true;

    runtime.hear(vec![closing_a_child(1)]);
    assert_eq!(
        runtime.held.len(),
        1,
        "a child leaving waits while its placer's frame still has its hole"
    );
    runtime.hear(vec![Heard::FrameNeeded]);
    assert_eq!(
        runtime.held.len(),
        1,
        "it waits until that frame is presented"
    );

    runtime.hear(vec![presented(1)]);
    assert!(
        runtime.held.is_empty(),
        "with the frame that fills its space, the child goes"
    );

    runtime.hear(vec![closing_a_child(2), presented(2)]);
    assert!(
        runtime.held.is_empty(),
        "heard with its frame, it goes at once"
    );
}
