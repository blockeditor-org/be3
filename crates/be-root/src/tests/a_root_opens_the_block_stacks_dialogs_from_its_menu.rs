use be_wasm_root_api::{CoreCommand, CoreView, CoreWait};

use super::*;
use crate::{DIALOG_LAYER, DIALOG_SURFACE, MENU_LAYER, STATISTICS_LAYER};

#[test]
fn a_root_opens_the_block_stacks_dialogs_from_its_menu() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    // While the block stack waits for an account, its menu does not show.
    root.show_core(CoreView {
        waits_for: Some(CoreWait::Account),
        ..CoreView::default()
    });
    let waiting = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the block stack's onboarding shows",
        |done| layer_of(done, crate::ONBOARDING_LAYER).is_some(),
    );
    assert!(
        layer_of(&waiting, MENU_LAYER).is_none(),
        "no menu while nothing is open: {:?}",
        shown(&waiting)
    );

    // Once it has an organization open, its button shows at the window's top right, hit only
    // where it is.
    root.show_core(CoreView::default());
    let open = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the menu shows",
        |done| layer_of(done, MENU_LAYER).is_some(),
    );
    let layer = layer_of(&open, MENU_LAYER).expect("the menu shows");
    assert!(
        matches!(layer.input.hit, be_lib_layers::Area::Region(_)),
        "the menu is hit only where it shows: {:?}",
        layer.input.hit
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_menu_button",
        menu_painting(&mut root, &mut time),
    );

    // Its button opens it.
    hand(&mut root, &mut time, click(menu_button()));
    beui_paint_snapshot::assert_painting(
        "a_root_opens_its_menu",
        menu_painting(&mut root, &mut time),
    );
    hand(
        &mut root,
        &mut time,
        chord(be_lib_ui_types::Key::Escape, be_lib_ui_types::Modifiers::NONE),
    );

    // Its entries open the block stack's dialogs, or ask the block stack.
    let _ = menu(&mut root, &mut time, Entry::Unsent);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "Unsent edits opens its dialog",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    let _ = painting_of(&mut root, &mut time, DIALOG_SURFACE);
    hand(
        &mut root,
        &mut time,
        chord(be_lib_ui_types::Key::Escape, be_lib_ui_types::Modifiers::NONE),
    );
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "Escape closes it",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    let commands = menu(&mut root, &mut time, Entry::SwitchOrganization);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::SwitchOrganization]),
        "Switch organization asks the block stack: {commands:?}"
    );
    for (entry, asked) in [
        (Entry::Sessions, "Sessions and profiles"),
        (Entry::NewRecoveryPhrase, "New recovery phrase"),
        (Entry::ManageAccounts, "Manage accounts"),
    ] {
        let commands = menu(&mut root, &mut time, entry);
        let right = match entry {
            Entry::Sessions => matches!(commands.as_slice(), [CoreCommand::ChooseSession]),
            Entry::NewRecoveryPhrase => {
                matches!(commands.as_slice(), [CoreCommand::NewRecoveryPhrase])
            }
            _ => matches!(commands.as_slice(), [CoreCommand::SwitchAccount]),
        };
        assert!(right, "{asked} asks the block stack: {commands:?}");
    }

    // Performance opens the statistics panel and closes it again.
    let _ = menu(&mut root, &mut time, Entry::Performance);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "Performance opens the statistics panel",
        |done| layer_of(done, STATISTICS_LAYER).is_some(),
    );
    let _ = menu(&mut root, &mut time, Entry::Performance);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "Performance again closes it",
        |done| layer_of(done, STATISTICS_LAYER).is_none(),
    );

    let _ = menu(&mut root, &mut time, Entry::Lock);
    assert!(root.take_lock_asked(), "Lock locks the session");
}
