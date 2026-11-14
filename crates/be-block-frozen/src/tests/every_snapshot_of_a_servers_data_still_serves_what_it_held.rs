use std::sync::Arc;

use be_block_client::ask;
use be_block_client::{Connection, contacts};
use be_block_content::{MY_FILES, NAME, ROW_PREFIX};
use be_block_keys::{InviteCode, RecoveryPhrase};
use be_block_protocol::{KeyKind, OrganizationKind, OrganizationRole, client};
use be_server::backup::{self, BackupKey, Directory};
use be_tool_cli::{cli, session};
use be_type_database::{
    database::DatabaseContent,
    database_schema::{ChoiceColor, ChoiceIcon, DatabaseSchemaContent},
    database_view::{DatabaseViewContent, DatabaseViewKind},
};
use be_type_text::{TextBlock, TextContent, TextIndentation, TextLanguage};
use be_type_workspace::{FolderContent, WorkspaceContent};

use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn every_snapshot_of_a_servers_data_still_serves_what_it_held() {
    for snapshot in SNAPSHOTS {
        check(snapshot).await;
    }
}

fn socket(server: &Running) -> String {
    be_block_client::socket_url(&server.url)
}

async fn portal_http(
    server: &Running,
    method: &str,
    path: &str,
    cookie: &str,
    body: &str,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let address = server.url.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nCookie: be3-account={cookie}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    String::from_utf8(bytes).unwrap()
}
fn csrf(page: &str) -> &str {
    page.split("\"csrf\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
}
fn form_value(value: &str) -> String {
    value.bytes().map(|byte| format!("%{byte:02X}")).collect()
}
async fn sign_in(server: &Running, email: &str, password: &str) -> (Arc<Connection>, String) {
    let be_block_protocol::BrowserLogin { code, secret, .. } =
        be_block_client::call_once(&server.url, |peer| {
            client::account::begin_browser_login(peer)
        })
        .await
        .expect("browser sign-in starts");
    let page = portal_http(server, "GET", "/auth/login", "", "").await;
    let signed_in = portal_http(
        server,
        "POST",
        "/auth/login",
        "",
        &format!(
            "csrf={}&email={}&password={}",
            csrf(&page),
            form_value(email),
            form_value(password)
        ),
    )
    .await;
    assert!(signed_in.starts_with("HTTP/1.1 303"), "{signed_in}");
    let cookie = signed_in
        .split("Set-Cookie: be3-account=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let page = portal_http(server, "GET", &format!("/auth?code={code}"), cookie, "").await;
    let approved = portal_http(
        server,
        "POST",
        "/auth/approve",
        cookie,
        &format!("csrf={}&code={code}", csrf(&page)),
    )
    .await;
    assert!(approved.starts_with("HTTP/1.1 303"), "{approved}");
    let Ok(be_block_protocol::BrowserLoginState::Approved(be_block_protocol::Authenticated {
        token,
        ..
    })) = be_block_client::call_once(&server.url, |peer| {
        client::account::poll_browser_login(peer, code, secret)
    })
    .await
    else {
        panic!("{email}'s password no longer signs in")
    };
    let connection = Connection::connect(&socket(server), &be_block_client::Clock::real())
        .await
        .unwrap();
    ask(client::account::authenticate(
        &connection,
        Some(token.clone()),
    ))
    .await
    .expect("the token signs in");
    (connection, token)
}

async fn peer(
    server: &Running,
    phrase: &str,
    token: &str,
    organization: Uuid,
) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            socket(server),
            RecoveryPhrase::parse(phrase).unwrap().secret(),
            Credentials::Token(token.into()),
        )
        .organization(Some(organization)),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

async fn register(server: &Running, email: &str, organization: Uuid) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            socket(server),
            RecoveryPhrase::generate().secret(),
            Credentials::Token(server.register(email, email).await),
        )
        .organization(Some(organization)),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

async fn text<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: &str) -> String {
    let content: TextContent = peer.open(id(block)).await.unwrap().unwrap();
    TextBlock::text(&content)
}

