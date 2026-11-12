use super::*;
use be_sdk_api::{DockChild, DockChildClose, DockTabPath};

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

fn dock_message() -> DockMessage {
    DockMessage::Close(DockChildClose {
        child: DockChild {
            instance: EditorInstanceId(1),
            child: be_sdk_api::ChildId(1),
        },
        tab: DockTabPath {
            via: vec![be_sdk_api::ChildId(1)],
            tab: 2,
        },
    })
}

fn laid_out(serial: u64, children: bool) -> Heard {
    Heard::LaidOut(Box::new(LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: match children {
            true => vec![ChildPlacements {
                instance: EditorInstanceId(1),
                view: ViewId::MAIN,
                children: Vec::new(),
                occluders: Vec::new(),
            }],
            false => Vec::new(),
        },
        window_requests: Vec::new(),
        docks: vec![dock_message()],
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

fn held_docks(runtime: &Runtime) -> usize {
    runtime
        .held
        .iter()
        .map(|held| match held {
            Answer::Laid(laid) => laid.docks.len(),
            _ => 0,
        })
        .sum()
}

/// A root's dock messages say which of a joined dock's tabs it shows, and the host draws only the
/// tab bodies it places that are shown: applied ahead of the children they came with, a tab
/// switched away from would stop drawing before the one switched to is placed, leaving its space
/// empty for a frame.
#[test]
fn a_frames_dock_messages_wait_with_the_children_it_places() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.taking_in = true;

    runtime.hear(vec![laid_out(1, false), presented(1)]);
    assert_eq!(
        held_docks(&runtime),
        0,
        "with no child coming or going, the dock messages apply at once"
    );

    runtime.hear(vec![laid_out(2, true)]);
    assert_eq!(
        held_docks(&runtime),
        1,
        "with a child coming or going, they wait with it"
    );
    runtime.hear(vec![laid_out(3, false)]);
    assert_eq!(
        held_docks(&runtime),
        2,
        "and the next frame's wait behind them, in order"
    );

    runtime.hear(vec![presented(2), presented(3)]);
    assert!(
        runtime.held.is_empty(),
        "with the frames that place the children, they apply"
    );
}
