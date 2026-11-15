use super::*;

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.switcher",
            "name": "Switcher",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41d",
                "edits": "00000000-0000-4000-8000-0000000000d4",
                "display_name": "Switcher",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.switcher.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

fn switching_children(serial: u64) -> Heard {
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
        input_waiting: false,
        presented: None,
    })
}

fn frame(presents: u64) -> Frame {
    Frame {
        textures: Vec::new(),
        presents,
        damage: None,
    }
}

/// A frame that switches the children it places (a stacked dock's bar going from the files to a
/// document) is presented between polls, but its children change only at the poll. Shown at
/// once, the new bar would sit a frame over the old children; it is shown at that poll instead,
/// with them.
#[test]
fn a_frame_whose_children_wait_for_the_poll_is_shown_with_them() {
    let mut runtime = Runtime::new(&manifest(), 0);

    runtime.received(frame(0));
    assert_eq!(runtime.frames, 1, "a frame that brings no children is shown at once");

    runtime.hear(vec![switching_children(1), presented(1)]);
    runtime.received(frame(1));
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (1, 1),
        "between polls, its children wait, and so does the frame"
    );

    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (0, 2),
        "the poll applies the children and shows the frame with them"
    );

    runtime.received(frame(2));
    assert_eq!(runtime.frames, 3, "during a poll, a frame is shown at once");
}
