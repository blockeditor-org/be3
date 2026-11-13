use std::collections::BTreeSet;

use be_block_graph::Access;
use be_block_protocol::ChangeCursor;
use be_block_protocol::embed::METHODS;

use super::*;

#[derive(Debug, PartialEq)]
pub(super) enum Answered {
    Allowed,
    Refused(ErrorCode),
    /// A stream the server ended at once, or a session it would not open.
    Denied,
}

fn answered<T, F>(response: Result<Result<T, Error>, F>) -> Answered {
    match response {
        Ok(Ok(_)) => Answered::Allowed,
        Ok(Err(error)) => Answered::Refused(error.code),
        Err(_) => Answered::Denied,
    }
}

impl TestClient {
    async fn opened<T, S: be_wasm_ops::wire::Decode<T>>(
        &mut self,
        mut stream: Stream<T, S>,
    ) -> Answered {
        let _ = client::organizations::list_organizations(&self.peer).await;
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match stream.poll_recv(&mut context) {
            std::task::Poll::Ready(None) => Answered::Denied,
            _ => Answered::Allowed,
        }
    }

    /// Makes every call of `be3:server` that names a block on `block`, and the rest with arguments
    /// that would do something were the caller allowed to; answers how each was answered.
    pub(super) async fn everything(
        &mut self,
        block: Uuid,
        organization: Uuid,
    ) -> Vec<(&'static str, Answered)> {
        let peer = self.peer.clone();
        let peer = &peer;
        let other = Uuid::new_v4();
        let hash = Hash::of(b"an object");
        let commit = CommitId::from_hash(hash);
        let sealed = || b"sealed".to_vec();
        let mut answers = vec![
            (
                "ping",
                match client::connection::ping(peer).await {
                    Ok(()) => Answered::Allowed,
                    Err(_) => Answered::Denied,
                },
            ),
            (
                "open-organization",
                answered(client::organizations::open_organization(peer, organization).await),
            ),
            (
                "begin-browser-login",
                answered(client::account::begin_browser_login(peer).await),
            ),
            (
                "poll-browser-login",
                answered(
                    client::account::poll_browser_login(peer, "code".into(), "secret".into()).await,
                ),
            ),
            (
                "create-local-profile",
                answered(client::account::create_local_profile(peer, "someone".into()).await),
            ),
            (
                "authenticate",
                answered(client::account::authenticate(peer, Some("not a token".into())).await),
            ),
            (
                "pair",
                answered(client::account::pair(peer, None, sealed()).await),
            ),
            (
                "list-organizations",
                answered(client::organizations::list_organizations(peer).await),
            ),
            (
                "create-organization",
                answered(client::organizations::create_organization(peer, "mine".into()).await),
            ),
            (
                "invite",
                answered(
                    client::organizations::invite(
                        peer,
                        other,
                        organization,
                        "invited@example.com".into(),
                        OrganizationRole::Administrator,
                        sealed(),
                        sealed(),
                        sealed(),
                    )
                    .await,
                ),
            ),
            (
                "get-invitation",
                answered(client::organizations::get_invitation(peer, other).await),
            ),
            (
                "accept-invitation",
                answered(
                    client::organizations::accept_invitation(
                        peer,
                        other,
                        sealed(),
                        sealed(),
                        sealed(),
                    )
                    .await,
                ),
            ),
            (
                "list-members",
                answered(client::members::list_members(peer).await),
            ),
            (
                "set-role",
                answered(
                    client::members::set_role(peer, other, OrganizationRole::Administrator).await,
                ),
            ),
            (
                "remove-member",
                answered(client::members::remove_member(peer, other).await),
            ),
            (
                "create-agent",
                answered(
                    client::members::create_agent(
                        peer,
                        organization,
                        "agent".into(),
                        [7; 32],
                        [7; 32],
                        sealed(),
                        sealed(),
                        None,
                    )
                    .await,
                ),
            ),
            (
                "list-agents",
                answered(client::members::list_agents(peer, organization).await),
            ),
            (
                "rotate-agent-token",
                answered(client::members::rotate_agent_token(peer, organization, other).await),
            ),
            (
                "remove-agent",
                answered(client::members::remove_agent(peer, organization, other).await),
            ),
            (
                "create-contact-link",
                answered(
                    client::contacts::create_contact_link(peer, other, sealed(), sealed(), None)
                        .await,
                ),
            ),
            (
                "get-contact-link",
                answered(client::contacts::get_contact_link(peer, other).await),
            ),
            (
                "accept-contact-link",
                answered(
                    client::contacts::accept_contact_link(
                        peer,
                        other,
                        sealed(),
                        sealed(),
                        sealed(),
                    )
                    .await,
                ),
            ),
            (
                "list-contact-links",
                answered(client::contacts::list_contact_links(peer).await),
            ),
            (
                "delete-contact-link",
                answered(client::contacts::delete_contact_link(peer, other).await),
            ),
            (
                "put-contact",
                answered(client::contacts::put_contact(peer, other, sealed()).await),
            ),
            (
                "list-contacts",
                answered(client::contacts::list_contacts(peer).await),
            ),
            (
                "remove-contact",
                answered(client::contacts::remove_contact(peer, other).await),
            ),
            (
                "upload-urls",
                answered(
                    client::objects::upload_urls(
                        peer,
                        vec![be_block_protocol::ObjectLength { hash, length: 9 }],
                    )
                    .await,
                ),
            ),
            (
                "download-urls",
                answered(client::objects::download_urls(peer, vec![hash]).await),
            ),
            (
                "missing-objects",
                answered(client::objects::missing_objects(peer, vec![hash]).await),
            ),
            (
                "hold-objects",
                answered(
                    client::objects::hold_objects(peer, block, vec![hash], Hold::Lasting).await,
                ),
            ),
            (
                "create-block",
                answered(
                    client::blocks::create_block(
                        peer,
                        other,
                        CONTENT,
                        BlockParent::Block(block),
                        Vec::new(),
                        Vec::new(),
                        KEY,
                        KeySet {
                            keys: vec![row(KEY, other, KeyKind::Data)],
                            wrapped: Vec::new(),
                        },
                        false,
                    )
                    .await,
                ),
            ),
            (
                "set-metadata",
                answered(client::blocks::set_metadata(peer, block, sealed(), Vec::new()).await),
            ),
            (
                "publish",
                answered(
                    client::blocks::publish(
                        peer,
                        block,
                        commit,
                        None,
                        Vec::new(),
                        objects_change(&[], Vec::new()),
                        1_000,
                        false,
                        listing(&[], Vec::new()),
                        Vec::new(),
                        vec![b"an object".to_vec()],
                    )
                    .await,
                ),
            ),
            (
                "read-contents",
                answered(client::blocks::read_contents(peer, vec![block], Vec::new()).await),
            ),
            (
                "set-parent",
                answered(
                    client::blocks::set_parent(peer, block, BlockParent::Root, KeySet::default())
                        .await,
                ),
            ),
            (
                "list-blocks",
                answered(
                    client::blocks::list_blocks(peer, Listing::Children(BlockParent::Root)).await,
                ),
            ),
            (
                "unlist",
                answered(client::blocks::unlist(peer, Listing::Children(BlockParent::Root)).await),
            ),
            (
                "list-history",
                answered(client::blocks::list_history(peer, block).await),
            ),
            (
                "changed-since",
                answered(
                    client::blocks::changed_since(
                        peer,
                        block,
                        ChangeCursor {
                            changed: 0,
                            block: Uuid::max(),
                        },
                        10,
                    )
                    .await,
                ),
            ),
            (
                "collect-detached",
                answered(client::blocks::collect_detached(peer).await),
            ),
            (
                "set-access",
                answered(client::blocks::set_access(peer, block, other, Access::Manage).await),
            ),
            (
                "list-access",
                answered(client::blocks::list_access(peer, block).await),
            ),
            (
                "get-keys",
                answered(client::keys::get_keys(peer, Some(vec![block])).await),
            ),
            (
                "put-keys",
                answered(client::keys::put_keys(peer, KeySet::default()).await),
            ),
            (
                "get-recovery-key",
                answered(client::keys::get_recovery_key(peer).await),
            ),
            (
                "set-recovery-key",
                answered(
                    client::keys::set_recovery_key(
                        peer,
                        [7; 32],
                        [7; 32],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                    .await,
                ),
            ),
            (
                "get-signing-keys",
                answered(client::keys::get_signing_keys(peer, vec![other]).await),
            ),
            (
                "reseal-keys",
                answered(client::keys::reseal_keys(peer, Vec::new()).await),
            ),
            (
                "rotate",
                answered(client::keys::rotate(peer, vec![block], KeySet::default()).await),
            ),
            (
                "pin-escrow",
                answered(client::keys::pin_escrow(peer, sealed()).await),
            ),
            (
                "push-key",
                answered(client::notifications::push_key(peer).await),
            ),
            (
                "offers-fcm",
                answered(client::notifications::offers_fcm(peer).await),
            ),
            (
                "register-endpoint",
                answered(
                    client::notifications::register_endpoint(
                        peer,
                        be_block_protocol::PushEndpoint {
                            service: be_block_protocol::PushService::WebPush,
                            address: "https://fcm.googleapis.com/fcm/send/endpoint".into(),
                            p256dh: vec![4; 65],
                            auth: vec![0; 16],
                        },
                    )
                    .await,
                ),
            ),
            (
                "unregister-endpoint",
                answered(
                    client::notifications::unregister_endpoint(
                        peer,
                        "https://fcm.googleapis.com/fcm/send/endpoint".into(),
                    )
                    .await,
                ),
            ),
            (
                "notify-modes",
                answered(client::notifications::notify_modes(peer).await),
            ),
            (
                "set-notify-mode",
                answered(
                    client::notifications::set_notify_mode(
                        peer,
                        block,
                        be_block_protocol::NotifyMode::Off,
                    )
                    .await,
                ),
            ),
            (
                "notify",
                answered(
                    client::notifications::notify(peer, block, sealed(), Vec::new(), Vec::new())
                        .await,
                ),
            ),
        ];
        let escrow = be_block_protocol::EscrowKey {
            id: other,
            epoch: 1,
            public: [1; 32],
            signing: [2; 32],
            signature: Vec::new(),
        };
        answers.push((
            "create-escrow",
            answered(client::keys::create_escrow(peer, escrow, KeySet::default()).await),
        ));
        for (name, stream) in [
            ("changes", Opened::Changes(client::blocks::changes(peer))),
            ("paired", Opened::Paired(client::account::paired(peer))),
            (
                "storage-changed",
                Opened::Storage(client::account::storage_changed(peer)),
            ),
            (
                "contacts-changed",
                Opened::Unit(client::contacts::contacts_changed(peer)),
            ),
            (
                "key-work-changed",
                Opened::KeyWork(client::keys::key_work_changed(peer)),
            ),
            (
                "notify-modes-changed",
                Opened::Unit(client::notifications::notify_modes_changed(peer)),
            ),
            ("watch", Opened::Watch(client::blocks::watch(peer, block))),
        ] {
            let answer = match stream {
                Opened::Changes(stream) => self.opened(stream).await,
                Opened::Paired(stream) => self.opened(stream).await,
                Opened::Storage(stream) => self.opened(stream).await,
                Opened::Unit(stream) => self.opened(stream).await,
                Opened::KeyWork(stream) => self.opened(stream).await,
                Opened::Watch(stream) => self.opened(stream).await,
            };
            answers.push((name, answer));
        }
        let session = client::sessions::Session::new(peer, block);
        let mut events = session.join();
        let joined = match events.recv().await {
            Some(SessionEvent::Joined { .. }) => Answered::Allowed,
            Some(SessionEvent::Refused(error)) => Answered::Refused(error.code),
            None => Answered::Denied,
            Some(other) => panic!("joining answered {other:?}"),
        };
        answers.push(("session-join", joined));
        answers.push((
            "session-claim-ownership",
            answered(session.claim_ownership(1).await),
        ));
        answers.push((
            "session-heartbeat",
            answered(session.heartbeat(1, None).await),
        ));
        answers.push((
            "session-relay",
            answered(session.relay(None, sealed()).await),
        ));
        answers.push((
            "session-append",
            answered(
                session
                    .append(
                        1,
                        be_block_store::Hash::from_bytes([0; 32]),
                        vec![be_block_protocol::JournalEntry {
                            generation: 1,
                            sequence: 1,
                            payload: sealed(),
                        }],
                        true,
                    )
                    .await,
            ),
        ));
        answers.push((
            "session-rebase-journal",
            answered(
                session
                    .rebase_journal(
                        1,
                        be_block_store::Hash::from_bytes([0; 32]),
                        be_block_protocol::JournalBase {
                            commit: None,
                            sequence: 0,
                            state: Vec::new(),
                        },
                    )
                    .await,
            ),
        ));
        answers.push(("logout", answered(client::account::logout(peer).await)));
        answers
    }

    fn assert_called_everything(&self) {
        let started = self.started.lock().unwrap();
        let missed: Vec<&str> = METHODS
            .iter()
            .filter(|(method, _)| !started.contains(method))
            .map(|(_, name)| *name)
            .collect();
        assert!(
            missed.is_empty(),
            "give these calls a rule in client.rs and make them in this test: {missed:?}"
        );
    }
}

enum Opened {
    Changes(Stream<BlockChange>),
    Paired(Stream<PairingMessage>),
    Storage(Stream<be_block_protocol::AccountStorage>),
    Unit(Stream<()>),
    KeyWork(Stream<be_block_protocol::KeyWork>),
    Watch(Stream<BlockEvent>),
}

#[tokio::test]
async fn every_call_states_who_may_make_it() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let before = owner.read_block(block).await;

