use std::collections::BTreeSet;

use be_block_graph::Access;
use be_block_protocol::{
    ChangeCursor, LinkKind, LogRecord, Page, PushEndpointChange, PutKeys, WriteError,
};

use super::*;

#[derive(Debug, PartialEq)]
pub(super) enum Answered {
    Allowed,
    Refused(ErrorCode),
    /// A stream the server ended at once, or a call on a connection it closed.
    Denied,
}

fn answered<T, F>(response: Result<Result<T, Error>, F>) -> Answered {
    match response {
        Ok(Ok(_)) => Answered::Allowed,
        Ok(Err(error)) => Answered::Refused(error.code),
        Err(_) => Answered::Denied,
    }
}

fn written<F>(response: Result<Result<Vec<be_block_protocol::Done>, WriteError>, F>) -> Answered {
    answered(response.map(|written| written.map_err(|refused| refused.error)))
}

/// The calls besides `read`, `write`, `send` and `unfollow`, each made once.
const CALLS: &[&str] = &[
    "ping",
    "device-login (begin)",
    "device-login (poll)",
    "events",
];

/// The name of a query's case. Its match names every case, so a new one does not compile until
/// it is named here; `QUERIES`, beside it, lists every name it answers, and the test fails until
/// `everything` makes each one.
fn query_case(query: &Query) -> &'static str {
    match query {
        Query::Blocks {
            contents: false, ..
        } => "blocks",
        Query::Blocks { contents: true, .. } => "blocks (contents)",
        Query::List { .. } => "list",
        Query::Tree { .. } => "tree",
        Query::History(_) => "history",
        Query::Log(Some(_)) => "log (block)",
        Query::Log(None) => "log (organization)",
        Query::Access(_) => "access",
        Query::Keys { .. } => "keys",
        Query::KeyWork => "key-work",
        Query::Revocations { .. } => "revocations",
        Query::RecoveryKey => "recovery-key",
        Query::SigningKeys(_) => "signing-keys",
        Query::Members => "members",
        Query::Organizations => "organizations",
        Query::Invitations => "invitations",
        Query::Storage => "storage",
        Query::Mailbox => "mailbox",
        Query::Link(_) => "link",
        Query::Invitation(_) => "invitation",
        Query::DownloadUrls(_) => "download-urls",
        Query::ServerInfo => "server-info",
        Query::Mutes => "mutes",
        Query::Session { .. } => "session",
    }
}

const QUERIES: &[&str] = &[
    "blocks",
    "blocks (contents)",
    "list",
    "tree",
    "history",
    "log (block)",
    "log (organization)",
    "access",
    "keys",
    "key-work",
    "revocations",
    "recovery-key",
    "signing-keys",
    "members",
    "organizations",
    "invitations",
    "storage",
    "mailbox",
    "link",
    "invitation",
    "download-urls",
    "server-info",
    "mutes",
    "session",
];

/// The name of an op's case, as `query_case` names a query's.
fn op_case(op: &Op) -> &'static str {
    match op {
        Op::CreateBlock { .. } => "create-block",
        Op::Publish { commit: None, .. } => "publish (no commit)",
        Op::Publish {
            commit: Some(_), ..
        } => "publish",
        Op::Move { .. } => "move",
        Op::AppendLog(LogRecord::Grant { .. }) => "append-log (grant)",
        Op::AppendLog(LogRecord::Role { .. }) => "append-log (role)",
        Op::AppendLog(LogRecord::Member { .. }) => "append-log (member)",
        Op::DeleteAgent { .. } => "delete-agent",
        Op::PutKeys(PutKeys::Rows(_)) => "put-keys (rows)",
        Op::PutKeys(PutKeys::Rotate { .. }) => "put-keys (rotate)",
        Op::PutKeys(PutKeys::Escrow { .. }) => "put-keys (escrow)",
        Op::PutKeys(PutKeys::Reseal(_)) => "put-keys (reseal)",
        Op::HoldObjects { .. } => "hold-objects",
        Op::ReserveUploads(_) => "reserve-uploads",
        Op::CreateOrganization { .. } => "create-organization",
        Op::CreateAgent { .. } => "create-agent",
        Op::RotateAgentToken(_) => "rotate-agent-token",
        Op::CreateLink {
            kind: LinkKind::Contact(_),
            ..
        } => "create-link (contact)",
        Op::CreateLink {
            kind: LinkKind::Invitation(_),
            ..
        } => "create-link (invitation)",
        Op::AcceptLink { .. } => "accept-link",
        Op::DeleteLink { .. } => "delete-link",
        Op::RemoveContact { .. } => "remove-contact",
        Op::SetRecoveryKey { .. } => "set-recovery-key",
        Op::SetPushEndpoint(PushEndpointChange::Register(_)) => "set-push-endpoint (register)",
        Op::SetPushEndpoint(PushEndpointChange::Forget(_)) => "set-push-endpoint (forget)",
        Op::SetMute { .. } => "set-mute",
        Op::DeleteMessage(_) => "delete-message",
        Op::RevokeToken(_) => "revoke-token",
    }
}

