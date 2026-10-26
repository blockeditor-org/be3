use std::collections::{BTreeMap, BTreeSet};

pub use be_block::ConflictKind;

use be_block::{BlockMetadata, NAME};
use be_commit::{ChangeId, Commit, CommitId, CommitKind, CommitStore};
use be_store::{Hash, Manifest, ObjectStore, StoreError, Vault};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TREE_CONTENT_TYPE: Uuid = Uuid::from_u128(0x6be3_7c5e_0a1d_4f6e_9c20_7472_6565_0001);

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Tree {
    pub root: Option<Uuid>,
    pub entries: BTreeMap<Uuid, Entry>,
    #[serde(default)]
    pub conflicts: BTreeMap<Uuid, Conflict>,
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
    pub fn write<S: ObjectStore>(
        &self,
        vault: &Vault<S>,
        key: Uuid,
    ) -> Result<Manifest, StoreError> {
        vault.write(key, TREE_CONTENT_TYPE, &be_store::encode_value(self)?)
    }

    pub fn read<S: ObjectStore>(vault: &Vault<S>, manifest: &Manifest) -> Result<Self, StoreError> {
        be_store::decode_value(&vault.read(manifest)?)
    }

    pub fn objects(&self) -> BTreeSet<Hash> {
        self.entries
            .values()
            .chain(self.conflicts.values().flat_map(Conflict::sides))
            .filter_map(|entry| entry.content.as_ref())
            .flat_map(Manifest::chunk_hashes)
            .collect()
    }

    pub fn resolve(&mut self, block: Uuid, take: Option<Side>) -> bool {
        let Some(conflict) = self.conflicts.remove(&block) else {
            return false;
        };
        if let Some(take) = take {
            match conflict.side(take) {
                Some(entry) => self.entries.insert(block, entry.clone()),
                None => self.entries.remove(&block),
            };
        }
        let ids: Vec<Uuid> = self.entries.keys().copied().collect();
        for id in ids {
            if Some(id) != self.root
                && !reaches_root(self, id)
                && let Some(entry) = self.entries.get_mut(&id)
            {
                entry.parent = self.root;
            }
        }
        true
    }

    pub fn children(&self, parent: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.entries
            .iter()
            .filter(move |(_, entry)| entry.parent == Some(parent))
            .map(|(id, _)| *id)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn snapshot<S: ObjectStore>(
    commits: &CommitStore<S>,
    key: Uuid,
    tree: Manifest,
    parents: Vec<CommitId>,
    change: ChangeId,
    author: Uuid,
    time: i64,
    message: impl Into<String>,
) -> Result<CommitId, StoreError> {
    let commit = Commit::new(tree, author, time)
        .with_parents(parents)
        .with_change(change)
        .with_kind(CommitKind::Snapshot(message.into()));
    commits.put(key, &commit)
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
                    let metadata = before.metadata.defined != after.metadata.defined;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Side {
    Base,
    Ours,
    Theirs,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub base: Option<Entry>,
    pub ours: Option<Entry>,
    pub theirs: Option<Entry>,
}

impl Conflict {
    pub fn between(
        kind: ConflictKind,
        block: Uuid,
        base: &Tree,
        ours: &Tree,
        theirs: &Tree,
    ) -> Self {
        Self {
            kind,
            base: base.entries.get(&block).cloned(),
            ours: ours.entries.get(&block).cloned(),
            theirs: theirs.entries.get(&block).cloned(),
        }
    }

    pub fn side(&self, side: Side) -> Option<&Entry> {
        match side {
            Side::Base => self.base.as_ref(),
            Side::Ours => self.ours.as_ref(),
            Side::Theirs => self.theirs.as_ref(),
        }
    }

    pub fn sides(&self) -> impl Iterator<Item = &Entry> {
        [&self.base, &self.ours, &self.theirs].into_iter().flatten()
    }
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
}

struct Sides<'a> {
    base: &'a Tree,
    ours: &'a Tree,
    theirs: &'a Tree,
}

impl Sides<'_> {
    fn record(&self, plan: &mut Plan, block: Uuid, kind: ConflictKind) {
        plan.tree
            .conflicts
            .entry(block)
            .or_insert_with(|| Conflict::between(kind, block, self.base, self.ours, self.theirs));
    }
}

pub fn merge(base: &Tree, ours: &Tree, theirs: &Tree) -> Plan {
    let mut plan = Plan {
        tree: Tree {
            root: pick(&base.root, &ours.root, &theirs.root).unwrap_or(ours.root),
            ..Tree::default()
        },
        ..Plan::default()
    };
    let sides = Sides { base, ours, theirs };
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
                    sides.record(&mut plan, block, ConflictKind::EditedAndRemoved);
                    Some(kept.clone())
                }
            }
            (before, Some(ours), Some(theirs)) => {
                Some(merge_entry(&mut plan, &sides, block, before, ours, theirs))
            }
        };
        if let Some(entry) = merged {
            plan.tree.entries.insert(block, entry);
        }
    }
    repair_placement(&mut plan, &sides);
    carry_conflicts(&mut plan, &sides);
    plan
}

fn carry_conflicts(plan: &mut Plan, sides: &Sides<'_>) {
    let blocks: BTreeSet<Uuid> = sides
        .ours
        .conflicts
        .keys()
        .chain(sides.theirs.conflicts.keys())
        .copied()
        .collect();
    for block in blocks {
        let [base, ours, theirs] =
            [sides.base, sides.ours, sides.theirs].map(|tree| tree.conflicts.get(&block).cloned());
        let carried = pick(&base, &ours, &theirs).unwrap_or(ours);
        if let Some(conflict) = carried
            && plan.tree.entries.contains_key(&block)
        {
            plan.tree.conflicts.entry(block).or_insert(conflict);
        }
    }
}

fn merge_entry(
    plan: &mut Plan,
    sides: &Sides<'_>,
    block: Uuid,
    before: Option<&Entry>,
    ours: &Entry,
    theirs: &Entry,
) -> Entry {
    let mut merged = ours.clone();
    let parent = before.map(|before| before.parent);
    match pick(&parent, &Some(ours.parent), &Some(theirs.parent)) {
        Some(Some(chosen)) => merged.parent = chosen,
        _ => sides.record(plan, block, ConflictKind::Placement),
    }
    let blank = BlockMetadata::default();
    let (defined, conflicted) = BlockMetadata::merge_defined(
        before.map_or(&blank, |before| &before.metadata),
        &ours.metadata,
        &theirs.metadata,
    );
    merged.metadata.defined = defined;
    for key in conflicted {
        let kind = match key == NAME.id {
            true => ConflictKind::Name,
            false => ConflictKind::Metadata,
        };
        sides.record(plan, block, kind);
    }
    let content = before.map(|before| (before.content.clone(), before.references.clone()));
    let ours_content = Some((ours.content.clone(), ours.references.clone()));
    let theirs_content = Some((theirs.content.clone(), theirs.references.clone()));
    match pick(&content, &ours_content, &theirs_content) {
        Some(Some((content, references))) => {
            if content != ours.content {
                merged.metadata.derived.clone_from(&theirs.metadata.derived);
            }
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

fn repair_placement(plan: &mut Plan, sides: &Sides<'_>) {
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
        sides.record(plan, block, ConflictKind::Placement);
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