async fn check(snapshot: &Snapshot) {
    let name = snapshot.name;
    let unpacked = unpack(snapshot);
    let server = start(Some(unpacked.data.clone())).await;
    let organization = id(snapshot.organization);
    let personal = id(snapshot.personal);

    let (by_password, owner_token) =
        sign_in(&server, scene::OWNER_EMAIL, scene::OWNER_PASSWORD).await;
    let store = be_server::ServerStore::open(&server.directory).unwrap();
    assert!(
        store.resolve_token(snapshot.owner_token).await.is_ok(),
        "{name}: the owner's session was not kept"
    );
    assert!(
        store.resolve_token(snapshot.friend_token).await.is_ok(),
        "{name}: the friend's session was not kept"
    );
    let Ok(be_block_protocol::RecoveryKey { public, .. }) =
        ask(client::keys::get_recovery_key(&by_password)).await
    else {
        panic!("{name}: the owner's recovery key was not sent");
    };
    assert_eq!(
        public,
        Some(
            RecoveryPhrase::parse(snapshot.owner_phrase)
                .unwrap()
                .secret()
                .public()
                .0
        ),
        "{name}: the owner's phrase no longer matches its recovery key"
    );
    let Ok(organizations) = ask(client::organizations::list_organizations(&by_password)).await
    else {
        panic!("{name}: the owner's organizations were not listed");
    };
    let kinds: Vec<_> = organizations
        .iter()
        .map(|listed| (listed.id, listed.kind, listed.role))
        .collect();
    assert!(kinds.contains(&(
        organization,
        OrganizationKind::Company,
        OrganizationRole::Administrator
    )));
    assert!(kinds.contains(&(
        personal,
        OrganizationKind::Personal,
        OrganizationRole::Editor
    )));

    let owner = peer(&server, snapshot.owner_phrase, &owner_token, organization).await;
    assert!(
        !owner
            .organization_policies()
            .await
            .unwrap()
            .shares_outside(),
        "{name}: the organization's policies no longer read"
    );
    let offer = owner
        .offer(id(snapshot.team), be_block_graph::Access::View)
        .await
        .unwrap();
    let card = contacts::card(
        owner.account(),
        owner.keys().identity(),
        scene::OWNER_NAME,
        scene::OWNER_EMAIL,
    );
    let refused = contacts::make_link(
        &owner.connection(),
        owner.keys().identity(),
        &card,
        Some(&offer),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            refused,
            be_block_client::ClientError::Refused(be_block_protocol::ErrorCode::PolicyRefused, _)
        ),
        "{name}: the server no longer reads the policies shared with it: {refused}"
    );
    check_storage(snapshot, &server, &store, &owner).await;
    let metadata = |block: &str| {
        let owner = &owner;
        let block = id(block);
        async move {
            owner
                .metadata(&owner.summary(block).await.unwrap())
                .unwrap()
        }
    };
    let my_files = metadata(snapshot.my_files).await;
    assert_eq!(my_files.get(&NAME).as_deref(), Some(scene::MY_FILES));
    assert_eq!(my_files.get(&MY_FILES), Some(true));
    assert_eq!(
        metadata(snapshot.team).await.get(&NAME).as_deref(),
        Some(scene::TEAM)
    );
    assert_eq!(
        metadata(snapshot.notes).await.get(&NAME).as_deref(),
        Some(scene::NOTES)
    );
    assert_eq!(
        metadata(snapshot.archive).await.get(&NAME).as_deref(),
        Some(scene::ARCHIVE)
    );
    let mine: WorkspaceContent = owner.open(id(snapshot.my_files)).await.unwrap().unwrap();
    assert_eq!(mine.root().blocks(), [id(snapshot.notes)]);
    let notes: FolderContent = owner.open(id(snapshot.notes)).await.unwrap().unwrap();
    assert_eq!(
        notes.root().blocks(),
        [snapshot.shopping, snapshot.program, snapshot.archive].map(id)
    );
    let team: WorkspaceContent = owner.open(id(snapshot.team)).await.unwrap().unwrap();
    assert_eq!(team.root().blocks(), [snapshot.view, snapshot.plan].map(id));

    let shopping: TextContent = owner.open(id(snapshot.shopping)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&shopping), scene::SHOPPING[2]);
    let history = owner.history(id(snapshot.shopping)).await.unwrap();
    assert_eq!(history.len(), 3);
    assert!(history[0].pinned);
    assert_eq!(history[0].parents, [history[1].commit]);
    let first: TextContent = owner.open_commit(history[2].commit).await.unwrap();
    assert_eq!(TextBlock::text(&first), scene::SHOPPING[0]);

    let program: TextContent = owner.open(id(snapshot.program)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&program), scene::PROGRAM);
    assert_eq!(program.root().language, TextLanguage::Rust);
    assert_eq!(program.root().indentation, TextIndentation::Tabs);

    let archive: FolderContent = owner.open(id(snapshot.archive)).await.unwrap().unwrap();
    assert_eq!(archive.root().blocks(), [id(snapshot.long)]);
    assert_eq!(text(&owner, snapshot.long).await, scene::long_text());
    let long = owner.manifest(id(snapshot.long)).await.unwrap().unwrap();
    assert!(owner.objects_of(&long).await.unwrap().len() > 1);

    let view: DatabaseViewContent = owner.open(id(snapshot.view)).await.unwrap().unwrap();
    assert_eq!(view.root().kind, DatabaseViewKind::List);
    assert_eq!(view.root().database, Some(id(snapshot.database)));
    assert_eq!(
        metadata(snapshot.database)
            .await
            .get(&ROW_PREFIX)
            .as_deref(),
        Some(scene::PREFIX)
    );
    let database: DatabaseContent = owner.open(id(snapshot.database)).await.unwrap().unwrap();
    assert_eq!(database.root().prefix, scene::PREFIX);
    let schema: DatabaseSchemaContent = owner
        .open(database.root().schema.unwrap())
        .await
        .unwrap()
        .unwrap();
    let status = schema.root().field_named("Status").unwrap();
    let options: Vec<_> = status
        .enum_options
        .iter()
        .map(|option| option.name.as_str())
        .collect();
    assert_eq!(options, scene::STATUSES);
    let done = &status.enum_options[2];
    assert_eq!(
        (done.color, done.icon, done.closed),
        (ChoiceColor::Blue, ChoiceIcon::Done, true)
    );
    assert_eq!(status.default_value(), Some(status.enum_options[0].id));

    assert_eq!(text(&owner, snapshot.plan).await, scene::PLAN[1]);
    let plan_history = owner.history(id(snapshot.plan)).await.unwrap();
    assert_eq!(plan_history.len(), 2);
    let before: TextContent = owner.open_commit(plan_history[1].commit).await.unwrap();
    assert_eq!(
        TextBlock::text(&before),
        scene::PLAN[0],
        "{name}: what was written before the rotation no longer opens"
    );
    assert_eq!(
        owner
            .keys()
            .keys_of(id(snapshot.team), KeyKind::Scope)
            .len(),
        2,
        "{name}: the team's scope key lost an epoch"
    );
    assert!(owner.keys().keys_of(id(snapshot.plan), KeyKind::Data).len() >= 2);

    let work = owner.key_work().await.unwrap();
    assert_eq!(work.escrows.len(), 2, "{name}: the escrow chain changed");
    let latest = work
        .escrows
        .iter()
        .max_by_key(|escrow| escrow.epoch)
        .cloned();
    assert_eq!(owner.keys().escrow(), latest);
    assert_eq!(owner.keys().escrow_warning(), None);

    let contacts = owner.refresh_contacts().await.unwrap();
    let friend_public = RecoveryPhrase::parse(snapshot.friend_phrase)
        .unwrap()
        .secret()
        .public();
    let friend = contacts
        .iter()
        .find(|contact| contact.email == scene::FRIEND_EMAIL)
        .expect("the friend is still a contact");
    assert_eq!(
        friend.public,
        Some(friend_public),
        "{name}: the friend's change of phrase was not followed"
    );
    for email in [scene::EDITOR_EMAIL, scene::FRIEND_EMAIL] {
        assert!(
            contacts
                .iter()
                .any(|contact| contact.email == email && contact.public.is_some()),
            "{name}: {email} is no longer a verified contact"
        );
    }
    assert!(
        contacts
            .iter()
            .any(|contact| contact.display_name == scene::AGENT_NAME && contact.public.is_some())
    );

    let agent = Tracker::new(
        session::connect(&session::Settings {
            server: server.url.clone(),
            organization: Some(organization),
            token: snapshot.agent_token.into(),
            phrase: snapshot.agent_phrase.into(),
            authorization: None,
            cache: unpacked.root.join("cache"),
            boat_id: None,
        })
        .await
        .unwrap(),
        sources(),
    );
    assert_eq!(agent.peer().keys().escrow(), latest);
    let listed = run(&agent, &["ls"]).await;
    assert!(listed.contains(scene::TEAM), "{listed}");
    assert_eq!(
        agent.find_database(scene::DATABASE).await.unwrap(),
        id(snapshot.database)
    );
    let rows = agent.list(id(snapshot.database), &[]).await.unwrap();
    let titles: Vec<_> = rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, [scene::FIRST_ROW, scene::SECOND_ROW]);
    let first = agent
        .show(id(snapshot.database), &rows[0].id)
        .await
        .unwrap();
    for (field, text) in [("Status", "Done"), ("Estimate", "3"), ("Urgent", "true")] {
        assert_eq!(
            first.field(field).map(|field| field.text.as_str()),
            Some(text),
            "{field}"
        );
    }
    assert_eq!(first.body.as_deref(), Some(scene::FIRST_PAGE));
    let blocked = rows[1].field("Blocked by").unwrap();
    assert_eq!(
        blocked.links,
        [(rows[0].id.clone(), scene::FIRST_ROW.to_owned())]
    );
    assert_eq!(text(agent.peer(), snapshot.plan).await, scene::PLAN[1]);
    assert!(
        !agent
            .peer()
            .summary(id(snapshot.shopping))
            .await
            .is_ok_and(|summary| summary.access.can_view())
    );

    let (_, editor_token) = sign_in(&server, scene::EDITOR_EMAIL, scene::EDITOR_PASSWORD).await;
    let editor = peer(&server, snapshot.editor_phrase, &editor_token, organization).await;
    assert_eq!(editor.role(), OrganizationRole::Editor);
    assert!(
        editor.summary(id(snapshot.plan)).await.is_err(),
        "{name}: the editor taken off the team still sees it"
    );
    assert_eq!(editor.keys().escrow(), latest);

    let (_, friend_token) = sign_in(&server, scene::FRIEND_EMAIL, scene::FRIEND_PASSWORD).await;
    let friend = peer(&server, snapshot.friend_phrase, &friend_token, personal).await;
    assert_eq!(
        text(&friend, snapshot.holiday_note).await,
        scene::HOLIDAY_NOTE
    );
    assert!(friend.summary(id(snapshot.recipes)).await.is_err());
    let owner_personal = peer(&server, snapshot.owner_phrase, &owner_token, personal).await;
    assert_eq!(
        text(&owner_personal, snapshot.usage.friend_note).await,
        scene::FRIEND_NOTE
    );
    let holiday = owner_personal.summary(id(snapshot.holiday)).await.unwrap();
    let holiday_note = owner_personal
        .summary(id(snapshot.holiday_note))
        .await
        .unwrap();
    let friend_note = owner_personal
        .summary(id(snapshot.usage.friend_note))
        .await
        .unwrap();
    assert!(friend_note.size > 0);
    assert_eq!(
        holiday.tree_size,
        holiday.size + holiday_note.tree_size + friend_note.tree_size,
        "{name}: what the friend made in the owner's folder is not in its tree"
    );
    let friend_storage = client::account::storage_changed(&friend.connection())
        .recv()
        .await
        .unwrap();
    assert_eq!(
        (friend_storage.held, snapshot.usage.friend_held),
        (0, 0),
        "{name}: the friend is charged for a block in the owner's tree"
    );
    assert_eq!(
        text(&owner_personal, snapshot.holiday_note).await,
        scene::HOLIDAY_NOTE
    );
    assert_eq!(
        text(&owner_personal, snapshot.recipes).await,
        scene::RECIPES
    );
    let personal_files: WorkspaceContent = owner_personal
        .open(id(snapshot.personal_files))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        personal_files.root().blocks(),
        [snapshot.holiday, snapshot.recipes].map(id)
    );

    let taker = register(&server, "taker@example.com", personal).await;
    let code = InviteCode::parse(snapshot.link_code).unwrap();
    let me = contacts::card(
        taker.account(),
        taker.keys().identity(),
        "Taker",
        "taker@example.com",
    );
    let card = contacts::accept_link(&taker.connection(), taker.keys().identity(), &me, &code)
        .await
        .unwrap_or_else(|error| panic!("{name}: the pending link no longer opens: {error}"));
    assert_eq!(card.email, scene::OWNER_EMAIL);
    let taker = Peer::connect(
        PeerConfig::new(
            socket(&server),
            taker.keys().identity().clone(),
            Credentials::Token(taker.token().to_owned()),
        )
        .organization(Some(personal)),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(text(&taker, snapshot.recipes).await, scene::RECIPES);

    let invited = register(&server, scene::INVITED_EMAIL, personal).await;
    let me = contacts::card(
        invited.account(),
        invited.keys().identity(),
        "Invited",
        scene::INVITED_EMAIL,
    );
    let code = InviteCode::parse(snapshot.invite_code).unwrap();
    let invitation =
        contacts::accept_invitation(&invited.connection(), invited.keys().identity(), &me, &code)
            .await
            .unwrap_or_else(|error| panic!("{name}: the invitation no longer opens: {error}"));
    assert_eq!(invitation.organization, organization);
    let invited = Peer::connect(
        PeerConfig::new(
            socket(&server),
            invited.keys().identity().clone(),
            Credentials::Token(invited.token().to_owned()),
        )
        .organization(Some(organization)),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        invited.keys().escrow(),
        latest,
        "{name}: the escrow key the invitation pinned no longer leads to the newest"
    );

    let mut edited = shopping.clone();
    let end = scene::SHOPPING[2].len();
    edited.apply(&TextBlock::insert(&shopping, 7, end, b"- bread\n").unwrap());
    assert!(
        owner
            .save(id(snapshot.shopping), &edited, Some(history[0].commit))
            .await
            .unwrap()
            .published()
            .is_some()
    );
    assert_eq!(
        text(&owner, snapshot.shopping).await,
        format!("{}- bread\n", scene::SHOPPING[2])
    );
    drop((owner, owner_personal, friend, editor, agent, taker, invited));
    server.stop().await;

    let restored = unpacked.root.join("restored");
    let report = backup::restore(
        &Directory(unpacked.backup.clone()),
        &BackupKey::from_hex(snapshot.backup_key).unwrap(),
        &restored,
        None,
        None,
    )
    .unwrap_or_else(|error| panic!("{name}: the backup no longer restores: {error}"));
    assert!(report.objects > 0);
    assert!(backup::verify(&restored, None).unwrap().missing.is_empty());
    let server = start(Some(restored)).await;
    let (_, owner_token) = sign_in(&server, scene::OWNER_EMAIL, scene::OWNER_PASSWORD).await;
    let owner = peer(&server, snapshot.owner_phrase, &owner_token, organization).await;
    assert_eq!(text(&owner, snapshot.shopping).await, scene::SHOPPING[2]);
    assert_eq!(text(&owner, snapshot.plan).await, scene::PLAN[1]);
    drop(owner);
    server.stop().await;
}

