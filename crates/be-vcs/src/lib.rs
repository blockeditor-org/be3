use std::collections::{BTreeMap, BTreeSet};

pub use be_block::ConflictKind;

use be_block::BlockMetadata;
use be_commit::{Commit, CommitId, CommitKind, CommitStore};
use be_store::{Hash, Manifest, ObjectStore, StoreError, Vault};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TREE_CONTENT_TYPE: Uuid = Uuid::from_u128(0x6be3_7c5e_0a1d_4f6e_9c20_7472_6565_0001);

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Tree {
    pub root: Option<Uuid>,
    pub entries: BTreeMap<Uuid, Entry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub content_type: Uuid,
    pub parent: Option<Uuid>,
    pub metadata: BlockMetadata,
    pub content: Option<Manifest>,
    pub references: Vec<Uuid>,
}

impl Tree {
    pub fn write<S: ObjectStore>(&self, vault: &Vault<S>) -> Result<Manifest, StoreError> {
        let encoded = postcard::to_stdvec(self).map_err(|_| StoreError::Encoding)?;
        vault.write(TREE_CONTENT_TYPE, &encoded)
    }

    pub fn read<S: ObjectStore>(vault: &Vault<S>, manifest: &Manifest) -> Result<Self, StoreError> {
        let bytes = vault.read(manifest)?;
        postcard::from_bytes(&bytes).map_err(|_| StoreError::Encoding)
    }

    pub fn objects(&self) -> BTreeSet<Hash> {
        self.entries
            .values()
            .filter_map(|entry| entry.content.as_ref())
            .flat_map(Manifest::chunk_hashes)
            .collect()
    }

    pub fn children(&self, parent: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.entries
            .iter()
            .filter(move |(_, entry)| entry.parent == Some(parent))
            .map(|(id, _)| *id)
    }
}

pub fn snapshot<S: ObjectStore>(
    commits: &CommitStore<S>,
    tree: Manifest,
    parents: Vec<CommitId>,
    author: Uuid,
    time: i64,
    message: impl Into<String>,
) -> Result<CommitId, StoreError> {
    let commit = Commit::new(tree, author, time)
        .with_parents(parents)
        .with_kind(CommitKind::Snapshot(message.into()));
    commits.put(&commit)
}

pub fn message_of(commit: &Commit) -> Option<&str> {
    match &commit.kind {
        CommitKind::Snapshot(message) => Some(message),
        _ => None,
    }
}

