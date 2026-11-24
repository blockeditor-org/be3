use be_block_commit::CommitId;
use be_block_content::{
    Along, Announcement, ChildChange, KeptEntry, LogImport, LogItem, LogPage, LogSearch,
    RefusedEntry, SearchDocument, Signer,
};
use be_block_protocol::{Journal, JournalBase, JournalEntry, Participant, SessionState};
use be_block_session::Resume;
use be_block_store::{ChunkRef, Hash, NodeRef, PartRef};
use uuid::Uuid;

use super::*;

/// Each type `with:` maps holds the shape of the WIT type it stands for, so the WIT still
/// describes the wire: a value of it decodes as the type the generator would make, and back.
#[test]
fn every_type_the_wit_maps_onto_the_stacks_has_its_wire_form() {
    let id = |n: u128| Uuid::from_u128(n);
    let hash = |n: u8| Hash::from_bytes([n; 32]);
    let commit = |n: u8| CommitId::from_hash(hash(n));

    agrees::<_, wit::SessionTag>(crate::SessionTag([7; 32]), false);

    let entry = JournalEntry {
        sequence: 1,
        origin: 2,
        counter: 3,
        base: 4,
        floor: 5,
        received: Some(-6),
        payload: vec![7, 8],
    };
    agrees::<_, wit::JournalEntry>(entry.clone(), true);
    let base = JournalBase {
        commit: Some(commit(1)),
        sequence: 9,
        state: vec![10],
    };
    agrees::<_, wit::JournalBase>(base.clone(), true);
    agrees::<_, wit::Journal>(
        Journal {
            tag: hash(2),
            base,
            entries: vec![entry],
            lost: true,
        },
        true,
    );

    let chunk = ChunkRef {
        hash: hash(3),
        length: 11,
    };
    let node = NodeRef {
        hash: hash(4),
        length: 12,
    };
    agrees::<_, wit::ChunkRef>(chunk, true);
    agrees::<_, wit::NodeRef>(node, true);
    for part in [PartRef::Chunk(chunk), PartRef::Chunks(node), PartRef::Node(node)] {
        agrees::<_, wit::PartRef>(part, true);
    }

    for signer in [
        Signer::Verified,
        Signer::Unverified,
        Signer::Changed,
        Signer::Replaced,
        Signer::Bad,
        Signer::Unknown,
        Signer::Redacted,
    ] {
        agrees::<_, wit::Signer>(signer, true);
    }
    let imported = LogImport {
        block: id(1),
        seq: 13,
        importer: id(2),
    };
    agrees::<_, wit::LogImport>(imported, true);
    let item = LogItem {
        seq: 14,
        id: id(3),
        author: id(4),
        arrival: -15,
        payload: Some(vec![16]),
        signer: Signer::Changed,
        imported: Some(imported),
    };
    agrees::<_, wit::LogItem>(item.clone(), true);
    agrees::<_, wit::LogItem>(
        LogItem {
            payload: None,
            imported: None,
            ..item.clone()
        },
        true,
    );
    let kept = KeptEntry {
        id: id(5),
        payload: vec![17],
    };
    let refused = RefusedEntry {
        id: id(6),
        payload: vec![18],
        why: "no".to_owned(),
    };
    agrees::<_, wit::KeptEntry>(kept.clone(), true);
    agrees::<_, wit::RefusedEntry>(refused.clone(), true);
    agrees::<_, wit::LogPage>(
        LogPage {
            size: 19,
            items: vec![item],
            kept: vec![kept],
            refused: vec![refused],
        },
        true,
    );
    for text in [Some("said".to_owned()), None] {
        agrees::<_, wit::LogSearch>(LogSearch { place: 20, text }, true);
    }

    let announcement = Announcement {
        key: "key".to_owned(),
        text: "text".to_owned(),
        several: "{count}".to_owned(),
        mentions: vec![id(7)],
    };
    agrees::<_, wit::Announcement>(announcement.clone(), true);
    agrees::<_, wit::Along>(
        Along {
            announcement: Some(announcement),
            redacts: Some(21),
        },
        true,
    );
    agrees::<_, wit::Along>(Along::default(), true);

    for change in [
        ChildChange::Add(id(8)),
        ChildChange::Delete(id(9)),
        ChildChange::Replace {
            old: id(10),
            new: id(11),
        },
        ChildChange::Reorder {
            block: id(12),
            after: Some(id(13)),
        },
        ChildChange::Reorder {
            block: id(12),
            after: None,
        },
    ] {
        agrees::<_, wit::ChildChange>(change, true);
    }
    agrees::<_, wit::SearchDocument>(
        SearchDocument {
            key: "row".to_owned(),
            source: Some(PartRef::Chunk(chunk)),
            name: Some("name".to_owned()),
            body: None,
        },
        true,
    );

    let participant = Participant {
        client: 22,
        tag: hash(5),
        may_edit: true,
        claimed: Some(23),
    };
    agrees::<_, wit::Participant>(participant.clone(), true);
    agrees::<_, wit::SessionState>(
        SessionState {
            index: 24,
            after: 25,
            participants: vec![participant],
        },
        true,
    );
    // The WIT's resume cases carry a commit, or a record of the same fields, where the Rust
    // cases name their fields: one wire form, under other names.
    for resume in [
        Resume::Empty,
        Resume::UpToDate,
        Resume::FastForward { to: commit(2) },
        Resume::Publish { from: commit(3) },
        Resume::Merge {
            base: None,
            ours: commit(4),
            theirs: commit(5),
        },
        Resume::Sealed {
            commit: commit(6),
            through: 26,
            restarts: 27,
        },
    ] {
        agrees::<_, wit::Resume>(resume, false);
    }
}