const OPS: &[&str] = &[
    "create-block",
    "publish",
    "publish (no commit)",
    "move",
    "append-log (grant)",
    "append-log (role)",
    "append-log (member)",
    "delete-agent",
    "put-keys (rows)",
    "put-keys (rotate)",
    "put-keys (escrow)",
    "put-keys (reseal)",
    "hold-objects",
    "reserve-uploads",
    "create-organization",
    "create-agent",
    "rotate-agent-token",
    "create-link (contact)",
    "create-link (invitation)",
    "accept-link",
    "delete-link",
    "remove-contact",
    "set-recovery-key",
    "set-push-endpoint (register)",
    "set-push-endpoint (forget)",
    "set-mute",
    "delete-message",
    "revoke-token",
];

/// The name of a send's target, as `query_case` names a query's.
fn target_case(target: &Target) -> &'static str {
    match target {
        Target::Session {
            kind: SessionSend::Ordered(_),
            ..
        } => "send session (ordered)",
        Target::Session {
            kind: SessionSend::Ephemeral(_),
            ..
        } => "send session (ephemeral)",
        Target::Session {
            kind: SessionSend::Claim,
            ..
        } => "send session (claim)",
        Target::Session {
            kind: SessionSend::Host(_),
            ..
        } => "send session (host)",
        Target::Devices(_) => "send devices",
        Target::Push { .. } => "send push",
    }
}

const TARGETS: &[&str] = &[
    "send session (ordered)",
    "send session (ephemeral)",
    "send session (claim)",
    "send session (host)",
    "send devices",
    "send push",
];

/// The name of an unfollow of a topic, as `query_case` names a query's.
fn topic_case(topic: &Topic) -> &'static str {
    match topic {
        Topic::Tree(_) => "unfollow tree",
        Topic::Session(_) => "unfollow session",
        Topic::Keys => "unfollow keys",
        Topic::KeyWork => "unfollow key-work",
        Topic::Roots => "unfollow roots",
        Topic::Storage => "unfollow storage",
        Topic::Mailbox => "unfollow mailbox",
        Topic::Mutes => "unfollow mutes",
    }
}

const TOPICS: &[&str] = &[
    "unfollow tree",
    "unfollow session",
    "unfollow keys",
    "unfollow key-work",
    "unfollow roots",
    "unfollow storage",
    "unfollow mailbox",
    "unfollow mutes",
];

/// The calls, queries, ops, targets and unfollows `everything` makes, by name.
fn every_case() -> BTreeSet<&'static str> {
    [CALLS, QUERIES, OPS, TARGETS, TOPICS]
        .concat()
        .into_iter()
        .collect()
}

/// Makes calls on one connection, naming the organization each case's rule asks for.
struct Everything<'a> {
    peer: &'a At,
    answers: Vec<(&'static str, Answered)>,
}

impl Everything<'_> {
    async fn read(&mut self, query: Query) {
        let name = query_case(&query);
        assert!(QUERIES.contains(&name), "list {name} in QUERIES");
        let organization = self
            .peer
            .organization_for(crate::client::query_rule(&query));
        let answer: Result<Result<Page, Error>, _> =
            client::server::read(self.peer, organization, query, false).await;
        self.answers.push((name, answered(answer)));
    }

    async fn write(&mut self, op: Op) {
        let name = op_case(&op);
        assert!(OPS.contains(&name), "list {name} in OPS");
        let organization = self.peer.op_organization(&op);
        let answer = client::server::write(self.peer, organization, vec![op]).await;
        self.answers.push((name, written(answer)));
    }

    async fn send(&mut self, target: Target, payload: Vec<u8>) {
        let name = target_case(&target);
        assert!(TARGETS.contains(&name), "list {name} in TARGETS");
        let answer = self.peer.send(target, payload).await;
        self.answers.push((name, answered(answer)));
    }

    async fn unfollow(&mut self, topic: Topic) {
        let name = topic_case(&topic);
        assert!(TOPICS.contains(&name), "list {name} in TOPICS");
        let answer = self.peer.unfollow(topic).await;
        self.answers.push((name, answered(answer)));
    }
}