pub fn held_objects(commit: CommitId, tree: &Manifest, current: &Tree, base: &Tree) -> Vec<Hash> {
    let known = base.objects();
    let mut held = vec![commit.hash()];
    held.extend(tree.chunk_hashes());
    held.extend(current.objects().difference(&known));
    held
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Change {
    Added,
    Removed,
    Changed {
        content: bool,
        placement: bool,
        metadata: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Difference {
    pub block: Uuid,
    pub change: Change,
}

pub fn diff(base: &Tree, current: &Tree) -> Vec<Difference> {
    let ids: BTreeSet<Uuid> = base
        .entries
        .keys()
        .chain(current.entries.keys())
        .copied()
        .collect();
    ids.into_iter()
        .filter_map(|block| {
            let change = match (base.entries.get(&block), current.entries.get(&block)) {
                (None, Some(_)) => Change::Added,
                (Some(_), None) => Change::Removed,
                (Some(before), Some(after)) => {
                    let content = before.content != after.content;
                    let placement = before.parent != after.parent;
                    let metadata = named(&before.metadata) != named(&after.metadata);
                    if !(content || placement || metadata) {
                        return None;
                    }
                    Change::Changed {
                        content,
                        placement,
                        metadata,
                    }
                }
                (None, None) => return None,
            };
            Some(Difference { block, change })
        })
        .collect()
}

fn named(metadata: &BlockMetadata) -> Option<&BlockMetadata> {
    metadata.named_by_hand.then_some(metadata)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Conflict {
    pub block: Uuid,
    pub kind: ConflictKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentMerge {
    pub block: Uuid,
    pub content_type: Uuid,
    pub base: Option<Manifest>,
    pub ours: Option<Manifest>,
    pub theirs: Option<Manifest>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Plan {
    pub tree: Tree,
    pub contents: Vec<ContentMerge>,
    pub conflicts: Vec<Conflict>,
}

pub fn merge(base: &Tree, ours: &Tree, theirs: &Tree) -> Plan {
    let mut plan = Plan {
        tree: Tree {
            root: pick(&base.root, &ours.root, &theirs.root).unwrap_or(ours.root),
            entries: BTreeMap::new(),
        },
        ..Plan::default()
    };
    let ids: BTreeSet<Uuid> = base
        .entries
        .keys()
        .chain(ours.entries.keys())
        .chain(theirs.entries.keys())
        .copied()
        .collect();
    for block in ids {
        let merged = match (
            base.entries.get(&block),
            ours.entries.get(&block),
            theirs.entries.get(&block),
        ) {
            (None, Some(side), None) | (None, None, Some(side)) => Some(side.clone()),
            (_, None, None) => None,
            (Some(before), Some(kept), None) | (Some(before), None, Some(kept)) => {
                if kept == before {
                    None
                } else {
                    plan.conflicts.push(Conflict {
                        block,
                        kind: ConflictKind::EditedAndRemoved,
                    });
                    Some(kept.clone())
                }
            }
            (before, Some(ours), Some(theirs)) => {
                Some(merge_entry(&mut plan, block, before, ours, theirs))
            }
        };
        if let Some(entry) = merged {
            plan.tree.entries.insert(block, entry);
        }
    }
    repair_placement(&mut plan);
    plan
}

fn merge_entry(
    plan: &mut Plan,
    block: Uuid,
    before: Option<&Entry>,
    ours: &Entry,
    theirs: &Entry,
) -> Entry {
    let mut merged = ours.clone();
    let parent = before.map(|before| before.parent);
    match pick(&parent, &Some(ours.parent), &Some(theirs.parent)) {
        Some(Some(chosen)) => merged.parent = chosen,
        _ => plan.conflicts.push(Conflict {
            block,
            kind: ConflictKind::Placement,
        }),
    }
    let metadata = before.map(|before| before.metadata.clone());
    match pick(
        &metadata,
        &Some(ours.metadata.clone()),
        &Some(theirs.metadata.clone()),
    ) {
        Some(Some(chosen)) => merged.metadata = chosen,
        _ if !ours.metadata.named_by_hand && !theirs.metadata.named_by_hand => {}
        _ => plan.conflicts.push(Conflict {
            block,
            kind: ConflictKind::Name,
        }),
    }
    let content = before.map(|before| (before.content.clone(), before.references.clone()));
    let ours_content = Some((ours.content.clone(), ours.references.clone()));
    let theirs_content = Some((theirs.content.clone(), theirs.references.clone()));
    match pick(&content, &ours_content, &theirs_content) {
        Some(Some((content, references))) => {
            merged.content = content;
            merged.references = references;
        }
        _ => plan.contents.push(ContentMerge {
            block,
            content_type: ours.content_type,
            base: before.and_then(|before| before.content.clone()),
            ours: ours.content.clone(),
            theirs: theirs.content.clone(),
        }),
    }
    merged
}

fn pick<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> Option<T> {
    if ours == theirs || theirs == base {
        Some(ours.clone())
    } else if ours == base {
        Some(theirs.clone())
    } else {
        None
    }
}

fn repair_placement(plan: &mut Plan) {
    let root = plan.tree.root;
    let ids: Vec<Uuid> = plan.tree.entries.keys().copied().collect();
    for block in ids {
        if Some(block) == root {
            continue;
        }
        if reaches_root(&plan.tree, block) {
            continue;
        }
        if let Some(entry) = plan.tree.entries.get_mut(&block) {
            entry.parent = root;
        }
        if !plan
            .conflicts
            .iter()
            .any(|conflict| conflict.block == block && conflict.kind == ConflictKind::Placement)
        {
            plan.conflicts.push(Conflict {
                block,
                kind: ConflictKind::Placement,
            });
        }
    }
}

fn reaches_root(tree: &Tree, block: Uuid) -> bool {
    let mut seen = BTreeSet::new();
    let mut current = block;
    loop {
        if Some(current) == tree.root {
            return true;
        }
        if !seen.insert(current) {
            return false;
        }
        match tree.entries.get(&current).and_then(|entry| entry.parent) {
            Some(parent) if tree.entries.contains_key(&parent) => current = parent,
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests;