    let mut stranger = harness.client().await;
    let answers = stranger.everything(block, organization).await;
    let names: BTreeSet<&str> = answers.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), answers.len(), "a call was made twice");
    // Its first call (after a ping, which the socket answers) did not sign it in, so the
    // connection was closed: nothing answered it.
    for (name, answer) in &answers {
        if *name == "ping" {
            continue;
        }
        assert!(
            matches!(
                answer,
                Answered::Denied | Answered::Refused(ErrorCode::NotAuthenticated)
            ),
            "{name} answered a connection that is not signed in: {answer:?}"
        );
    }

    let mut knower = harness.client().await;
    let account = knower.register("knower@example.com").await;
    assert_eq!(knower.personal().await, organization);
    let referrer = Uuid::new_v4();
    let key = Uuid::new_v4();
    let response = knower
        .call(|peer| {
            client::blocks::create_block(
                peer,
                referrer,
                CONTENT,
                BlockParent::Root,
                Vec::new(),
                vec![block],
                key,
                KeySet {
                    keys: vec![row(key, referrer, KeyKind::Data)],
                    wrapped: Vec::new(),
                },
                false,
            )
        })
        .await;
    assert!(response.is_ok(), "{response:?}");
    assert_eq!(knower.read_block(block).await.access, Access::KnowExists);
    let answers = knower.everything(block, organization).await;
    knower.assert_called_everything();
    let viewing = [
        "hold-objects",
        "create-block",
        "set-metadata",
        "publish",
        "read-contents",
        "set-parent",
        "list-history",
        "changed-since",
        "list-access",
        "set-access",
        "rotate",
        "watch",
        "session-join",
        "session-claim-ownership",
        "session-heartbeat",
        "session-relay",
        "session-append",
        "session-rebase-journal",
        "set-notify-mode",
        "notify",
    ];
    for (name, answer) in &answers {
        if viewing.contains(name) {
            assert!(
                matches!(
                    answer,
                    Answered::Denied | Answered::Refused(ErrorCode::PermissionDenied)
                ),
                "{name} answered an account that only knows the block exists: {answer:?}"
            );
        }
    }
    let after = owner.read_block(block).await;
    assert_eq!(
        (after.head, after.parent, after.metadata, after.version),
        (before.head, before.parent, before.metadata, before.version),
        "an account that only knows the block exists changed it"
    );
    let _ = account;

    harness.stop().await;
}