impl TestClient {
    /// Makes every call, query, op, send and unfollow of `be3:server`, each case that names a
    /// block on `block` and the rest with arguments that would do something were the caller
    /// allowed to, in `organization`; answers how each was answered, by name.
    pub(super) async fn everything(
        &mut self,
        block: Uuid,
        organization: Uuid,
    ) -> Vec<(&'static str, Answered)> {
        let mut at = self.peer.clone();
        at.organization = Some(organization);
        let other = Uuid::new_v4();
        let hash = Hash::of(b"an object");
        let commit = CommitId::from_hash(hash);
        let sealed = || b"sealed".to_vec();
        let root = self.account.map_or_else(Vec::new, |_| self.root(other));
        let placement = self.placement(other, BlockParent::Block(block)).await;
        let moving = self.signed_parent(block, BlockParent::Root).await;
        let (new_organization, new_root) = organization_root(self.account.unwrap_or_default());
        let mut calls = Everything {
            peer: &at,
            answers: Vec::new(),
        };
        let pinged = match client::server::ping(&at).await {
            Ok(()) => Answered::Allowed,
            Err(_) => Answered::Denied,
        };
        calls.answers.push(("ping", pinged));
        calls
            .read(Query::List {
                listing: Listing::Children(BlockParent::Root),
                after: None,
            })
            .await;
        let begun = client::server::device_login(&at, DeviceLoginStep::Begin).await;
        calls
            .answers
            .push(("device-login (begin)", answered(begun)));
        let polled = client::server::device_login(
            &at,
            DeviceLoginStep::Poll {
                code: "code".into(),
                secret: "secret".into(),
            },
        )
        .await;
        calls
            .answers
            .push(("device-login (poll)", answered(polled)));
        calls.send(Target::Devices(None), sealed()).await;
        calls.read(Query::Mailbox).await;
        calls.write(Op::DeleteMessage(1)).await;
        calls.read(Query::Organizations).await;
        calls
            .write(Op::CreateOrganization {
                id: new_organization,
                name: "mine".into(),
                root: new_root,
                pinned: Vec::new(),
            })
            .await;
        calls
            .write(Op::CreateLink {
                link: other,
                card: sealed(),
                kind: LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: sealed(),
                }),
                signed: None,
            })
            .await;
        calls.read(Query::Members).await;
        calls
            .write(Op::AppendLog(LogRecord::Role {
                account: other,
                role: OrganizationRole::Administrator,
                signed: None,
            }))
            .await;
        calls
            .write(Op::DeleteAgent {
                account: other,
                signed: None,
                revocations: Vec::new(),
            })
            .await;
        calls
            .read(Query::Revocations {
                account: other,
                contact: false,
            })
            .await;
        calls
            .write(Op::AppendLog(LogRecord::Member {
                signed: sealed(),
                revocations: Vec::new(),
            }))
            .await;
        calls
            .write(Op::CreateAgent {
                account: Uuid::new_v4(),
                name: "agent".into(),
                recovery: [7; 32],
                signing: [7; 32],
                contact: sealed(),
                record: sealed(),
                escrow: sealed(),
                bound_to: None,
                member: None,
            })
            .await;
        calls.write(Op::RotateAgentToken(other)).await;
        calls
            .write(Op::CreateLink {
                link: other,
                card: sealed(),
                kind: LinkKind::Contact(None),
                signed: None,
            })
            .await;
        calls.read(Query::Link(other)).await;
        calls.read(Query::Invitation(other)).await;
        calls.read(Query::Invitations).await;
        calls
            .write(Op::AcceptLink {
                link: other,
                card: sealed(),
                contact: sealed(),
                key: sealed(),
                member: None,
            })
            .await;
        calls
            .write(Op::DeleteLink {
                link: other,
                signed: None,
            })
            .await;
        calls
            .write(Op::RemoveContact {
                account: other,
                revocations: Vec::new(),
            })
            .await;
        calls
            .write(Op::ReserveUploads(vec![be_block_protocol::ObjectLength {
                hash,
                length: Some(9),
            }]))
            .await;
        calls.read(Query::DownloadUrls(vec![hash])).await;
        calls
            .write(Op::HoldObjects {
                block,
                objects: vec![hash],
                hold: Hold::Lasting,
            })
            .await;
        calls
            .write(Op::CreateBlock {
                block: other,
                content_type: CONTENT,
                parent: BlockParent::Block(block),
                metadata: Vec::new(),
                references: Vec::new(),
                key: KEY,
                owner_only: false,
                root,
                placement,
            })
            .await;
        calls
            .write(Op::Publish {
                block: block,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
                metadata: sealed(),
                journal: None,
            })
            .await;
        calls
            .write(Op::Publish {
                block: block,
                expected: None,
                commit: Some(be_block_protocol::NewCommit {
                    id: commit,
                    parents: Vec::new(),
                    held: objects_change(&[], Vec::new()),
                    time: 1_000,
                    pinned: false,
                    objects: vec![b"an object".to_vec()],
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
            .await;
        calls
            .read(Query::Blocks {
                blocks: vec![block],
                contents: true,
                held: Vec::new(),
            })
            .await;
        calls
            .read(Query::Blocks {
                blocks: vec![block],
                contents: false,
                held: Vec::new(),
            })
            .await;
        calls
            .write(Op::Move {
                block,
                parent: BlockParent::Root,
                placement: moving,
            })
            .await;
        calls.read(Query::Log(None)).await;
        calls.read(Query::History(block)).await;
        calls.read(Query::Log(Some(block))).await;
        calls
            .read(Query::Tree {
                entry: block,
                after: ChangeCursor {
                    changed: 0,
                    block: Uuid::max(),
                },
                limit: 10,
            })
            .await;
        calls
            .write(Op::AppendLog(LogRecord::Grant {
                block,
                account: other,
                access: Access::Manage,
                signed: None,
            }))
            .await;
        calls.read(Query::Access(block)).await;
        calls
            .read(Query::Keys {
                blocks: Some(vec![block]),
                after: None,
            })
            .await;
        calls.read(Query::KeyWork).await;
        calls
            .write(Op::PutKeys(PutKeys::Rows(KeySet::default())))
            .await;
        calls.read(Query::RecoveryKey).await;
        calls
            .write(Op::SetRecoveryKey {
                public: [7; 32],
                signing: [7; 32],
                sealed: Vec::new(),
                changes: Vec::new(),
                retirements: Vec::new(),
            })
            .await;
        calls.read(Query::SigningKeys(vec![other])).await;
        calls.write(Op::PutKeys(PutKeys::Reseal(Vec::new()))).await;
        calls
            .write(Op::PutKeys(PutKeys::Rotate {
                roots: vec![block],
                keys: KeySet::default(),
            }))
            .await;
        calls.read(Query::ServerInfo).await;
        calls.read(Query::Storage).await;
        let endpoint = "https://fcm.googleapis.com/fcm/send/endpoint";
        calls
            .write(Op::SetPushEndpoint(PushEndpointChange::Register(
                be_block_protocol::PushEndpoint {
                    service: be_block_protocol::PushService::WebPush,
                    address: endpoint.into(),
                    p256dh: vec![4; 65],
                    auth: vec![0; 16],
                },
            )))
            .await;
        calls
            .write(Op::SetPushEndpoint(PushEndpointChange::Forget(
                endpoint.into(),
            )))
            .await;
        calls.read(Query::Mutes).await;
        calls
            .write(Op::SetMute {
                block,
                mode: be_block_protocol::NotifyMode::Off,
            })
            .await;
        calls
            .send(
                Target::Push {
                    block,
                    mentions: Vec::new(),
                    keys: Vec::new(),
                },
                sealed(),
            )
            .await;
        calls
            .write(Op::PutKeys(PutKeys::Escrow {
                escrow: be_block_protocol::EscrowKey {
                    id: other,
                    epoch: 1,
                    public: [1; 32],
                    head: [0; 32],
                    maker: Uuid::nil(),
                    signature: Vec::new(),
                },
                keys: KeySet::default(),
            }))
            .await;
        calls
            .read(Query::Session {
                block,
                tag: TYPE_BUILD,
            })
            .await;
        calls
            .send(
                Target::Session {
                    block,
                    kind: SessionSend::Claim,
                },
                Vec::new(),
            )
            .await;
        calls
            .send(
                Target::Session {
                    block,
                    kind: SessionSend::Ephemeral(None),
                },
                sealed(),
            )
            .await;
        calls
            .send(
                Target::Session {
                    block,
                    kind: SessionSend::Host(None),
                },
                sealed(),
            )
            .await;
        calls
            .send(
                Target::Session {
                    block,
                    kind: SessionSend::Ordered(be_block_protocol::Submission {
                        origin: 1,
                        counter: 1,
                        base: 0,
                        barrier: false,
                    }),
                },
                sealed(),
            )
            .await;
        for topic in [
            Topic::Tree(block),
            Topic::Session(block),
            Topic::Keys,
            Topic::KeyWork,
            Topic::Roots,
            Topic::Storage,
            Topic::Mailbox,
            Topic::Mutes,
        ] {
            calls.unfollow(topic).await;
        }
        let mut answers = calls.answers;
        // A second events stream replaces the first; one refused ends at once.
        let mut events = client::server::events(&at);
        let _ = at
            .read::<Vec<be_block_protocol::Organization>>(Query::Organizations)
            .await;
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let opened = match events.poll_recv(&mut context) {
            std::task::Poll::Ready(None) => Answered::Denied,
            _ => Answered::Allowed,
        };
        answers.push(("events", opened));
        let mut calls = Everything { peer: &at, answers };
        calls.write(Op::RevokeToken(None)).await;
        calls.answers
    }
}

#[tokio::test]
async fn every_call_states_who_may_make_it() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let before = owner.read_block(block).await;

    let mut socket = greet(&harness.socket(), BUILD, Some("not a token")).await;
    next_binary(&mut socket)
        .await
        .expect("the server's greeting refuses the token");
    let mut stranger = TestClient::over(socket);
    let answers = stranger.everything(block, organization).await;
    let names: BTreeSet<&str> = answers.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), answers.len(), "a case was made twice");
    let missed: Vec<&str> = every_case().difference(&names).copied().collect();
    assert!(
        missed.is_empty(),
        "give these cases a rule in client.rs and make them in `everything`: {missed:?}"
    );
    // Its greeting did not sign it in, so the connection was closed before any call: nothing
    // answered it.
    for (name, answer) in &answers {
        assert!(
            matches!(
                answer,
                Answered::Denied | Answered::Refused(ErrorCode::NotAuthenticated)
            ),
            "{name} answered a connection that is not signed in: {answer:?}"
        );
    }

    let mut knower = harness.member("knower@example.com").await;
    assert_eq!(knower.personal().await, organization);
    let referrer = Uuid::new_v4();
    let key = Uuid::new_v4();
    let root = knower.root(referrer);
    let placement = knower.placement(referrer, BlockParent::Root).await;
    let response = knower
        .call(|peer| {
            peer.write_all::<be_block_protocol::BlockSummary>(vec![
                Op::CreateBlock {
                    block: referrer,
                    content_type: CONTENT,
                    parent: BlockParent::Root,
                    metadata: Vec::new(),
                    references: vec![block],
                    key: key,
                    owner_only: false,
                    root: root,
                    placement: placement,
                },
                Op::PutKeys(be_block_protocol::PutKeys::Rows(KeySet {
                    keys: vec![row(key, referrer, KeyKind::Data)],
                    wrapped: Vec::new(),
                })),
            ])
        })
        .await;
    assert!(response.is_ok(), "{response:?}");
    assert_eq!(knower.read_block(block).await.access, Access::KnowExists);
    let answers = knower.everything(block, organization).await;
    let names: BTreeSet<&str> = answers.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, every_case());
    let viewing = [
        "hold-objects",
        "create-block",
        "publish (no commit)",
        "publish",
        "blocks (contents)",
        "move",
        "history",
        "log (block)",
        "tree",
        "access",
        "append-log (grant)",
        "put-keys (rotate)",
        "session",
        "send session (claim)",
        "send session (ephemeral)",
        "send session (ordered)",
        "set-mute",
        "send push",
    ];
    for name in viewing {
        assert!(names.contains(name), "{name} is not a case");
    }
    // A browser sign-in's steps and reading an invitation are one-shot calls alone, limited per
    // address: a signed-in connection is refused them.
    for (name, answer) in &answers {
        if ["device-login (begin)", "device-login (poll)", "invitation"].contains(name) {
            assert!(
                matches!(answer, Answered::Refused(ErrorCode::PermissionDenied)),
                "{name} answered a signed-in connection: {answer:?}"
            );
        }
    }
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

    harness.stop().await;
}
