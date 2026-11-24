use crate::{DIALOG_LAYER, DIALOG_SURFACE};
use be_lib_geometry::pos2;
use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_wasm_root_api::{
    ContactCard, ContactMismatch, ContactRow, ContactsArea, CoreCommand, CoreSecret, CoreView,
    Leaving, SecretCommand, ShareAccess, ShareLinksArea, SharePreview,
};

use super::*;

const NOTES: [u8; 16] = [5; 16];
const GRACE: [u8; 16] = [2; 16];

/// Hands the root `events` and draws a few frames, answering what it asked of the block stack.
fn asks(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
) -> Vec<CoreCommand> {
    if !events.is_empty() {
        root.input(None, be_lib_ui_types::input::encode(&events))
            .expect("the root takes input");
    }
    for _ in 0..4 {
        *time += 1;
        step(root, *time);
    }
    root.take_core_commands()
}

fn key(key: Key) -> Vec<be_lib_ui_types::input::Timed> {
    chord(key, Modifiers::NONE)
}

fn typed(text: &str) -> Vec<be_lib_ui_types::input::Timed> {
    [ImeEvent::Enabled, ImeEvent::CommitText(text.to_owned())]
        .into_iter()
        .map(|event| Input::Ime(event).into())
        .collect()
}

/// What the root paints its dialog with, a few frames from `*time` on.
fn painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    let _ = asks(root, time, Vec::new());
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its dialog")
}

fn shows_dialog(root: &mut Root, time: &mut u64) -> bool {
    *time += 1;
    layer_of(&step(root, *time), DIALOG_LAYER).is_some()
}

fn leaving(busy: bool) -> CoreView {
    CoreView {
        leaving: Some(Leaving {
            block: NOTES,
            name: "Notes".to_owned(),
            within: Some("Projects".to_owned()),
            busy,
            refused: None,
        }),
        ..CoreView::default()
    }
}

fn contacts(offered: Option<ContactCard>) -> CoreView {
    CoreView {
        contacts: ContactsArea {
            contacts: vec![
                ContactRow {
                    account: GRACE,
                    name: "Grace".to_owned(),
                    email: "grace@example.com".to_owned(),
                    verified: true,
                    mismatch: None,
                },
                ContactRow {
                    account: [3; 16],
                    name: "Lin".to_owned(),
                    email: String::new(),
                    verified: true,
                    mismatch: Some(ContactMismatch::Older),
                },
            ],
            offered,
            connected: None,
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

fn shared(offered: bool) -> CoreView {
    let preview = SharePreview {
        name: "Garden plan".to_owned(),
        organization: "Looms".to_owned(),
        access: ShareAccess::Edit,
        guest: true,
    };
    CoreView {
        share_links: ShareLinksArea {
            offered: offered.then(|| preview.clone()),
            opened: (!offered).then_some(preview),
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

#[test]
fn a_root_asks_before_leaving_a_block_and_opens_its_hosts_contact_and_share_links() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // The host asks before leaving a block an editor asked to leave.
    show_core(&mut root, leaving(false));
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_leaving_a_block",
        painting(&mut root, &mut time),
    );
    let _ = asks(&mut root, &mut time, key(Key::Tab));
    let commands = asks(&mut root, &mut time, key(Key::Enter));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ConfirmLeave(NOTES)]),
        "Leave leaves it: {commands:?}"
    );
    show_core(&mut root, leaving(true));
    assert!(shows_dialog(&mut root, &mut time), "it stays while leaving");
    show_core(&mut root, CoreView::default());
    let commands = asks(&mut root, &mut time, Vec::new());
    assert!(
        commands.is_empty(),
        "left, it goes asking nothing more: {commands:?}"
    );
    assert!(!shows_dialog(&mut root, &mut time));
    show_core(&mut root, leaving(false));
    let _ = asks(&mut root, &mut time, Vec::new());
    let commands = asks(&mut root, &mut time, key(Key::Escape));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelLeave]),
        "Escape stays: {commands:?}"
    );
    show_core(&mut root, CoreView::default());
    let _ = asks(&mut root, &mut time, Vec::new());

    // F12 opens the contacts afresh; New link makes a contact link, which comes back once.
    let commands = menu(&mut root, &mut time, Entry::Contacts);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ForgetContactLink]),
        "opening them starts afresh: {commands:?}"
    );
    show_core(&mut root, contacts(None));
    let listed = painting(&mut root, &mut time);
    beui_paint_snapshot::assert_painting("a_root_lists_its_hosts_contacts", listed.clone());
    // While it is being made the contacts stay open: Escape waits, so the link is never left
    // live and unheld.
    root.input(None, be_lib_ui_types::input::encode(&key(Key::Enter)))
        .expect("the root takes input");
    root.input(None, be_lib_ui_types::input::encode(&key(Key::Escape)))
        .expect("the root takes input");
    let _ = asks(&mut root, &mut time, Vec::new());
    assert!(
        shows_dialog(&mut root, &mut time),
        "the contacts stay while the link is made"
    );
    let requests = root.take_secret_requests();
    let [(id, SecretCommand::ContactLink)] = requests.as_slice() else {
        panic!("New link asks for one, Escape cancelling nothing: {requests:?}");
    };
    root.answer_secret(
        *id,
        Some(CoreSecret::Link(Password::from(
            "https://example.com/contact#c0de".to_owned(),
        ))),
    );
    assert_ne!(painting(&mut root, &mut time), listed, "the link shows");
    let _ = asks(&mut root, &mut time, key(Key::Escape));

    // A contact link opened: who it is from, to connect with.
    let _ = menu(&mut root, &mut time, Entry::Contacts);
    show_core(
        &mut root,
        contacts(Some(ContactCard {
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
        })),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_asks_to_connect_with_a_contact",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, click(pos2(566.0, 248.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::AcceptContact]),
        "Connect connects: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(477.0, 248.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ForgetContactLink]),
        "Cancel puts it aside: {commands:?}"
    );

    // Removing a contact asks first.
    show_core(&mut root, contacts(None));
    let _ = painting(&mut root, &mut time);
    let commands = asks(&mut root, &mut time, click(pos2(593.0, 270.0)));
    assert!(commands.is_empty(), "asking removes no one: {commands:?}");
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_removing_a_contact",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, click(pos2(566.0, 340.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::RemoveContact(GRACE)]),
        "Remove removes them: {commands:?}"
    );
    let _ = asks(&mut root, &mut time, key(Key::Escape));

    // F8 opens a share link: the pasted link crosses as typed, inside the command alone.
    let commands = menu(&mut root, &mut time, Entry::ShareLink);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ForgetShareLink]),
        "opening it starts afresh: {commands:?}"
    );
    let _ = asks(
        &mut root,
        &mut time,
        typed("https://example.com/share#s3cret"),
    );
    let commands = asks(&mut root, &mut time, key(Key::Enter));
    match commands.as_slice() {
        [CoreCommand::OpenShareLink(link)] => {
            assert_eq!(link.as_bytes(), b"https://example.com/share#s3cret");
        }
        other => panic!("Open opens the link: {other:?}"),
    }
    show_core(&mut root, shared(true));
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_opening_a_share_link",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, key(Key::Enter));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::AcceptShareLink]),
        "Open it opens what it shares: {commands:?}"
    );
    show_core(&mut root, shared(false));
    beui_paint_snapshot::assert_painting(
        "a_root_shows_what_a_share_link_opened",
        painting(&mut root, &mut time),
    );
    let _ = asks(&mut root, &mut time, key(Key::Escape));
    assert!(!shows_dialog(&mut root, &mut time));
}
