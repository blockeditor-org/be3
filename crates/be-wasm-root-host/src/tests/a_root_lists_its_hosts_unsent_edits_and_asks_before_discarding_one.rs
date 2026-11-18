use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, OPENS_UNSENT};
use be_wasm_root_api::{CoreCommand, CoreView, Refusal, SetAside, UnsentEdit};

use super::*;

const NOTES: [u8; 16] = [1; 16];
const PLAN: [u8; 16] = [2; 16];

/// The block stack's view with `unsent` set aside.
fn view(unsent: Vec<UnsentEdit>) -> CoreView {
    CoreView {
        generation: 0,
        unsent,
        choosing_account: false,
        accounts: Vec::new(),
        signing_in: None,
        account_error: None,
    }
}

/// A block whose edits went unsent when its access was lost, which may be edited again.
fn notes() -> UnsentEdit {
    UnsentEdit {
        block: NOTES,
        name: Some("Notes".to_owned()),
        offerable: true,
        aside: None,
        kept_edits: 2,
    }
}

/// A block, with no name, whose edits the server refused.
fn plan() -> UnsentEdit {
    UnsentEdit {
        block: PLAN,
        name: None,
        offerable: false,
        aside: Some(SetAside::Refused(Refusal::OverQuota)),
        kept_edits: 0,
    }
}

/// Presses `key` with no modifiers, drawing a few frames after.
fn press(root: &mut Root, time: &mut u64, key: Key) {
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(key, Modifiers::NONE)),
    )
    .expect("the root takes input");
    for _ in 0..3 {
        *time += 1;
        step(root, *time);
    }
}

/// What the root paints its dialog with, a few frames from `*time` on.
fn dialog_painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    for _ in 0..3 {
        *time += 1;
        step(root, *time);
    }
    own_paintings(root, *time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its dialog")
}

#[test]
fn a_root_lists_its_hosts_unsent_edits_and_asks_before_discarding_one() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    root.show_core(view(vec![notes(), plan()]));

    // F7 opens the list, over its scrim, taking the focus.
    until(
        &mut root,
        &mut time,
        chord(OPENS_UNSENT, Modifiers::NONE),
        "F7 opens the list of unsent edits",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_lists_its_hosts_unsent_edits",
        dialog_painting(&mut root, &mut time),
    );

    // Restore, the first of the first block's buttons, sends its edits again.
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Enter);
    assert!(
        matches!(
            root.take_core_commands().as_slice(),
            [CoreCommand::RestoreUnsent(NOTES)]
        ),
        "Restore asks the block stack to send the block's edits again"
    );

    // Discard asks first, discarding nothing until confirmed.
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Enter);
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_discarding_unsent_edits",
        dialog_painting(&mut root, &mut time),
    );
    assert!(
        root.take_core_commands().is_empty(),
        "asking discards nothing"
    );

    // Escape cancels the question, discarding nothing, and leaves the list open.
    press(&mut root, &mut time, Key::Escape);
    assert!(root.take_core_commands().is_empty(), "cancelled, nothing is discarded");
    assert!(
        layer_of(&step(&mut root, time), DIALOG_LAYER).is_some(),
        "the list stays open"
    );

    // Confirmed, the block's edits are discarded.
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Enter);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Enter);
    assert!(
        matches!(
            root.take_core_commands().as_slice(),
            [CoreCommand::DiscardUnsent(NOTES)]
        ),
        "confirming discards the block's edits"
    );

    // A block that leaves the list while its question is asked takes the question with it.
    root.show_core(view(vec![plan()]));
    let listed = dialog_painting(&mut root, &mut time);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Tab);
    press(&mut root, &mut time, Key::Enter);
    let asking = dialog_painting(&mut root, &mut time);
    assert_ne!(listed, asking, "Discard asks about the block left");
    root.show_core(view(Vec::new()));
    assert_ne!(
        dialog_painting(&mut root, &mut time),
        asking,
        "its block gone, the question goes"
    );
    press(&mut root, &mut time, Key::Enter);
    assert!(
        root.take_core_commands().is_empty(),
        "a question whose block left the list discards nothing"
    );

    // Escape closes the list.
    until(
        &mut root,
        &mut time,
        chord(Key::Escape, Modifiers::NONE),
        "Escape closes the list",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
}
