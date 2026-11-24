use be_wasm_root_api::{
    AccountRow, CoreCommand, CoreView, CoreWait, HostedBlock, Refusal, SetAside, UnsentEdit,
};

use super::*;
use crate::{DIALOG_LAYER, DIALOG_SURFACE, MENU_LAYER, MENU_SURFACE, STATISTICS_LAYER};

/// A company's organization the person administers, with an unsent edit, a block kept
/// hosting and an escrow key not to be trusted yet.
fn organization() -> CoreView {
    CoreView {
        organization: Some("Analytical Engines".to_owned()),
        accounts: vec![AccountRow {
            key: "https://example.com\u{0}ada".to_owned(),
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            server: Some("https://example.com".to_owned()),
            has_last_organization: true,
            current: true,
            organizations: vec!["Analytical Engines".to_owned()],
        }],
        company: true,
        administrator: true,
        escrow_warning: Some("Its fingerprint changed since this device last saw it.".to_owned()),
        hosting: vec![HostedBlock {
            block: [7; 16],
            name: "Board".to_owned(),
            now: true,
        }],
        unsent: vec![UnsentEdit {
            block: [9; 16],
            name: Some("Notes".to_owned()),
            offerable: false,
            aside: Some(SetAside::Refused(Refusal::OverQuota)),
            kept_edits: 0,
        }],
        ..CoreView::default()
    }
}

#[test]
fn a_root_opens_the_block_stacks_dialogs_from_its_menu() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    // While the block stack waits for an account, its menu does not show.
    show_core(
        &mut root,
        CoreView {
            waits_for: Some(CoreWait::Account),
            ..CoreView::default()
        },
    );
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
    // where it is: here a company's, which the person administers, with an unsent edit, a
    // block kept hosting and an escrow key not to be trusted yet, so it offers everything.
    show_core(&mut root, organization());
    let open = until(&mut root, &mut time, Vec::new(), "the menu shows", |done| {
        layer_of(done, MENU_LAYER).is_some()
    });
    let layer = layer_of(&open, MENU_LAYER).expect("the menu shows");
    assert!(
        matches!(layer.input.hit, be_lib_layers::Area::Region(_)),
        "the menu is hit only where it shows: {:?}",
        layer.input.hit
    );
    // Its surfaces fill the window, and follow it as it resizes, the menu's button at its
    // top right.
    let sized = |root: &mut Root, time: &mut u64| {
        let painting = painting_of(root, time, MENU_SURFACE);
        let own = own_paintings(root, *time)
            .into_iter()
            .find(|(surface, _)| *surface == u64::from(crate::ROOT_SURFACE))
            .map(|(_, painting)| painting.frame.size)
            .expect("the root paints its own surface");
        (painting.frame.size, own)
    };
    assert_eq!(sized(&mut root, &mut time), ([800, 600], [800, 600]));
    resize(1100.0, 720.0);
    assert_eq!(sized(&mut root, &mut time), ([1100, 720], [1100, 720]));
    resize(800.0, 600.0);
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
        chord(
            be_lib_ui_types::Key::Escape,
            be_lib_ui_types::Modifiers::NONE,
        ),
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
        chord(
            be_lib_ui_types::Key::Escape,
            be_lib_ui_types::Modifiers::NONE,
        ),
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
}
