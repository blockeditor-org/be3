use crate::{ONBOARDING_LAYER, ONBOARDING_SURFACE};
use be_lib_geometry::pos2;
use be_lib_ui_types::input::{ImeEvent, Input};
use be_wasm_root_api::{
    AccountRow, CoreCommand, CoreView, CoreWait, Invitation, OrganizationRow, OrganizationsArea,
    OrganizationsListed, ProfileRow, SessionKind, SessionRow, SessionsArea,
};

use super::*;

const ENGINES: [u8; 16] = [1; 16];
const LOOMS: [u8; 16] = [2; 16];
const DESKTOP: [u8; 16] = [3; 16];
const NOTEBOOK: [u8; 16] = [4; 16];

/// The block stack's view while it waits for an organization.
fn organizations(listed: OrganizationsListed, joining: Option<Invitation>) -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Organization),
        accounts: vec![AccountRow {
            key: "https://example.com\u{0}ada".to_owned(),
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            server: Some("https://example.com".to_owned()),
            has_last_organization: true,
            current: true,
            organizations: Vec::new(),
        }],
        organizations: OrganizationsArea {
            listed,
            joining,
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

fn listed() -> OrganizationsListed {
    OrganizationsListed::Loaded(vec![
        OrganizationRow {
            id: ENGINES,
            name: "Analytical Engines".to_owned(),
        },
        OrganizationRow {
            id: LOOMS,
            name: "Looms".to_owned(),
        },
    ])
}

/// The block stack's view while it waits for a session.
fn sessions() -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Session),
        organization: Some("Analytical Engines".to_owned()),
        sessions: SessionsArea {
            loaded: true,
            sessions: vec![
                SessionRow {
                    id: [5; 16],
                    name: "Work".to_owned(),
                    session_type: DESKTOP,
                    current: true,
                },
                SessionRow {
                    id: [6; 16],
                    name: "Sketches".to_owned(),
                    session_type: NOTEBOOK,
                    current: false,
                },
            ],
            session_kinds: vec![
                SessionKind {
                    id: DESKTOP,
                    name: "desktop".to_owned(),
                },
                SessionKind {
                    id: NOTEBOOK,
                    name: "notebook".to_owned(),
                },
            ],
            session_type: DESKTOP,
            profiles: vec![ProfileRow {
                id: [7; 16],
                name: "Laptop".to_owned(),
                current: true,
            }],
        },
        ..CoreView::default()
    }
}

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

/// `text` committed through the input method into the focused field.
fn typed(text: &str) -> Vec<be_lib_ui_types::input::Timed> {
    [ImeEvent::Enabled, ImeEvent::CommitText(text.to_owned())]
        .into_iter()
        .map(|event| Input::Ime(event).into())
        .collect()
}

/// What the root paints its onboarding with, a few frames from `*time` on.
fn painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    for _ in 0..3 {
        *time += 1;
        step(root, *time);
    }
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(ONBOARDING_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints what the block stack waits for")
}

#[test]
fn a_root_shows_its_hosts_organizations_and_sessions() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // Waiting for an organization, while they are listed, then listed.
    show_core(&mut root, organizations(OrganizationsListed::Loading, None));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "while the block stack waits for an organization, the organizations show",
        |done| layer_of(done, ONBOARDING_LAYER).is_some(),
    );
    show_core(&mut root, organizations(listed(), None));
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_organizations",
        painting(&mut root, &mut time),
    );

    // Each organization opens in its last session, or choosing one.
    let commands = asks(&mut root, &mut time, click(pos2(380.0, 210.0)));
    assert!(
        matches!(
            commands.as_slice(),
            [CoreCommand::OpenOrganization(ENGINES)]
        ),
        "Open opens the organization: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(597.0, 254.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ChooseSessionIn(LOOMS)]),
        "Sessions opens it choosing a session: {commands:?}"
    );

    // Creating one, by name.
    let _ = asks(&mut root, &mut time, click(pos2(357.0, 427.0)));
    let _ = asks(&mut root, &mut time, typed("Lovelace Ltd"));
    let commands = asks(&mut root, &mut time, click(pos2(575.0, 427.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CreateOrganization(name)] if name == "Lovelace Ltd"),
        "Create creates it: {commands:?}"
    );

    // An invitation's link, a bearer secret, crosses as typed inside the command alone.
    let _ = asks(&mut root, &mut time, click(pos2(361.0, 351.0)));
    let _ = asks(
        &mut root,
        &mut time,
        typed("https://example.com/invite#code"),
    );
    let commands = asks(&mut root, &mut time, click(pos2(579.0, 351.0)));
    match commands.as_slice() {
        [CoreCommand::OpenInvitation(link)] => assert_eq!(
            link.as_bytes(),
            b"https://example.com/invite#code",
            "the link is handed over as typed"
        ),
        other => panic!("Open invitation opens it: {other:?}"),
    }

    // The invitation opened, to join.
    show_core(
        &mut root,
        organizations(
            listed(),
            Some(Invitation {
                organization: "Looms".to_owned(),
                inviter: "Ada".to_owned(),
                problem: None,
                can_join: true,
            }),
        ),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_an_invitation_to_join_an_organization",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, click(pos2(567.0, 345.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::JoinInvited]),
        "Join joins it: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(490.0, 345.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelJoin]),
        "Cancel puts it aside: {commands:?}"
    );

    // Listing failed: Retry lists them again.
    show_core(&mut root, organizations(OrganizationsListed::Failed, None));
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_organizations_could_not_be_listed",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, click(pos2(409.0, 226.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ReloadOrganizations]),
        "Retry lists them again: {commands:?}"
    );

    // Waiting for a session: those of this device's type, each to open, and a new one.
    show_core(&mut root, sessions());
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_sessions",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, click(pos2(400.0, 295.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::OpenSession([5, ..])]),
        "a session opens: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(282.0, 339.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::NewSession(DESKTOP)]),
        "a new session is of this device's type: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(400.0, 131.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::UseProfile([7, ..])]),
        "a profile is used: {commands:?}"
    );

    // Every type, which the root shows by its own choice, asking the host nothing.
    let commands = asks(&mut root, &mut time, click(pos2(593.0, 215.0)));
    assert!(commands.is_empty(), "{commands:?}");
    let commands = asks(&mut root, &mut time, click(pos2(400.0, 339.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::OpenSession([6, ..])]),
        "a session of another type shows, and opens: {commands:?}"
    );

    // Once an organization's blocks are open, nothing waits.
    show_core(&mut root, CoreView::default());
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "nothing waiting, the onboarding goes",
        |done| layer_of(done, ONBOARDING_LAYER).is_none(),
    );
}
