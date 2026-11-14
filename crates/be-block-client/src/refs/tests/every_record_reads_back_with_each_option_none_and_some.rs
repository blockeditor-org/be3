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
        signing: [2; 32],
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
            pinned: some.then_some(escrow()),
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
        ] {
            reads_back(&op);
        }
        reads_back(&Unsent {
            head: some.then_some(commit(b"head")),
            since: 9,
            ops: vec![Op::Publish(pending)],
            refused: some.then_some(ErrorCode::OverQuota),
        });
        reads_back(&vec![KnownSigner {
            key: [5; 32],
            verified: some,
        }]);
        reads_back(&JournalHead {
            tag: Hash::of(b"tag"),
            generation: 2,
            base: JournalBase {
                commit: some.then_some(commit(b"base")),
                sequence: 4,
                state: vec![9],
            },
            sealing: some.then_some((commit(b"seal"), 4)),
        });
        reads_back(&JournalEntry {
            generation: 2,
            sequence: 5,
            payload: vec![10],
        });
        for until in [
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
