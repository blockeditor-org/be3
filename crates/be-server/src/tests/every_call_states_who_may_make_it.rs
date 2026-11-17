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
        let _ = self.peer.read::<Vec<be_block_protocol::Organization>>(be_block_protocol::Query::Organizations).await;
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
        let root = self.account.map_or_else(Vec::new, |_| self.root(other));
        let placement = self.placement(other, BlockParent::Block(block)).await;
        let moving = self.signed_parent(block, BlockParent::Root).await;
        let mut answers = vec![
            (
                "ping",
                match be_block_protocol::client::server::ping(peer).await {
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
                "authenticate",
                answered(be_block_protocol::client::server::authenticate(peer, Some("not a token".into())).await),
            ),
            (
                "pair",
                answered(peer.send(be_block_protocol::Target::Devices(None), sealed()).await),
            ),
            (
                "account-notices",
                answered(peer.read::<Vec<be_block_protocol::AccountNotice>>(be_block_protocol::Query::Notices).await),
            ),
            (
                "dismiss-account-notice",
                answered(peer.write::<()>(be_block_protocol::Op::DismissNotice(1)).await),
            ),
            (
                "list-organizations",
                answered(peer.read::<Vec<be_block_protocol::Organization>>(be_block_protocol::Query::Organizations).await),
            ),
            (
                "create-organization",
                answered({
                    let (id, root) = organization_root(self.account.unwrap_or_default());
                    peer.write::<be_block_protocol::Organization>(be_block_protocol::Op::CreateOrganization { id: id, name: "mine".into(), root: root, pinned: Vec::new() })
                    .await
                }),
            ),
            (
                "create-link (invitation)",
                answered(
                    peer.write::<()>(be_block_protocol::Op::CreateLink { link: other, card: sealed(), code: sealed(), kind: be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                            organization,
                            escrow: sealed(),
                        }) })
                    .await,
                ),
            ),
            (
                "list-members",
                answered(peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members).await),
            ),
            (
                "set-role",
                answered(
                    peer.write::<()>(be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Role { account: other, role: OrganizationRole::Administrator, signed: None }))
                        .await,
                ),
            ),
            (
                "remove-member",
                answered(peer.write::<()>(be_block_protocol::Op::RemoveMember { account: other, signed: None, revocations: Vec::new() }).await),
            ),
            (
                "removal-revocations",
                answered(peer.read::<Vec<be_block_protocol::Revocation>>(be_block_protocol::Query::Revocations { account: other, contact: false }).await),
            ),
            (
                "sign-member",
                answered(peer.write::<()>(be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Member(sealed()))).await),
            ),
            (
                "create-agent",
                answered(
                    peer.write_in::<be_block_protocol::CreatedAgent>(Some(organization), be_block_protocol::Op::CreateAgent { account: Uuid::new_v4(), name: "agent".into(), recovery: [7; 32], signing: [7; 32], contact: sealed(), escrow: sealed(), bound_to: None, member: None })
                    .await,
                ),
            ),
            (
                "rotate-agent-token",
                answered(peer.write_in::<String>(Some(organization), be_block_protocol::Op::RotateAgentToken(other)).await),
            ),
            (
                "create-link",
                answered(
                    peer.write::<()>(be_block_protocol::Op::CreateLink { link: other, card: sealed(), code: sealed(), kind: be_block_protocol::LinkKind::Contact(None) })
                    .await,
                ),
            ),
            (
                "get-link",
                answered(peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(other)).await),
            ),
            (
                "read-invitation",
                answered(peer.read::<be_block_protocol::InvitationPreview>(be_block_protocol::Query::Invitation(other)).await),
            ),
            (
                "list-invitations",
                answered(peer.read_in::<Vec<be_block_protocol::InvitationRow>>(Some(organization), be_block_protocol::Query::Invitations).await),
            ),
            (
                "accept-link",
                answered(
                    peer.write::<()>(be_block_protocol::Op::AcceptLink { link: other, card: sealed(), contact: sealed(), key: sealed() }).await,
                ),
            ),
            (
                "list-links",
                answered(peer.read::<Vec<be_block_protocol::OwnLink>>(be_block_protocol::Query::Links).await),
            ),
            (
                "delete-link",
                answered(peer.write::<()>(be_block_protocol::Op::DeleteLink(other)).await),
            ),
            (
                "put-contact",
                answered(peer.write::<()>(be_block_protocol::Op::PutContact { contact: other, sealed: sealed() }).await),
            ),
            (
                "list-contacts",
                answered(peer.read::<Vec<be_block_protocol::ContactRow>>(be_block_protocol::Query::Contacts).await),
            ),
            (
                "remove-contact",
                answered(peer.write::<()>(be_block_protocol::Op::RemoveContact { account: other, revocations: Vec::new() }).await),
            ),
            (
                "upload-urls",
                answered(
                    peer.write::<Vec<Option<be_block_protocol::UploadUrl>>>(be_block_protocol::Op::ReserveUploads(vec![be_block_protocol::ObjectLength {
                            hash,
                            length: Some(9),
                        }]))
                    .await,
                ),
            ),
            (
                "download-urls",
                answered(peer.read::<Vec<Option<String>>>(be_block_protocol::Query::DownloadUrls(vec![hash])).await),
            ),
            (
                "hold-objects",
                answered(
                    peer.write::<()>(be_block_protocol::Op::HoldObjects { block: block, objects: vec![hash], hold: Hold::Lasting }).await,
                ),
            ),
            (
                "create-block",
                answered(
                    peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::CreateBlock { block: other, content_type: CONTENT, parent: BlockParent::Block(block), metadata: Vec::new(), references: Vec::new(), key: KEY, keys: KeySet {
                            keys: vec![row(KEY, other, KeyKind::Data)],
                            wrapped: Vec::new(),
                        }, owner_only: false, root: root, placement: placement })
                    .await,
                ),
            ),
            (
                "set-metadata",
                answered(peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::SetMetadata { block: block, metadata: sealed(), references: Vec::new() }).await),
            ),
            (
                "publish",
                answered(
                    peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: commit, expected: None, parents: Vec::new(), held: objects_change(&[], Vec::new()), time: 1_000, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: vec![b"an object".to_vec()], journal: None })
                    .await,
                ),
            ),
            (
                "read-contents",
                answered(peer.read::<be_block_protocol::Contents>(be_block_protocol::Query::Blocks { blocks: vec![block], contents: true, held: Vec::new() }).await),
            ),
            (
                "read-summaries",
                answered(peer.read::<Vec<Option<be_block_protocol::BlockSummary>>>(be_block_protocol::Query::Blocks { blocks: vec![block], contents: false, held: Vec::new() }).await),
            ),
            (
                "set-parent",
                answered(
                    peer.write::<()>(be_block_protocol::Op::Move { block: block, parent: BlockParent::Root, keys: KeySet::default(), placement: moving })
                    .await,
                ),
            ),
            (
                "list-blocks",
                answered(
                    peer.read::<be_block_protocol::Listed>(be_block_protocol::Query::List { listing: Listing::Children(BlockParent::Root), after: None })
                        .await,
                ),
            ),
            (
                "unfollow",
                answered(peer.unfollow(be_block_protocol::Topic::Tree(block)).await),
            ),
            (
                "get-organization-log",
                answered(peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(None)).await),
            ),
            (
                "list-history",
                answered(peer.read::<Vec<be_block_protocol::HistoryEntry>>(be_block_protocol::Query::History(block)).await),
            ),
            (
                "get-access-log",
                answered(peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(Some(block))).await),
            ),
            (
                "changed-since",
                answered(
                    peer.ask::<be_block_protocol::ChangedPage>(be_block_protocol::Query::Tree { entry: block, after: ChangeCursor {
                            changed: 0,
                            block: Uuid::max(),
                        }, limit: 10 }, false)
                    .await,
                ),
            ),
            (
                "collect-detached",
                answered(client::blocks::collect_detached(peer).await),
            ),
            (
                "set-access",
                answered(
                    peer.write::<()>(be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Grant { block: block, account: other, access: Access::Manage, signed: None })).await,
                ),
            ),
            (
                "list-access",
                answered(peer.read::<be_block_protocol::AccessList>(be_block_protocol::Query::Access(block)).await),
            ),
            (
                "get-keys",
                answered(peer.read::<be_block_protocol::KeyPage>(be_block_protocol::Query::Keys { blocks: Some(vec![block]), after: None }).await),
            ),
            (
                "put-keys",
                answered(peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rows(KeySet::default()))).await),
            ),
            (
                "get-recovery-key",
                answered(peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey).await),
            ),
            (
                "set-recovery-key",
                answered(
                    peer.write::<()>(be_block_protocol::Op::SetRecoveryKey { public: [7; 32], signing: [7; 32], sealed: Vec::new(), own: Vec::new(), changes: Vec::new(), retirements: Vec::new() })
                    .await,
                ),
            ),
            (
                "get-signing-keys",
                answered(peer.read::<Vec<be_block_protocol::SigningKey>>(be_block_protocol::Query::SigningKeys(vec![other])).await),
            ),
            (
                "reseal-keys",
                answered(peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Reseal(Vec::new()))).await),
            ),
            (
                "rotate",
                answered(peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rotate { roots: vec![block], keys: KeySet::default() })).await),
            ),
            (
                "pin-escrow",
                answered(peer.write::<()>(be_block_protocol::Op::PinEscrow(sealed())).await),
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
                    peer.write::<()>(be_block_protocol::Op::SetPushEndpoint(be_block_protocol::PushEndpointChange::Register(be_block_protocol::PushEndpoint {
                            service: be_block_protocol::PushService::WebPush,
                            address: "https://fcm.googleapis.com/fcm/send/endpoint".into(),
                            p256dh: vec![4; 65],
                            auth: vec![0; 16],
                        })))
                    .await,
                ),
            ),
            (
                "unregister-endpoint",
                answered(
                    peer.write::<()>(be_block_protocol::Op::SetPushEndpoint(be_block_protocol::PushEndpointChange::Forget("https://fcm.googleapis.com/fcm/send/endpoint".into())))
                    .await,
                ),
            ),
            (
                "notify-modes",
                answered(peer.read::<Vec<be_block_protocol::BlockNotifyMode>>(be_block_protocol::Query::NotifyModes).await),
            ),
            (
                "set-notify-mode",
                answered(
                    peer.write::<()>(be_block_protocol::Op::SetNotifyMode { block: block, mode: be_block_protocol::NotifyMode::Off })
                    .await,
                ),
            ),
            (
                "notify",
                answered(
                    peer.send(be_block_protocol::Target::Push { block: block, mentions: Vec::new(), keys: Vec::new() }, sealed())
                        .await,
                ),
            ),
        ];
        let escrow = be_block_protocol::EscrowKey {
            id: other,
            epoch: 1,
            public: [1; 32],
            signing: [2; 32],
            head: [0; 32],
            maker: Uuid::nil(),
            signature: Vec::new(),
        };
        answers.push((
            "create-escrow",
            answered(peer.write::<()>(be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Escrow { escrow: escrow, keys: KeySet::default() })).await),
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
                "account-notices-changed",
                Opened::Unit(client::account::account_notices_changed(peer)),
            ),
            (
                "key-work-changed",
                Opened::KeyWork(client::keys::key_work_changed(peer)),
            ),
            (
                "keys-changed",
                Opened::Unit(client::keys::keys_changed(peer)),
            ),
            (
                "notify-modes-changed",
                Opened::Unit(client::notifications::notify_modes_changed(peer)),
            ),
        ] {
            let answer = match stream {
                Opened::Changes(stream) => self.opened(stream).await,
                Opened::Paired(stream) => self.opened(stream).await,
                Opened::Storage(stream) => self.opened(stream).await,
                Opened::Unit(stream) => self.opened(stream).await,
                Opened::KeyWork(stream) => self.opened(stream).await,
            };
            answers.push((name, answer));
        }
        let session = client::sessions::Session::new(peer, block, TYPE_BUILD);
        let mut events = session.join();
        let joined = match events.recv().await {
            Some(SessionEvent::Joined { .. }) => Answered::Allowed,
            Some(SessionEvent::Refused(error)) => Answered::Refused(error.code),
            None => Answered::Denied,
            Some(other) => panic!("joining answered {other:?}"),
        };
        answers.push(("session-join", joined));
        answers.push(("session-claim", answered(session.claim().await)));
        answers.push((
            "session-relay",
            answered(session.relay(None, sealed()).await),
        ));
        answers.push((
            "session-submit",
            answered(session.submit(1, 1, 0, false, sealed()).await),
        ));
        answers.push((
            "logout",
            answered(peer.write::<()>(be_block_protocol::Op::RevokeToken(None)).await),
        ));
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
    Changes(Stream<be_block_protocol::TreeChange>),
    Paired(Stream<PairingMessage>),
    Storage(Stream<be_block_protocol::AccountStorage>),
    Unit(Stream<()>),
    KeyWork(Stream<be_block_protocol::KeyWork>),
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
    let root = knower.root(referrer);
    let placement = knower.placement(referrer, BlockParent::Root).await;
    let response = knower
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::CreateBlock { block: referrer, content_type: CONTENT, parent: BlockParent::Root, metadata: Vec::new(), references: vec![block], key: key, keys: KeySet {
                    keys: vec![row(key, referrer, KeyKind::Data)],
                    wrapped: Vec::new(),
                }, owner_only: false, root: root, placement: placement })
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
        "get-access-log",
        "changed-since",
        "list-access",
        "set-access",
        "rotate",
        "session-join",
        "session-claim",
        "session-relay",
        "session-submit",
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
