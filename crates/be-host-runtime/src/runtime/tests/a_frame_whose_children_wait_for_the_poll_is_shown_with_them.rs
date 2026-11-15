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

fn placing(serial: u64, instances: &[u64]) -> Heard {
    Heard::LaidOut(Box::new(LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: instances
            .iter()
            .map(|instance| ChildPlacements {
                instance: EditorInstanceId(*instance),
                view: ViewId::MAIN,
                children: Vec::new(),
                occluders: Vec::new(),
            })
            .collect(),
        window_requests: Vec::new(),
        docks: Vec::new(),
    }))
}

fn presented(serial: u64, sequence: u64) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        input_waiting: false,
        presented: Some(PresentedFrame {
            sequence,
            damage: Vec::new(),
        }),
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

    runtime.hear(vec![presented(1, 1)]);
    runtime.received(frame(1));
    assert_eq!(runtime.frames, 1, "a frame that brings no children is shown at once");

    runtime.hear(vec![placing(2, &[1])]);
    runtime.received(frame(2));
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (1, 1),
        "between polls, the children wait, and so does the frame drawn after their layout, even \
         before its present is heard"
    );
    runtime.hear(vec![presented(2, 2)]);
    assert_eq!(runtime.frames, 1, "its present heard, it still waits for the poll");

    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (0, 2),
        "the poll applies the children and shows the frame with them"
    );

    runtime.received(frame(3));
    assert_eq!(runtime.frames, 3, "with nothing held, a frame is shown at once");

    runtime.taking_in = false;
    runtime.hear(vec![placing(3, &[1]), unpainted(3)]);
    runtime.received(frame(4));
    assert_eq!(runtime.frames, 3, "a present that painted nothing still holds the next frame");
    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert_eq!(
        (runtime.held.len(), runtime.frames),
        (0, 4),
        "and the poll that applies the children shows it"
    );
}

fn unpainted(serial: u64) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        input_waiting: false,
        presented: None,
    })
}

/// Children that only move go with the painting that moves them. When a child comes or goes in
/// the same layout, that painting waits for the poll, so the moving ones wait with it.
#[test]
fn children_that_move_wait_with_a_child_that_comes_in_the_same_layout() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.instances.mount(
        EditorInstanceId(1),
        View::MAIN,
        Uuid::nil(),
        InstanceRole::Editor(
            Uuid::nil(),
            crate::EditorBlock {
                id: Uuid::nil(),
                block_type: Uuid::nil(),
                view_block: None,
            },
        ),
        &Arc::new(crate::AppCatalog::default()),
    );

    runtime.hear(vec![placing(1, &[1])]);
    assert!(runtime.held.is_empty(), "a child that only moves is applied at once");

    runtime.hear(vec![placing(2, &[1, 2])]);
    let held: Vec<u64> = runtime
        .held
        .iter()
        .flat_map(|held| match held {
            Answer::Laid(laid) => laid.children.iter().map(|placed| placed.instance.0).collect(),
            _ => Vec::new(),
        })
        .collect();
    assert_eq!(held, vec![1, 2], "beside a child that comes, it waits with it");
}

/// A frame held for its children belongs to the session that drew it: a stop or restart drops
/// it, so the next session never shows it.
#[test]
fn a_frame_held_for_its_children_is_dropped_when_the_app_stops_or_restarts() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.hear(vec![placing(1, &[1])]);
    runtime.received(frame(1));
    assert!(runtime.held_frame.is_some(), "the frame waits for its children");
    runtime.restart();
    assert!(runtime.held_frame.is_none(), "a restart drops it");

    runtime.hear(vec![placing(1, &[1])]);
    runtime.received(frame(1));
    assert!(runtime.held_frame.is_some(), "the frame waits for its children");
    runtime.stopped("stopped".to_owned());
    assert!(runtime.held_frame.is_none(), "a stop drops it");
}
