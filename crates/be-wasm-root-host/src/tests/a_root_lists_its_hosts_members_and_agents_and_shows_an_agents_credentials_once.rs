use be_lib_geometry::pos2;
use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, OPENS_AGENTS, OPENS_MEMBERS};
use be_wasm_root_api::{
    AgentCredentials, AgentRow, AgentsArea, CoreCommand, CoreSecret, CoreView, MemberRole,
    MemberRow, MembersArea, RoleChange, SecretCommand,
};

use super::*;

const ME: [u8; 16] = [1; 16];
const GRACE: [u8; 16] = [2; 16];
const BUILDER: [u8; 16] = [3; 16];

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

/// What the root paints its dialog with, a few frames from `*time` on.
fn painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    let _ = asks(root, time, Vec::new());
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its dialog")
}

fn members() -> CoreView {
    let member = |account, name: &str, role, me, agent| MemberRow {
        account,
        name: name.to_owned(),
        email: format!("{}@example.com", name.to_lowercase()),
        role,
        me,
        agent,
        unsigned: false,
    };
    CoreView {
        members: MembersArea {
            listed: true,
            members: vec![
                member(GRACE, "Grace", MemberRole::Editor, false, false),
                member(ME, "Ada", MemberRole::Administrator, true, false),
                member(BUILDER, "Builder", MemberRole::Editor, false, true),
            ],
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

fn agents() -> CoreView {
    CoreView {
        agents: AgentsArea {
            listed: true,
            agents: vec![AgentRow {
                account: BUILDER,
                name: "Builder".to_owned(),
            }],
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

#[test]
fn a_root_lists_its_hosts_members_and_agents_and_shows_an_agents_credentials_once() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // F10 lists the members.
    let commands = asks(&mut root, &mut time, key(OPENS_MEMBERS));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ListMembers]),
        "opening them lists them: {commands:?}"
    );
    root.show_core(members());
    beui_paint_snapshot::assert_painting(
        "a_root_lists_its_hosts_members",
        painting(&mut root, &mut time),
    );

    // A member's role changes; the person themselves and agents have no such buttons.
    let commands = asks(&mut root, &mut time, click(pos2(222.0, 136.0)));
    assert!(
        matches!(
            commands.as_slice(),
            [CoreCommand::SetMemberRole(RoleChange { account: GRACE, role: MemberRole::Administrator })]
        ),
        "Make administrator makes them one: {commands:?}"
    );

    // Removing one asks first: Escape keeps them, Remove removes them.
    let commands = asks(&mut root, &mut time, click(pos2(324.0, 136.0)));
    assert!(commands.is_empty(), "asking removes no one: {commands:?}");
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_removing_a_member",
        painting(&mut root, &mut time),
    );
    let commands = asks(&mut root, &mut time, key(Key::Escape));
    assert!(commands.is_empty(), "Escape keeps them: {commands:?}");
    assert!(layer_of(&step(&mut root, time), DIALOG_LAYER).is_some(), "the list stays open");
    let _ = asks(&mut root, &mut time, click(pos2(324.0, 136.0)));
    let commands = asks(&mut root, &mut time, click(pos2(296.0, 163.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::RemoveMember(GRACE)]),
        "Remove removes them: {commands:?}"
    );
    let _ = asks(&mut root, &mut time, key(Key::Escape));
    assert!(!layer_of(&step(&mut root, time), DIALOG_LAYER).is_some(), "Escape closes the list");

    // F11 lists the agents; a name makes one, whose credentials come back once.
    let commands = asks(&mut root, &mut time, key(OPENS_AGENTS));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ListAgents]),
        "opening them lists them: {commands:?}"
    );
    root.show_core(agents());
    let listed = painting(&mut root, &mut time);
    beui_paint_snapshot::assert_painting("a_root_lists_its_hosts_agents", listed.clone());
    let _ = asks(
        &mut root,
        &mut time,
        [ImeEvent::Enabled, ImeEvent::CommitText("Indexer".to_owned())]
            .into_iter()
            .map(|event| Input::Ime(event).into())
            .collect(),
    );
    let _ = asks(&mut root, &mut time, key(Key::Enter));
    let requests = root.take_secret_requests();
    let [(id, SecretCommand::CreateAgent(name))] = requests.as_slice() else {
        panic!("Create agent asks for one: {requests:?}");
    };
    assert_eq!(name, "Indexer");
    root.answer_secret(
        *id,
        Some(CoreSecret::Agent(AgentCredentials {
            name: "Indexer".to_owned(),
            server: "https://example.com".to_owned(),
            organization: [4; 16],
            token: Password::from("t0ken".to_owned()),
            phrase: Password::from("apple banana cherry".to_owned()),
        })),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_an_agents_credentials",
        painting(&mut root, &mut time),
    );
    let _ = asks(&mut root, &mut time, key(Key::Enter));
    assert_eq!(root.take_copies().len(), 1, "Copy copies them once");

    // Closed, they go: opened again, the agents show without them.
    let _ = asks(&mut root, &mut time, key(Key::Escape));
    let _ = asks(&mut root, &mut time, key(OPENS_AGENTS));
    assert_eq!(painting(&mut root, &mut time), listed, "reopened, no credentials are held");
    // While an agent is being made the dialog stays open, its credentials not to be lost:
    // Escape waits, and so does a question of the host's, until they come.
    let _ = asks(
        &mut root,
        &mut time,
        [ImeEvent::Enabled, ImeEvent::CommitText("Crawler".to_owned())]
            .into_iter()
            .map(|event| Input::Ime(event).into())
            .collect(),
    );
    root.input(None, be_lib_ui_types::input::encode(&key(Key::Enter)))
        .expect("the root takes input");
    root.input(None, be_lib_ui_types::input::encode(&key(Key::Escape)))
        .expect("the root takes input");
    root.show_core(CoreView {
        discarding: Some(be_wasm_root_api::DiscardReason::Close),
        ..agents()
    });
    let _ = asks(&mut root, &mut time, Vec::new());
    let requests = root.take_secret_requests();
    let [(id, SecretCommand::CreateAgent(name))] = requests.as_slice() else {
        panic!("Create agent asks for one, Escape cancelling nothing: {requests:?}");
    };
    assert_eq!(name, "Crawler");
    assert!(layer_of(&step(&mut root, time), DIALOG_LAYER).is_some(), "the agents stay");
    let waiting = painting(&mut root, &mut time);
    root.answer_secret(
        *id,
        Some(CoreSecret::Agent(AgentCredentials {
            name: "Crawler".to_owned(),
            server: "https://example.com".to_owned(),
            organization: [4; 16],
            token: Password::from("t0ken2".to_owned()),
            phrase: Password::from("date elder fig".to_owned()),
        })),
    );
    assert_ne!(painting(&mut root, &mut time), listed, "the credentials come, and show");
    // Shown, Escape closes them, and the question waiting opens.
    let _ = asks(&mut root, &mut time, key(Key::Escape));
    let discard = painting(&mut root, &mut time);
    assert_ne!(discard, listed, "the host's question opens once the agents close");
    assert_ne!(waiting, discard, "while the agent was made, the question waited");
    let _ = asks(&mut root, &mut time, key(Key::Escape));
    root.show_core(agents());
    let _ = asks(&mut root, &mut time, Vec::new());

    let _ = asks(&mut root, &mut time, key(OPENS_AGENTS));
    let _ = root.take_core_commands();
    let commands = asks(&mut root, &mut time, click(pos2(297.0, 122.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::RemoveAgent(BUILDER)]),
        "Remove removes the agent: {commands:?}"
    );
}
