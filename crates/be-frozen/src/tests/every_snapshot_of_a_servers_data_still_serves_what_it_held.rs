use be_block::{
    FolderContent, NAME, ROW_PREFIX, TextBlock, TextContent, TextIndentation, TextLanguage,
    database::DatabaseContent,
    database_schema::{ChoiceColor, ChoiceIcon, DatabaseSchemaContent},
    database_view::{DatabaseViewContent, DatabaseViewKind},
};
use be_cli::{session, tracker::Tracker};
use be_client::{Credentials, Peer, PeerConfig};
use be_keys::RecoveryPhrase;
use be_server::backup::{self, BackupKey, Directory};
use be_store::{ContentKey, MemoryStore};

use super::*;
use crate::scene;

#[tokio::test(flavor = "multi_thread")]
async fn every_snapshot_of_a_servers_data_still_serves_what_it_held() {
    assert!(!SNAPSHOTS.is_empty());
    for snapshot in SNAPSHOTS {
        check(snapshot).await;
    }
}

async fn check(snapshot: &Snapshot) {
    let unpacked = unpack(snapshot);
    let server = Server::start(&unpacked.data).await;
    let workspace = id(snapshot.workspace);
    let workspace_key = key(snapshot.workspace_key);

    let owner = server.connect().await;
    let ServerMessage::Authenticated { account, .. } =
        request(&owner, |request| ClientMessage::Login {
            request,
            email: scene::OWNER_EMAIL.into(),
            password: scene::OWNER_PASSWORD.into(),
        })
        .await
    else {
        panic!("{}: the owner's password no longer signs in", snapshot.name);
    };
    let by_token = server.connect().await;
    assert!(
        matches!(
            request(&by_token, |request| ClientMessage::Authenticate {
                request,
                token: snapshot.owner_token.into(),
            })
            .await,
            ServerMessage::Authenticated { account: again, .. } if again == account
        ),
        "{}: the owner's token no longer signs in",
        snapshot.name
    );
    assert_eq!(
        opened_key(&owner, workspace, snapshot.owner_phrase).await,
        workspace_key,
        "{}: the owner's recovery phrase no longer opens the workspace key",
        snapshot.name
    );

    let peer = Peer::connect(
        PeerConfig::new(
            server.socket(),
            ContentKey::from_bytes(workspace_key),
            Credentials::Token(snapshot.owner_token.into()),
        )
        .workspace(Some(workspace)),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let name = |block: &str| {
        let peer = &peer;
        let block = id(block);
        async move { peer.metadata(&peer.summary(block).await.unwrap()).unwrap() }
    };
    assert_eq!(
        name(snapshot.notes).await.get(&NAME).as_deref(),
        Some(scene::NOTES)
    );
    assert_eq!(
        name(snapshot.archive).await.get(&NAME).as_deref(),
        Some(scene::ARCHIVE)
    );
    let notes: FolderContent = peer.open(id(snapshot.notes)).await.unwrap().unwrap();
    assert_eq!(
        notes.root().blocks(),
        [
            snapshot.shopping,
            snapshot.program,
            snapshot.archive,
            snapshot.view
        ]
        .map(id)
    );

    let shopping: TextContent = peer.open(id(snapshot.shopping)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&shopping), scene::SHOPPING[2]);
    assert_eq!(shopping.root().language, TextLanguage::Markdown);
    assert_eq!(
        shopping.root().indentation,
        TextIndentation::Spaces { width: 2 }
    );
    let history = peer.history(id(snapshot.shopping)).await.unwrap();
    assert_eq!(history.len(), 3);
    assert!(history[0].pinned);
    assert_eq!(history[0].parents, [history[1].commit]);
    let first: TextContent = peer.open_commit(history[2].commit).await.unwrap();
    assert_eq!(TextBlock::text(&first), scene::SHOPPING[0]);

    let program: TextContent = peer.open(id(snapshot.program)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&program), scene::PROGRAM);
    assert_eq!(program.root().language, TextLanguage::Rust);
    assert_eq!(program.root().indentation, TextIndentation::Tabs);

    let archive: FolderContent = peer.open(id(snapshot.archive)).await.unwrap().unwrap();
    assert_eq!(archive.root().blocks(), [id(snapshot.long)]);
    let long: TextContent = peer.open(id(snapshot.long)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&long), scene::long_text());
    assert!(
        peer.manifest(id(snapshot.long))
            .await
            .unwrap()
            .unwrap()
            .chunks
            .len()
            > 1
    );

    let view: DatabaseViewContent = peer.open(id(snapshot.view)).await.unwrap().unwrap();
    assert_eq!(view.root().kind, DatabaseViewKind::List);
    assert_eq!(view.root().database, Some(id(snapshot.database)));
    assert_eq!(
        name(snapshot.database).await.get(&ROW_PREFIX).as_deref(),
        Some(scene::PREFIX)
    );
    let database: DatabaseContent = peer.open(id(snapshot.database)).await.unwrap().unwrap();
    assert_eq!(database.root().prefix, scene::PREFIX);
    let schema: DatabaseSchemaContent = peer
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

    let agent = Tracker::new(
        session::connect(&session::Settings {
            server: server.url.clone(),
            workspace: Some(workspace),
            token: snapshot.agent_token.into(),
            phrase: snapshot.agent_phrase.into(),
            authorization: None,
            cache: unpacked.root.join("cache"),
        })
        .await
        .unwrap(),
    );
    assert_eq!(
        agent.find_database(scene::DATABASE).await.unwrap(),
        id(snapshot.database)
    );
    let rows = agent.list(id(snapshot.database), &[]).await.unwrap();
    let titles: Vec<_> = rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, [scene::FIRST_ROW, scene::SECOND_ROW]);
    let first = agent.show(&rows[0].id).await.unwrap();
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

    let editor = server.connect().await;
    assert!(
        matches!(
            request(&editor, |request| ClientMessage::Login {
                request,
                email: scene::EDITOR_EMAIL.into(),
                password: scene::EDITOR_PASSWORD.into(),
            })
            .await,
            ServerMessage::Authenticated { .. }
        ),
        "{}: the editor's password no longer signs in",
        snapshot.name
    );
    assert_eq!(
        opened_key(&editor, workspace, snapshot.editor_phrase).await,
        workspace_key
    );
    request(&editor, |request| ClientMessage::OpenWorkspace {
        request,
        workspace,
    })
    .await;
    let ServerMessage::Blocks { blocks, .. } =
        request(&editor, |request| ClientMessage::ListBlocks { request }).await
    else {
        panic!("{}: the editor's blocks were not listed", snapshot.name);
    };
    let seen: Vec<_> = blocks.iter().map(|block| block.id).collect();
    assert!(seen.contains(&id(snapshot.shopping)));
    assert!(!seen.contains(&id(snapshot.program)));

    let invited = server.connect().await;
    request(&invited, |request| ClientMessage::Register {
        request,
        email: scene::INVITED_EMAIL.into(),
        display_name: "Invited".into(),
        password: "a password long enough".into(),
    })
    .await;
    let invitation: String = rusqlite::Connection::open(unpacked.data.join("metadata.sqlite"))
        .unwrap()
        .query_row(
            "SELECT id FROM invitations WHERE email = ?1",
            [scene::INVITED_EMAIL],
            |row| row.get(0),
        )
        .unwrap_or_else(|error| panic!("{}: the invitation is gone: {error}", snapshot.name));
    let invitation = id(&invitation);
    let ServerMessage::Invitation {
        invitation: found, ..
    } = request(&invited, |request| ClientMessage::GetInvitation {
        request,
        invitation,
    })
    .await
    else {
        panic!("{}: the invitation was not served", snapshot.name);
    };
    assert_eq!(found.workspace, workspace);
    assert!(matches!(
        request(&invited, |request| ClientMessage::AcceptInvitation {
            request,
            invitation,
        })
        .await,
        ServerMessage::Ok { .. }
    ));

    let mut edited = shopping.clone();
    let end = scene::SHOPPING[2].len();
    edited.apply(&TextBlock::insert(&shopping, 7, end, b"- bread\n").unwrap());
    let head = history[0].commit;
    assert!(
        peer.save(id(snapshot.shopping), &edited, Some(head))
            .await
            .unwrap()
            .published()
            .is_some()
    );
    let reopened: TextContent = peer.open(id(snapshot.shopping)).await.unwrap().unwrap();
    assert_eq!(
        TextBlock::text(&reopened),
        format!("{}- bread\n", scene::SHOPPING[2])
    );
    drop(peer);
    server.stop().await;

    let restored = unpacked.root.join("restored");
    let report = backup::restore(
        &Directory(unpacked.backup.clone()),
        &BackupKey::from_hex(snapshot.backup_key).unwrap(),
        &restored,
        None,
    )
    .unwrap_or_else(|error| panic!("{}: the backup no longer restores: {error}", snapshot.name));
    assert!(report.objects > 0);
    assert!(backup::verify(&restored).unwrap().missing.is_empty());
    let server = Server::start(&restored).await;
    let peer = Peer::connect(
        PeerConfig::new(
            server.socket(),
            ContentKey::from_bytes(workspace_key),
            Credentials::Token(snapshot.owner_token.into()),
        )
        .workspace(Some(workspace)),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let shopping: TextContent = peer.open(id(snapshot.shopping)).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&shopping), scene::SHOPPING[2]);
    drop(peer);
    server.stop().await;
}

async fn opened_key(connection: &Connection, workspace: Uuid, phrase: &str) -> [u8; 32] {
    let ServerMessage::Keys {
        recovery, sealed, ..
    } = request(connection, |request| ClientMessage::GetKeys { request }).await
    else {
        panic!("the keys were not sent");
    };
    let secret = RecoveryPhrase::parse(phrase).unwrap().secret();
    assert_eq!(recovery, Some(secret.public().0));
    let sealed = sealed
        .into_iter()
        .find(|key| key.workspace == workspace)
        .unwrap();
    secret.open(&sealed.sealed).unwrap().try_into().unwrap()
}
