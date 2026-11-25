use std::collections::HashMap;
use std::time::Instant;

use be_lib_ui_types::{Key, Modifiers};
use be_sdk_api::accesskit::{Node, NodeId, TreeUpdate};
use be_wasm_root_api::CoreView;

use super::*;

/// Draws a frame of the root for `time` with its host asking for its tree, and keeps what the
/// frame changed of it in `tree`, as its host's graft does.
fn describe(root: &mut Root, time: u64, tree: &mut HashMap<NodeId, Node>) {
    let done = root
        .frame(
            FrameRequest {
                time,
                wall_clock: 0,
                utc_offset: 0,
                window: window(),
                describe: true,
            },
            Instant::now() + PATIENT,
        )
        .expect("the root draws");
    let update: TreeUpdate = be_wasm_ops::decode(
        &done
            .accessibility
            .expect("the root answers its tree while asked for it"),
    )
    .expect("the tree is an AccessKit update");
    tree.extend(update.nodes);
}

/// Whether the tree has a node labelled `label`, reached from its window node.
fn names(tree: &HashMap<NodeId, Node>, label: &str) -> bool {
    let mut pending = vec![NodeId(0)];
    while let Some(id) = pending.pop() {
        let Some(node) = tree.get(&id) else {
            continue;
        };
        if node.label() == Some(label) {
            return true;
        }
        pending.extend(node.children().iter().copied());
    }
    false
}

#[test]
fn a_root_describes_its_surfaces_once_asked_and_those_it_opens_after() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    show_core(&mut root, CoreView::default());

    // Its palette opens while its host asks for no tree: the root answers none.
    let chord = chord(
        Key::P,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    );
    hand(&mut root, &mut time, chord);
    time += 1;
    assert!(
        step(&mut root, time).accessibility.is_none(),
        "no tree while its host asks for none"
    );

    // Asked for it then, the root answers its whole tree within a few frames: the palette open
    // since before is in it, by name.
    let mut tree = HashMap::new();
    for _ in 0..4 {
        time += 1;
        describe(&mut root, time, &mut tree);
    }
    assert!(
        names(&tree, "Command palette"),
        "the palette opened before the host asked is in the tree"
    );

    // A dialog opened after that is in it too, from what each frame changed.
    hand(&mut root, &mut time, chord_escape());
    let _ = menu(&mut root, &mut time, Entry::Contacts);
    for _ in 0..4 {
        time += 1;
        describe(&mut root, time, &mut tree);
    }
    assert!(
        names(&tree, "Contacts"),
        "the dialog opened after the host asked is in the tree"
    );
}

fn chord_escape() -> Vec<be_lib_ui_types::input::Timed> {
    chord(Key::Escape, Modifiers::NONE)
}