/// Storage accounting: block and subtree sizes, usage, the quota policy, an agent's charges, and
/// invite codes.
async fn check_storage(
    snapshot: &Snapshot,
    server: &Running,
    store: &be_server::ServerStore,
    owner: &Peer<MemoryStore>,
) {
    let name = snapshot.name;
    let summary = |block: &str| {
        let block = id(block);
        async move { owner.summary(block).await.unwrap() }
    };
    let (my_files, notes, shopping, program, archive, long) = (
        summary(snapshot.my_files).await,
        summary(snapshot.notes).await,
        summary(snapshot.shopping).await,
        summary(snapshot.program).await,
        summary(snapshot.archive).await,
        summary(snapshot.long).await,
    );
    for leaf in [&shopping, &program, &long] {
        assert!(leaf.size > 0, "{name}: a block's objects were not counted");
        assert_eq!(
            leaf.tree_size, leaf.size,
            "{name}: a leaf's tree is more than itself"
        );
    }
    assert_eq!(archive.tree_size, archive.size + long.tree_size, "{name}");
    assert_eq!(
        notes.tree_size,
        notes.size + shopping.tree_size + program.tree_size + archive.tree_size,
        "{name}: a folder's tree is not itself and its children's"
    );
    assert_eq!(
        my_files.tree_size,
        my_files.size + notes.tree_size,
        "{name}"
    );
    let storage = client::account::storage_changed(&owner.connection())
        .recv()
        .await
        .unwrap();
    assert!(
        storage.held >= long.size,
        "{name}: the owner is charged less than one of its blocks holds"
    );

    let usage = &snapshot.usage;
    assert_eq!(
        (storage.held, storage.pending),
        (usage.owner_held, usage.owner_pending),
        "{name}: the owner's usage changed"
    );
    assert!(
        usage.owner_pending >= scene::PENDING_UPLOAD.len() as u64,
        "{name}: the upload no block holds no longer counts"
    );
    assert_eq!(my_files.tree_size, usage.my_files_tree_size, "{name}");
    let team = summary(snapshot.team).await;
    assert_eq!(
        (team.size, team.tree_size),
        (usage.team_size, usage.team_tree_size),
        "{name}: the team's sizes changed"
    );
    assert_eq!(text(owner, usage.agent_note).await, scene::AGENT_NOTE);
    assert_eq!(
        owner.organization_policies().await.unwrap().quota(),
        Some(scene::STORAGE_QUOTA),
        "{name}: the storage quota no longer reads in bytes"
    );
    let agent = Connection::connect(&socket(server), &be_block_client::Clock::real())
        .await
        .unwrap();
    ask(client::account::authenticate(
        &agent,
        Some(snapshot.agent_token.to_owned()),
    ))
    .await
    .unwrap();
    ask(client::organizations::open_organization(
        &agent,
        id(snapshot.organization),
    ))
    .await
    .unwrap();
    let charged = client::account::storage_changed(&agent)
        .recv()
        .await
        .unwrap();
    assert_eq!(
        (charged.held, charged.pending),
        (storage.held, storage.pending),
        "{name}: the agent is no longer charged to the account that created it"
    );

    let register = |email: &'static str, invite: &'static str| async move {
        store
            .registration_link(
                email,
                email,
                scene::REGISTERED_PASSWORD,
                invite,
                "",
                "192.0.2.7",
            )
            .await
    };
    assert!(
        register("again@example.com", usage.used_invite)
            .await
            .is_err(),
        "{name}: a used-up invite code registered another account"
    );
    let link = register("unused@example.com", usage.unused_invite)
        .await
        .unwrap_or_else(|error| panic!("{name}: the unused invite code was refused: {error}"));
    store
        .confirm_mail(&link.token, None, "192.0.2.7")
        .await
        .unwrap();
    assert!(
        register("third@example.com", usage.unused_invite)
            .await
            .is_err(),
        "{name}: an invite code registered more accounts than it allows"
    );
    assert!(
        store
            .login_browser(
                scene::REGISTERED_EMAIL,
                scene::REGISTERED_PASSWORD,
                "192.0.2.7"
            )
            .await
            .is_ok(),
        "{name}: the account the used code made no longer signs in"
    );
}

async fn run(tracker: &Tracker<be_block_store::FileStore>, arguments: &[&str]) -> String {
    let invocation = cli::parse(
        arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        || Err("no standard input".to_owned()),
    )
    .unwrap();
    match cli::run(tracker, &invocation).await.unwrap() {
        cli::Outcome::Print(printed) => printed,
        cli::Outcome::Edit { .. } | cli::Outcome::Watch { .. } => {
            panic!("{arguments:?} asked for an editor or a watch")
        }
    }
}
