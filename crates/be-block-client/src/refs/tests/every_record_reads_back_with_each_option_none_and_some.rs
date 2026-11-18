use be_block_commit::CommitId;
use be_block_graph::{Access, BlockParent};
use be_block_protocol::{BlockSummary, EscrowKey, JournalBase, JournalEntry, KeySet};
use be_block_store::Hash;
use uuid::Uuid;

use super::super::announcements::{Made, Until, Waiting};
use super::*;

fn commit(seed: &[u8]) -> CommitId {
    CommitId::from_hash(Hash::of(seed))
}

fn escrow() -> EscrowKey {
    EscrowKey {
        id: Uuid::new_v4(),
        epoch: 3,
        public: [1; 32],
        head: [4; 32],
        maker: Uuid::new_v4(),
        signature: vec![3; 64],
    }
}

/// Every record the refs database keeps reads back as it was written, with each optional field
/// absent and present. Records are postcard, which is not self-describing: a field written as
/// nothing (`skip_serializing_if`) never reads back, and `default` changes nothing.
#[test]
fn every_record_reads_back_with_each_option_none_and_some() {
    let block = Uuid::new_v4();
    for some in [false, true] {
        reads_back(&Session {
            role: OrganizationRole::Editor,
            kind: OrganizationKind::Personal,
            escrows: vec![escrow()],
            pinned: crate::contacts::Pinned {
                root: some.then_some([4; 32]),
                escrow: some.then_some(escrow()),
                made: 0,
            },
            server: some.then_some(be_block_protocol::ServerAccount {
                account: Uuid::new_v4(),
                public: [4; 32],
            }),
            server_offset: some.then_some(-17),
        });
        let pending = Pending {
            block,
            commit: commit(b"commit"),
            expected: some.then_some(commit(b"expected")),
            parents: vec![commit(b"parent")],
            chunks: vec![Hash::of(b"chunk")],
            time: 5,
            pinned: some,
            references: vec![Uuid::new_v4()],
            metadata: vec![6],
            journal: some.then_some(be_block_protocol::JournalSeal {
                tag: Hash::of(b"build"),
                run: 3,
                base: be_block_protocol::JournalBase {
                    commit: Some(commit(b"commit")),
                    sequence: 9,
                    state: vec![10],
                },
            }),
        };
        let creation = Creation {
            block,
            content_type: Uuid::new_v4(),
            parent: BlockParent::Root,
            metadata: vec![7],
            references: vec![Uuid::new_v4()],
            key: Uuid::new_v4(),
            keys: KeySet {
                keys: Vec::new(),
                wrapped: Vec::new(),
            },
            owner_only: some,
            root: vec![8],
            placement: vec![9],
            unchecked: some,
        };
        let moving = Moving {
            block,
            parent: BlockParent::Block(Uuid::new_v4()),
            from: some.then_some(BlockParent::Detached),
        };
        let defining = Defining {
            block,
            metadata: vec![8],
            references: Vec::new(),
        };
        for op in [
            Op::Publish(pending.clone()),
            Op::Create(creation.clone()),
            Op::Move(moving.clone()),
            Op::Define(defining.clone()),
            Op::Append(Appending {
                block,
                id: Uuid::new_v4(),
                payload: vec![11],
                signature: vec![12; 64],
            }),
        ] {
            reads_back(&op);
        }
        let mut frontier = be_block_keys::merkle::Frontier::default();
        frontier.push([13; 32]);
        let creator = be_block_keys::RecoveryPhrase::generate().secret();
        let root = be_block_keys::Entry::root(
            &creator,
            [1; 16],
            be_block_keys::LogKind::Block,
            [2; 16],
        );
        reads_back(&KeptLog {
            head: root.hash(),
            place: 0,
            lowering_place: 0,
            current: vec![root.hash()],
            levels: vec![([1; 16], be_block_keys::Level::Manage)],
            vouched: vec![([3; 16], [4; 16], [5; 32])],
            checked: some.then(|| {
                be_block_keys::check_log(
                    be_block_keys::LogKind::Block,
                    [2; 16],
                    root.hash(),
                    &[root.clone()],
                    &|_| None,
                )
                .unwrap()
            }),
        });
        reads_back(&KeptLogHead {
            size: 1,
            root: [13; 32],
            frontier: some.then_some(frontier),
        });
        reads_back(&be_block_protocol::LogEntry {
            seq: 3,
            id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            arrival: 1_000,
            payload_hash: Hash::of(b"payload"),
            signature: vec![14; 64],
            payload: some.then(|| vec![15]),
        });
        reads_back(&SetAside {
            id: Uuid::new_v4(),
            count: 2,
            kept: some.then(|| OpenEdits {
                origin: 1,
                given_up: 2,
                submitted_through: 3,
                restarts: 4,
                edits: vec![vec![5]],
                submissions: vec![vec![6]],
            }),
        });
        reads_back(&Unsent {
            head: some.then_some(commit(b"head")),
            since: 9,
            ops: vec![Op::Publish(pending)],
            refused: some.then_some(crate::refs::Aside::Refused(ErrorCode::OverQuota)),
        });
        reads_back(&some.then_some(ListedBlock {
            root: Uuid::new_v4(),
            content_type: Uuid::new_v4(),
            head: some.then_some(commit(b"listed")),
        }));
        reads_back(&vec![KnownSigner {
            key: [5; 32],
            verified: some,
            replaced: some.then_some(7),
            superseded: some,
        }]);
        reads_back(&JournalHead {
            tag: Hash::of(b"tag"),
            base: JournalBase {
                commit: some.then_some(commit(b"base")),
                sequence: 4,
                state: vec![9],
            },
            order: be_block_session::Sequencer {
                sequence: 5,
                barrier: 3,
            },
        });
        reads_back(&JournalEntry {
            sequence: 5,
            origin: 7,
            counter: 8,
            base: 4,
            floor: 3,
            received: some.then_some(1_000),
            payload: vec![10],
        });
        for until in [
            Until::Submitted,
            Until::Journal(5),
            Until::Commit(commit(b"kept")),
            Until::Held,
            Until::Sent,
        ] {
            reads_back(&Waiting {
                order: 11,
                until,
                made: Made {
                    key: "key".to_owned(),
                    sealed: vec![12],
                    several: "several".to_owned(),
                    mentions: vec![Uuid::new_v4()],
                },
            });
        }
        let mut summary = BlockSummary::no_access(block, Uuid::new_v4(), Access::View);
        if some {
            summary.author = Some(Uuid::new_v4());
            summary.parent = Some(BlockParent::Root);
            summary.head = Some(commit(b"summary"));
            summary.version = Some(13);
            summary.key = Some(Uuid::new_v4());
            summary.scope = Some(Uuid::new_v4());
        }
        reads_back(&summary);
    }
}
