use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use be_block::{BlockMetadata, EXECUTABLE, Entries, NAME};
use be_client::transfer::Saved;
use be_client::{Peer, Saved as Published};
use be_commit::CommitId;
use be_graph::BlockParent;
use be_store::{ChunkRef, Hash, Manifest, ObjectStore};
use be_types_host::{Modules, Plain, plain_of};
use be_vcs::{ConflictKind, Entry, Tree};
use block_type_api::{Change as Edit, ChildChange, FileChange};
use futures_util::StreamExt;
use uuid::Uuid;

use crate::disk::{self, Disk, Kind, join, name_of, parent_of};
use crate::index::{Index, Placed, Record, Skip, Stat};
use crate::remote::{self, Block};
use crate::{CLIENT, ERRORS_FILE, Report, STATE_DIRECTORY, UNEXPORTED_FILE, explain};

const IN_FLIGHT: usize = 16;
const ATTEMPTS: usize = 8;

pub(crate) async fn run<S: ObjectStore>(
    peer: &Peer<S>,
    types: &mut Modules,
    directory: &Path,
    index: &mut Index,
) -> Result<Report, String> {
    let remote = remote::list(peer, index.root).await?;
    let disk = disk::walk(directory)?;
    let folder = types
        .claims("", &[], true)
        .await
        .ok_or("no type module makes a directory into a block")?;
    let mut pass = Pass {
        peer,
        types,
        folder,
        directory,
        index,
        remote,
        disk,
        files: Files::default(),
        assigned: BTreeMap::new(),
        located: HashMap::new(),
        contents: HashMap::new(),
        merged: HashMap::new(),
        failed: HashSet::new(),
        heads: HashMap::new(),
        report: Report::default(),
    };
    pass.report.problems.extend(pass.disk.problems.clone());
    pass.assign()?;
    let base = pass.base_tree();
    let ours = pass.disk_tree()?;
    let theirs = pass.server_tree().await?;
    let mut plan = be_vcs::merge(&base, &ours, &theirs);
    pass.conflicts(&plan, &theirs);
    pass.resolve(&mut plan).await?;
    let layout = pass.layout(&plan.tree);
    pass.upload(&plan.tree, &theirs).await?;
    let stats = pass.download(&plan.tree, &ours, &layout)?;
    pass.record(&plan.tree, &layout, &stats);
    pass.index.save(directory)?;
    pass.explain_skipped(&plan.tree, &layout)?;
    Ok(pass.report)
}

#[derive(Default)]
struct Files {
    hashes: HashMap<String, (Hash, Stat)>,
}

struct Pass<'a, S: ObjectStore> {
    peer: &'a Peer<S>,
    types: &'a mut Modules,
    folder: Uuid,
    directory: &'a Path,
    index: &'a mut Index,
    remote: BTreeMap<Uuid, Block>,
    disk: Disk,
    files: Files,
    assigned: BTreeMap<String, Uuid>,
    located: HashMap<Uuid, String>,
    contents: HashMap<Uuid, (Option<Vec<u8>>, Vec<u8>)>,
    merged: HashMap<Uuid, Vec<u8>>,
    failed: HashSet<Uuid>,
    heads: HashMap<Uuid, Option<CommitId>>,
    report: Report,
}

struct Layout {
    placed: HashMap<Uuid, (String, Kind)>,
    skipped: HashMap<Uuid, (Skip, String)>,
}

fn identity(hash: Hash) -> Manifest {
    Manifest {
        content_type: Uuid::nil(),
        length: 0,
        chunks: vec![ChunkRef { hash, length: 0 }],
    }
}

fn identity_hash(manifest: &Option<Manifest>) -> Option<Hash> {
    manifest
        .as_ref()
        .and_then(|manifest| manifest.chunks.first())
        .map(|chunk| chunk.hash)
}

fn metadata(name: &str, executable: bool) -> BlockMetadata {
    let mut metadata = BlockMetadata::named(name);
    if executable {
        metadata.insert(&EXECUTABLE, &true);
    }
    metadata
}

fn name_in(entry: &Entry) -> String {
    entry.metadata.get(&NAME).unwrap_or_default()
}

fn executable_in(entry: &Entry) -> bool {
    entry.metadata.get(&EXECUTABLE).unwrap_or(false)
}

fn depth(path: &str) -> usize {
    match path.is_empty() {
        true => 0,
        false => path.matches('/').count() + 1,
    }
}

fn tree_depth(tree: &Tree, mut id: Uuid) -> usize {
    let mut depth = 0;
    while let Some(parent) = tree.entries.get(&id).and_then(|entry| entry.parent) {
        depth += 1;
        id = parent;
        if depth > tree.entries.len() {
            break;
        }
    }
    depth
}

fn children_of(tree: &Tree) -> HashMap<Uuid, Vec<Uuid>> {
    let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (id, entry) in &tree.entries {
        if let Some(parent) = entry.parent {
            children.entry(parent).or_default().push(*id);
        }
    }
    children
}

impl<S: ObjectStore> Pass<'_, S> {
    fn root(&self) -> Uuid {
        self.index.root
    }

    fn absolute(&self, path: &str) -> PathBuf {
        self.directory.join(path)
    }

    fn hash_file(&mut self, path: &str) -> Result<(Hash, Stat), String> {
        if let Some(known) = self.files.hashes.get(path) {
            return Ok(*known);
        }
        let absolute = self.directory.join(path);
        let stat = disk::stat(&absolute).ok_or_else(|| format!("{path} went away"))?;
        let bytes = std::fs::read(&absolute)
            .map_err(|error| format!("could not read {}: {error}", absolute.display()))?;
        let known = (Hash::of(&bytes), stat);
        self.files.hashes.insert(path.to_owned(), known);
        Ok(known)
    }

    fn read_file(&self, id: Uuid, expected: Option<Hash>) -> Option<Vec<u8>> {
        let path = self.located.get(&id)?;
        let bytes = std::fs::read(self.absolute(path)).ok()?;
        match expected {
            Some(expected) if Hash::of(&bytes) != expected => None,
            _ => Some(bytes),
        }
    }

    fn assign(&mut self) -> Result<(), String> {
        let root = self.root();
        self.assigned.insert(String::new(), root);
        let mut claimed: HashSet<Uuid> = HashSet::from([root]);
        let by_path: HashMap<String, (Uuid, bool)> = self
            .index
            .records
            .iter()
            .filter(|(id, _)| **id != root)
            .filter_map(|(id, record)| {
                record
                    .path()
                    .map(|path| (path.to_owned(), (*id, record.is_directory())))
            })
            .collect();
        for (path, found) in &self.disk.found {
            if let Some((id, directory)) = by_path.get(path)
                && *directory == (found.kind == Kind::Directory)
            {
                self.assigned.insert(path.clone(), *id);
                claimed.insert(*id);
            }
        }
        let vanished: Vec<Uuid> = self
            .index
            .records
            .iter()
            .filter(|(id, record)| {
                !claimed.contains(id)
                    && record
                        .path()
                        .is_some_and(|path| !self.now_ignored(path, record.is_directory()))
            })
            .map(|(id, _)| *id)
            .collect();
        let mut by_inode: HashMap<(u64, bool), Uuid> = HashMap::new();
        let mut by_content: HashMap<Hash, Vec<Uuid>> = HashMap::new();
        let mut vanished_directories = HashSet::new();
        for id in &vanished {
            let record = &self.index.records[id];
            match &record.placed {
                Placed::Directory { inode, .. } => {
                    vanished_directories.insert(*id);
                    if *inode != 0 {
                        by_inode.insert((*inode, true), *id);
                    }
                }
                Placed::File { stat, .. } => {
                    if stat.inode != 0 {
                        by_inode.insert((stat.inode, false), *id);
                    }
                    if let Some(content) = record.content {
                        by_content.entry(content).or_default().push(*id);
                    }
                }
                Placed::Skipped { .. } => {}
            }
        }
        let natural = self.natural_paths();
        let mut rest: Vec<String> = self
            .disk
            .found
            .keys()
            .filter(|path| !self.assigned.contains_key(*path))
            .cloned()
            .collect();
        rest.sort_by_key(|path| Reverse(depth(path)));
        for path in rest {
            let found = self.disk.found[&path].clone();
            let directory = found.kind == Kind::Directory;
            let unclaimed = |id: &Uuid| !claimed.contains(id);
            let mut pick = by_inode
                .get(&(found.stat.inode, directory))
                .copied()
                .filter(unclaimed);
            if pick.is_none() {
                pick = natural
                    .get(&path)
                    .filter(|(_, is_directory)| *is_directory == directory)
                    .map(|(id, _)| *id)
                    .filter(unclaimed);
            }
            if pick.is_none() && !directory && !by_content.is_empty() {
                let (hash, _) = self.hash_file(&path)?;
                let name = name_of(&path);
                let candidates: Vec<Uuid> = by_content
                    .get(&hash)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(unclaimed)
                    .collect();
                pick = candidates
                    .iter()
                    .copied()
                    .find(|id| self.index.records[id].name == name)
                    .or_else(|| candidates.first().copied());
            }
            if pick.is_none() && directory && !vanished_directories.is_empty() {
                let mut votes: HashMap<Uuid, usize> = HashMap::new();
                let prefix = format!("{path}/");
                for (child, id) in self.assigned.range(prefix.clone()..) {
                    if !child.starts_with(&prefix) {
                        break;
                    }
                    if parent_of(child) != path {
                        continue;
                    }
                    if let Some(parent) =
                        self.index.records.get(id).and_then(|record| record.parent)
                        && vanished_directories.contains(&parent)
                        && unclaimed(&parent)
                    {
                        *votes.entry(parent).or_default() += 1;
                    }
                }
                pick = votes
                    .into_iter()
                    .max_by_key(|(id, count)| (*count, Reverse(*id)))
                    .map(|(id, _)| id);
            }
            let id = pick.unwrap_or_else(Uuid::now_v7);
            claimed.insert(id);
            self.assigned.insert(path, id);
        }
        self.located = self
            .assigned
            .iter()
            .map(|(path, id)| (*id, path.clone()))
            .collect();
        Ok(())
    }

    fn now_ignored(&self, path: &str, directory: bool) -> bool {
        self.disk.ignored(path, directory) && std::fs::symlink_metadata(self.absolute(path)).is_ok()
    }

    fn natural_paths(&self) -> HashMap<String, (Uuid, bool)> {
        let mut paths: HashMap<Uuid, Option<String>> = HashMap::new();
        let mut natural = HashMap::new();
        for (id, block) in &self.remote {
            let adoptable = match self.index.records.get(id) {
                None => true,
                Some(record) => matches!(
                    record.placed,
                    Placed::Skipped {
                        reason: Skip::Ignored,
                        ..
                    }
                ),
            };
            if !adoptable || plain_of(block.block_type) == Plain::None {
                continue;
            }
            if let Some(path) = self.path_of(*id, &mut paths) {
                natural.insert(path, (*id, plain_of(block.block_type) == Plain::Directory));
            }
        }
        let mut seen: HashMap<String, usize> = HashMap::new();
        for path in natural.keys() {
            *seen.entry(path.to_lowercase()).or_default() += 1;
        }
        natural.retain(|path, _| seen[&path.to_lowercase()] == 1);
        natural
    }

    fn path_of(&self, id: Uuid, memo: &mut HashMap<Uuid, Option<String>>) -> Option<String> {
        if id == self.root() {
            return Some(String::new());
        }
        if let Some(known) = memo.get(&id) {
            return known.clone();
        }
        let path = match self.index.records.get(&id).and_then(Record::path) {
            Some(path) => Some(path.to_owned()),
            None => {
                let block = self.remote.get(&id);
                let parent = block.and_then(|block| block.parent);
                match (block, parent) {
                    (Some(block), Some(parent))
                        if self.remote.get(&parent).is_some_and(|above| {
                            plain_of(above.block_type) == Plain::Directory
                        }) =>
                    {
                        self.path_of(parent, memo)
                            .map(|above| join(&above, &block.name))
                    }
                    _ => None,
                }
            }
        };
        memo.insert(id, path.clone());
        path
    }

    fn root_entry(&self) -> Entry {
        let block_type = self
            .remote
            .get(&self.root())
            .map_or(self.folder, |block| block.block_type);
        Entry {
            content_type: block_type,
            parent: None,
            metadata: BlockMetadata::default(),
            content: None,
            references: Vec::new(),
        }
    }

    fn record_entry(&self, record: &Record) -> Entry {
        Entry {
            content_type: record.block_type,
            parent: record.parent,
            metadata: metadata(&record.name, record.executable),
            content: record.content.map(identity),
            references: Vec::new(),
        }
    }

    fn base_tree(&self) -> Tree {
        let root = self.root();
        let mut tree = Tree {
            root: Some(root),
            ..Tree::default()
        };
        for (id, record) in &self.index.records {
            if *id != root {
                tree.entries.insert(*id, self.record_entry(record));
            }
        }
        tree.entries.insert(root, self.root_entry());
        tree
    }

    fn disk_tree(&mut self) -> Result<Tree, String> {
        let root = self.root();
        let mut tree = Tree {
            root: Some(root),
            ..Tree::default()
        };
        tree.entries.insert(root, self.root_entry());
        let assigned: Vec<(String, Uuid)> = self
            .assigned
            .iter()
            .filter(|(path, _)| !path.is_empty())
            .map(|(path, id)| (path.clone(), *id))
            .collect();
        for (path, id) in assigned {
            let found = self.disk.found[&path].clone();
            let parent = self.assigned[parent_of(&path)];
            let known_type = self
                .remote
                .get(&id)
                .map(|block| block.block_type)
                .or_else(|| self.index.records.get(&id).map(|record| record.block_type));
            let (content_type, content) =
                match found.kind {
                    Kind::Directory => (known_type.unwrap_or(self.folder), None),
                    Kind::File => {
                        let recorded = self.index.records.get(&id).and_then(|record| match &record
                            .placed
                        {
                            Placed::File { stat, .. } if *stat == found.stat => record.content,
                            _ => None,
                        });
                        let hash = match recorded {
                            Some(hash) => hash,
                            None => self.hash_file(&path)?.0,
                        };
                        (known_type.unwrap_or_else(Uuid::nil), Some(identity(hash)))
                    }
                };
            tree.entries.insert(
                id,
                Entry {
                    content_type,
                    parent: Some(parent),
                    metadata: metadata(name_of(&path), found.executable),
                    content,
                    references: Vec::new(),
                },
            );
        }
        let base = self.base_tree();
        let mut mirrored: Vec<Uuid> = self
            .index
            .records
            .iter()
            .filter(|(id, record)| {
                **id != root
                    && !self.located.contains_key(id)
                    && match record.path() {
                        None => true,
                        Some(path) => self.now_ignored(path, record.is_directory()),
                    }
            })
            .map(|(id, _)| *id)
            .collect();
        mirrored.sort_by_key(|id| tree_depth(&base, *id));
        for id in mirrored {
            let record = &self.index.records[&id];
            if record
                .parent
                .is_some_and(|parent| tree.entries.contains_key(&parent))
            {
                tree.entries.insert(id, self.record_entry(record));
            }
        }
        Ok(tree)
    }

    async fn server_tree(&mut self) -> Result<Tree, String> {
        let root = self.root();
        let mut unread = Vec::new();
        for (id, block) in &self.remote {
            if *id == root || plain_of(block.block_type) != Plain::File {
                continue;
            }
            let known = self.index.records.get(id).is_some_and(|record| {
                record.head == block.head
                    && record.content.is_some()
                    && record.block_type == block.block_type
            });
            if !known {
                unread.push((*id, block.head));
            }
        }
        let read = remote::read(self.peer, &unread).await?;
        let files = self
            .types
            .to_files(
                read.iter()
                    .filter_map(|(id, _, content)| {
                        Some((self.remote[id].block_type, content.clone()?))
                    })
                    .collect(),
            )
            .await;
        let mut files = files.into_iter();
        for (id, head, content) in read {
            let block = self.remote.get_mut(&id).expect("a listed block");
            block.head = head;
            let plain = match &content {
                Some(_) => match files
                    .next()
                    .unwrap_or_else(|| Err("the type module did not answer".to_owned()))
                {
                    Ok(Some(plain)) => plain.bytes,
                    Ok(None) => {
                        self.report
                            .problems
                            .push(format!("block {id} has no plain-file form"));
                        self.failed.insert(id);
                        continue;
                    }
                    Err(error) => {
                        self.report
                            .problems
                            .push(format!("block {id} could not be read: {error}"));
                        self.failed.insert(id);
                        continue;
                    }
                },
                None => Vec::new(),
            };
            self.contents.insert(id, (content, plain));
        }
        let mut tree = Tree {
            root: Some(root),
            ..Tree::default()
        };
        tree.entries.insert(root, self.root_entry());
        for (id, block) in &self.remote {
            if *id == root {
                continue;
            }
            let content = match plain_of(block.block_type) {
                Plain::Directory => None,
                Plain::None => block.head.map(|head| identity(head.hash())),
                Plain::File => match self.contents.get(id) {
                    Some((_, plain)) => Some(identity(Hash::of(plain))),
                    None => self
                        .index
                        .records
                        .get(id)
                        .and_then(|record| record.content)
                        .map(identity),
                },
            };
            tree.entries.insert(
                *id,
                Entry {
                    content_type: block.block_type,
                    parent: block.parent,
                    metadata: metadata(&block.name, block.executable),
                    content,
                    references: Vec::new(),
                },
            );
        }
        Ok(tree)
    }

    fn label(&self, tree: &Tree, id: Uuid) -> String {
        if let Some(path) = self.located.get(&id) {
            return path.clone();
        }
        let mut names = Vec::new();
        let mut current = id;
        while current != self.root() {
            let Some(entry) = tree.entries.get(&current) else {
                break;
            };
            names.push(name_in(entry));
            match entry.parent {
                Some(parent) => current = parent,
                None => break,
            }
            if names.len() > tree.entries.len() {
                break;
            }
        }
        names.reverse();
        names.join("/")
    }

    fn conflicts(&mut self, plan: &be_vcs::Plan, theirs: &Tree) {
        for (block, conflict) in &plan.tree.conflicts {
            let label = self.label(&plan.tree, *block);
            let text = match conflict.kind {
                ConflictKind::Name => {
                    format!("{label} was renamed here and in block-app; the name here was kept")
                }
                ConflictKind::Placement => format!(
                    "{label} was moved here and in block-app, or its folder went away; it was put where it is now"
                ),
                ConflictKind::EditedAndRemoved => match theirs.entries.contains_key(block) {
                    true => {
                        format!("{label} was deleted here and changed in block-app; it was kept")
                    }
                    false => {
                        format!("{label} was deleted in block-app and changed here; it was kept")
                    }
                },
                ConflictKind::Metadata => format!(
                    "{label} was made executable or not, here and in block-app; this side was kept"
                ),
                ConflictKind::Content => format!("{label} changed on both sides"),
            };
            self.report.conflicts.push(text);
        }
    }

    async fn resolve(&mut self, plan: &mut be_vcs::Plan) -> Result<(), String> {
        for merge in std::mem::take(&mut plan.contents) {
            let id = merge.block;
            let label = self.label(&plan.tree, id);
            let Some(local) = self.read_file(id, identity_hash(&merge.ours)) else {
                self.failed.insert(id);
                self.report.retry = true;
                continue;
            };
            let theirs = self
                .contents
                .get(&id)
                .and_then(|(content, _)| content.clone());
            let block_type = self
                .remote
                .get(&id)
                .map_or(merge.content_type, |block| block.block_type);
            let name = plan.tree.entries.get(&id).map(name_in).unwrap_or_default();
            let base = match self.index.records.get(&id).and_then(|record| record.head) {
                Some(head) => remote::at_commit(self.peer, head).await.ok(),
                None => None,
            };
            let bytes = match self.merge(block_type, base, theirs, &name, local).await {
                Ok((bytes, false)) => bytes,
                Ok((bytes, true)) => {
                    self.report.conflicts.push(format!(
                        "{label} changed here and in block-app; what both changed is kept as the block's type merges it (text between conflict markers, a file as it is here), and the other side is in the block's history"
                    ));
                    bytes
                }
                Err(error) => {
                    self.report
                        .problems
                        .push(format!("{label} could not be merged: {error}"));
                    self.failed.insert(id);
                    continue;
                }
            };
            if let Some(entry) = plan.tree.entries.get_mut(&id) {
                entry.content = Some(identity(Hash::of(&bytes)));
            }
            self.merged.insert(id, bytes);
        }
        Ok(())
    }

    async fn file_block(
        &mut self,
        current: Option<(Uuid, Option<Vec<u8>>)>,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<FileBlock, String> {
        let (block_type, current) = match current {
            Some(current) => current,
            None => {
                let claimed = self
                    .types
                    .claims(name, &bytes, false)
                    .await
                    .ok_or("no block type takes it")?;
                (claimed, None)
            }
        };
        let edit = Edit::FromFile(FileChange {
            name: name.to_owned(),
            bytes,
        });
        let content = self
            .types
            .apply(block_type, current, CLIENT, edit)
            .await?
            .ok_or("its type has no plain-file form")?;
        self.described(block_type, content).await
    }

    async fn directory(&mut self, children: &[Uuid]) -> Result<FileBlock, String> {
        let block_type = self.folder;
        let mut content = None;
        for child in children {
            let change = Edit::Child(ChildChange::Add(*child));
            if let Some(changed) = self
                .types
                .apply(block_type, content.clone(), CLIENT, change)
                .await?
            {
                content = Some(changed);
            }
        }
        let content = match content {
            Some(content) => content,
            None => {
                be_types_host::kind_of(block_type)
                    .ok_or("the directory type has no module")?
                    .blank
            }
        };
        self.described(block_type, content).await
    }

    async fn described(&mut self, block_type: Uuid, content: Vec<u8>) -> Result<FileBlock, String> {
        let description = self
            .types
            .describe(block_type, &content, self.peer.organization())
            .await?;
        Ok(FileBlock {
            block_type,
            content,
            references: description.references,
            derived: description.derived,
        })
    }

    async fn merge(
        &mut self,
        block_type: Uuid,
        base: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
        name: &str,
        local: Vec<u8>,
    ) -> Result<(Vec<u8>, bool), String> {
        let edit = Edit::FromFile(FileChange {
            name: name.to_owned(),
            bytes: local,
        });
        let ours = self
            .types
            .apply(block_type, base.clone(), CLIENT, edit)
            .await?
            .ok_or("its type has no plain-file form")?;
        let (merged, conflicts) = self
            .types
            .merge3(block_type, base, Some(ours), theirs)
            .await?;
        let file = self
            .types
            .to_files(vec![(block_type, merged)])
            .await
            .pop()
            .unwrap_or_else(|| Err("the type module did not answer".to_owned()))?
            .ok_or("its type has no plain-file form")?;
        Ok((file.bytes, conflicts))
    }

    fn kind_in(&self, tree: &Tree, id: Uuid) -> Option<Kind> {
        if let Some(path) = self.located.get(&id) {
            return Some(
                self.disk
                    .found
                    .get(path)
                    .map_or(Kind::Directory, |found| found.kind),
            );
        }
        match plain_of(tree.entries.get(&id)?.content_type) {
            Plain::Directory => Some(Kind::Directory),
            Plain::File => Some(Kind::File),
            Plain::None => None,
        }
    }

    fn layout(&self, tree: &Tree) -> Layout {
        let root = self.root();
        let children = children_of(tree);
        let mut layout = Layout {
            placed: HashMap::from([(root, (String::new(), Kind::Directory))]),
            skipped: HashMap::new(),
        };
        let mut queue = vec![(root, String::new())];
        while let Some((directory, at)) = queue.pop() {
            let mut groups: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
            for child in children.get(&directory).into_iter().flatten() {
                let entry = &tree.entries[child];
                match self.kind_in(tree, *child) {
                    None => {
                        layout.skipped.insert(
                            *child,
                            (
                                Skip::NoFileForm,
                                format!(
                                    "a block of type {} has no plain-file form",
                                    entry.content_type
                                ),
                            ),
                        );
                        self.skip_inside(tree, &children, *child, &mut layout);
                    }
                    Some(_) => groups
                        .entry(name_in(entry).to_lowercase())
                        .or_default()
                        .push(*child),
                }
            }
            for members in groups.into_values() {
                let wanted = |id: &Uuid| join(&at, &name_in(&tree.entries[id]));
                let exact: Vec<Uuid> = members
                    .iter()
                    .copied()
                    .filter(|id| self.located.get(id) == Some(&wanted(id)))
                    .collect();
                let mut placed = Vec::new();
                if !exact.is_empty() {
                    placed = exact.clone();
                    for id in members.iter().filter(|id| !exact.contains(id)) {
                        let other = &exact[0];
                        layout.skipped.insert(
                            *id,
                            (
                                Skip::Collision,
                                format!(
                                    "{} is already the name of another file here{}",
                                    name_in(&tree.entries[other]),
                                    match name_in(&tree.entries[other])
                                        == name_in(&tree.entries[id])
                                    {
                                        true => "",
                                        false => ", apart from letter case",
                                    }
                                ),
                            ),
                        );
                    }
                } else if let [only] = members.as_slice() {
                    let name = name_in(&tree.entries[only]);
                    let path = wanted(only);
                    let directory = self.kind_in(tree, *only) == Some(Kind::Directory);
                    let problem = match disk::name_problem(&name) {
                        Some(problem) => Some((Skip::Name, problem)),
                        None if self.disk.ignored(&path, directory) => Some((
                            Skip::Ignored,
                            format!("{path} is ignored by {} or .gitignore", crate::IGNORE_FILE),
                        )),
                        None if !self.assigned.contains_key(&path)
                            && std::fs::symlink_metadata(self.absolute(&path)).is_ok() =>
                        {
                            Some((
                                Skip::Occupied,
                                format!("something be sync does not keep is already at {path}"),
                            ))
                        }
                        None => None,
                    };
                    match problem {
                        Some(problem) => {
                            layout.skipped.insert(*only, problem);
                        }
                        None => placed.push(*only),
                    }
                } else {
                    for id in &members {
                        layout.skipped.insert(
                            *id,
                            (
                                Skip::Collision,
                                format!(
                                    "{} other blocks here have the same name, or one differing only in letter case",
                                    members.len() - 1
                                ),
                            ),
                        );
                    }
                }
                for id in &members {
                    if layout.skipped.contains_key(id) {
                        self.skip_inside(tree, &children, *id, &mut layout);
                    }
                }
                for id in placed {
                    let path = wanted(&id);
                    let kind = self.kind_in(tree, id).unwrap_or(Kind::File);
                    match kind {
                        Kind::Directory => queue.push((id, path.clone())),
                        Kind::File => self.skip_inside(tree, &children, id, &mut layout),
                    }
                    layout.placed.insert(id, (path, kind));
                }
            }
        }
        layout
    }

    fn skip_inside(
        &self,
        tree: &Tree,
        children: &HashMap<Uuid, Vec<Uuid>>,
        outer: Uuid,
        layout: &mut Layout,
    ) {
        let name = name_in(&tree.entries[&outer]);
        let mut pending: Vec<Uuid> = children.get(&outer).cloned().unwrap_or_default();
        while let Some(id) = pending.pop() {
            layout.skipped.insert(
                id,
                (
                    Skip::Inside,
                    format!("it is inside {name}, which is not a directory here"),
                ),
            );
            pending.extend(children.get(&id).into_iter().flatten());
        }
    }

    async fn upload(&mut self, target: &Tree, theirs: &Tree) -> Result<(), String> {
        let root = self.root();
        let target_children = children_of(target);
        let server_children = children_of(theirs);
        let mut created: Vec<(Uuid, usize)> = target
            .entries
            .keys()
            .filter(|id| **id != root && !theirs.entries.contains_key(id))
            .map(|id| (*id, tree_depth(target, *id)))
            .collect();
        created.sort_by_key(|(_, depth)| *depth);
        let mut placed = Vec::new();
        for (id, _) in created {
            let entry = &target.entries[&id];
            let parent = entry.parent.unwrap_or(root);
            if self.failed.contains(&parent) {
                self.failed.insert(id);
                continue;
            }
            if self.index.records.contains_key(&id) {
                match self.peer.set_parent(id, BlockParent::Block(parent)).await {
                    Ok(()) => {
                        self.report.renamed += 1;
                        self.failed.insert(id);
                        self.report.retry = true;
                        continue;
                    }
                    Err(error) => {
                        self.report.problems.push(format!(
                            "{} was deleted in block-app and could not be brought back ({}); it will be uploaded as a new block",
                            self.label(target, id),
                            explain(error)
                        ));
                        self.index.records.remove(&id);
                        self.failed.insert(id);
                        self.report.retry = true;
                        continue;
                    }
                }
            }
            let kind = self.kind_in(target, id);
            if kind == Some(Kind::Directory) && !self.holds_files(target, &target_children, id) {
                continue;
            }
            let name = name_in(entry);
            let mut defined = metadata(&name, executable_in(entry));
            let saved = match kind {
                Some(Kind::Directory) => {
                    let children: Vec<Uuid> = target_children
                        .get(&id)
                        .into_iter()
                        .flatten()
                        .copied()
                        .collect();
                    let folder = match self.directory(&children).await {
                        Ok(folder) => folder,
                        Err(error) => {
                            self.report.problems.push(format!(
                                "{} could not be made: {error}",
                                self.label(target, id)
                            ));
                            self.failed.insert(id);
                            continue;
                        }
                    };
                    defined.derived = folder.derived;
                    Saved {
                        id,
                        block_type: folder.block_type,
                        parent: None,
                        metadata: defined,
                        references: folder.references,
                        content: Some(folder.content),
                    }
                }
                _ => {
                    let bytes = match self.merged.get(&id) {
                        Some(bytes) => Some(bytes.clone()),
                        None => self.read_file(id, identity_hash(&entry.content)),
                    };
                    let Some(bytes) = bytes else {
                        self.failed.insert(id);
                        self.report.retry = true;
                        continue;
                    };
                    let block = match self.file_block(None, &name, bytes).await {
                        Ok(block) => block,
                        Err(error) => {
                            self.report.problems.push(format!(
                                "{} could not be read: {error}",
                                self.label(target, id)
                            ));
                            self.failed.insert(id);
                            continue;
                        }
                    };
                    defined.derived = block.derived;
                    Saved {
                        id,
                        block_type: block.block_type,
                        parent: None,
                        metadata: defined,
                        references: block.references,
                        content: Some(block.content),
                    }
                }
            };
            placed.push((saved, BlockParent::Block(parent)));
        }
        let mut types = HashMap::new();
        for (saved, _) in &placed {
            types.insert(saved.id, saved.block_type);
        }
        let mut refused = Vec::new();
        let mut made = Vec::new();
        self.peer
            .import_blocks(&placed, |id, imported| match imported {
                Ok(summary) => made.push((id, summary.head)),
                Err(error) => refused.push((id, explain(error))),
            })
            .await;
        for (id, head) in made {
            let head = self.peer.local_head(id).or(head);
            self.heads.insert(id, head);
            self.remote.insert(
                id,
                Block {
                    block_type: types[&id],
                    parent: target.entries[&id].parent,
                    name: name_in(&target.entries[&id]),
                    executable: executable_in(&target.entries[&id]),
                    head,
                    defined: Entries::new(),
                    can_edit: true,
                },
            );
            self.report.created += 1;
        }
        for (id, error) in refused {
            self.report.problems.push(format!(
                "{} could not be uploaded: {error}",
                self.label(target, id)
            ));
            self.failed.insert(id);
        }
        self.update(target, theirs).await?;
        self.delete(target, theirs).await;
        self.tidy_folders(target, theirs, &target_children, &server_children)
            .await;
        Ok(())
    }

    fn holds_files(&self, target: &Tree, children: &HashMap<Uuid, Vec<Uuid>>, id: Uuid) -> bool {
        children.get(&id).into_iter().flatten().any(|child| {
            self.remote.contains_key(child)
                || match self.kind_in(target, *child) {
                    Some(Kind::Directory) => self.holds_files(target, children, *child),
                    _ => true,
                }
        })
    }

    async fn update(&mut self, target: &Tree, theirs: &Tree) -> Result<(), String> {
        let root = self.root();
        let mut changes = Vec::new();
        for (id, wanted) in &target.entries {
            let Some(current) = theirs.entries.get(id) else {
                continue;
            };
            if *id == root || self.failed.contains(id) {
                continue;
            }
            let content =
                wanted.content != current.content && plain_of(current.content_type) == Plain::File;
            let placement = wanted.parent != current.parent;
            let named = name_in(wanted) != name_in(current)
                || executable_in(wanted) != executable_in(current);
            if !(content || placement || named) {
                continue;
            }
            if !self.remote[id].can_edit {
                self.report.problems.push(format!(
                    "{} changed here, but this account may only read it in block-app",
                    self.label(target, *id)
                ));
                self.failed.insert(*id);
                continue;
            }
            changes.push((*id, content, placement, named));
        }
        let unread: Vec<(Uuid, Option<CommitId>)> = changes
            .iter()
            .filter(|(id, content, _, _)| *content && !self.contents.contains_key(id))
            .map(|(id, _, _, _)| (*id, self.remote[id].head))
            .collect();
        let mut current_bytes: HashMap<Uuid, Option<Vec<u8>>> = HashMap::new();
        for (id, _, content) in remote::read(self.peer, &unread).await? {
            current_bytes.insert(id, content);
        }
        let mut work = Vec::new();
        for (id, content, placement, named) in changes {
            let wanted = &target.entries[&id];
            let block = self.remote[&id].clone();
            let name = name_in(wanted);
            let executable = executable_in(wanted);
            let mut defined = block.defined.clone();
            let mut named_metadata = BlockMetadata {
                defined: defined.clone(),
                derived: Entries::new(),
            };
            named_metadata.insert(&NAME, &name);
            named_metadata.put(&EXECUTABLE, executable.then_some(&true));
            defined = named_metadata.defined;
            let file = match content {
                false => None,
                true => {
                    let bytes = match self.merged.get(&id) {
                        Some(bytes) => Some(bytes.clone()),
                        None => self.read_file(id, identity_hash(&wanted.content)),
                    };
                    let Some(bytes) = bytes else {
                        self.failed.insert(id);
                        self.report.retry = true;
                        continue;
                    };
                    let current = match self.contents.get(&id) {
                        Some((encoded, _)) => encoded.clone(),
                        None => current_bytes.get(&id).cloned().flatten(),
                    };
                    let block_type = block.block_type;
                    match self
                        .file_block(Some((block_type, current)), &name, bytes)
                        .await
                    {
                        Ok(file) => Some(file),
                        Err(error) => {
                            self.report.problems.push(format!(
                                "{} could not be read: {error}",
                                self.label(target, id)
                            ));
                            self.failed.insert(id);
                            continue;
                        }
                    }
                }
            };
            work.push(Change {
                id,
                file,
                defined,
                named,
                parent: placement.then(|| wanted.parent.unwrap_or(root)),
                head: block.head,
                name,
                executable,
                label: self.label(target, id),
            });
        }
        let peer = self.peer;
        let mut results = futures_util::stream::iter(work.into_iter().map(|change| async move {
            let outcome = change.apply(peer).await;
            (change, outcome)
        }))
        .buffer_unordered(IN_FLIGHT);
        while let Some((change, outcome)) = results.next().await {
            match outcome {
                Ok(head) => {
                    if change.file.is_some() {
                        self.report.uploaded += 1;
                    }
                    if change.named || change.parent.is_some() {
                        self.report.renamed += 1;
                    }
                    self.heads.insert(change.id, head);
                }
                Err(Retry) => {
                    self.failed.insert(change.id);
                    self.report.retry = true;
                }
                Err(Refused(error)) => {
                    self.failed.insert(change.id);
                    self.report
                        .problems
                        .push(format!("{} could not be saved: {error}", change.label));
                }
            }
        }
        Ok(())
    }

    async fn delete(&mut self, target: &Tree, theirs: &Tree) {
        let root = self.root();
        let gone: Vec<Uuid> = theirs
            .entries
            .iter()
            .filter(|(id, entry)| {
                **id != root
                    && !target.entries.contains_key(id)
                    && entry
                        .parent
                        .is_none_or(|parent| target.entries.contains_key(&parent))
            })
            .map(|(id, _)| *id)
            .collect();
        let peer = self.peer;
        let mut results = futures_util::stream::iter(
            gone.into_iter()
                .map(|id| async move { (id, peer.delete(id).await) }),
        )
        .buffer_unordered(IN_FLIGHT);
        while let Some((id, outcome)) = results.next().await {
            match outcome {
                Ok(()) => self.report.deleted += 1,
                Err(error) => {
                    self.report.problems.push(format!(
                        "{} could not be deleted in block-app: {}",
                        self.label(theirs, id),
                        explain(error)
                    ));
                    self.failed.insert(id);
                }
            }
        }
    }

    async fn tidy_folders(
        &mut self,
        target: &Tree,
        theirs: &Tree,
        target_children: &HashMap<Uuid, Vec<Uuid>>,
        server_children: &HashMap<Uuid, Vec<Uuid>>,
    ) {
        let empty = Vec::new();
        let mut folders = Vec::new();
        for (id, entry) in &target.entries {
            if plain_of(entry.content_type) != Plain::Directory || self.failed.contains(id) {
                continue;
            }
            if !theirs.entries.contains_key(id) {
                continue;
            }
            let now: BTreeSet<Uuid> = target_children
                .get(id)
                .unwrap_or(&empty)
                .iter()
                .copied()
                .filter(|child| !self.failed.contains(child))
                .collect();
            let before: BTreeSet<Uuid> = server_children
                .get(id)
                .unwrap_or(&empty)
                .iter()
                .copied()
                .collect();
            let gained: Vec<Uuid> = now.difference(&before).copied().collect();
            let lost: Vec<Uuid> = before
                .difference(&now)
                .copied()
                .filter(|child| !self.failed.contains(child))
                .collect();
            if !gained.is_empty() || !lost.is_empty() {
                folders.push((*id, entry.content_type, gained, lost));
            }
        }
        for (id, block_type, gained, lost) in folders {
            let tidied = list_children(self.peer, self.types, id, block_type, &gained, &lost).await;
            if let Err(error) = tidied {
                self.report.problems.push(format!(
                    "{} could not list its new contents: {error}",
                    self.label(target, id)
                ));
            }
        }
    }

    fn download(
        &mut self,
        target: &Tree,
        ours: &Tree,
        layout: &Layout,
    ) -> Result<HashMap<Uuid, Stat>, String> {
        let root = self.root();
        let scratch = disk::temporary(self.directory)?;
        let staging = scratch.join(format!("staging-{}", Uuid::new_v4().simple()));
        let mut stats = HashMap::new();
        let mut staged: HashMap<Uuid, PathBuf> = HashMap::new();
        let mut moving: Vec<(Uuid, String)> = Vec::new();
        let mut removing: Vec<(Uuid, String)> = Vec::new();
        for (id, current) in &self.located {
            if *id == root || self.failed.contains(id) {
                continue;
            }
            match layout.placed.get(id) {
                None => removing.push((*id, current.clone())),
                Some((wanted, _)) => {
                    let parent = self.assigned[parent_of(current)];
                    let wanted_parent = target.entries[id].parent.unwrap_or(root);
                    if parent != wanted_parent || name_of(current) != name_of(wanted) {
                        moving.push((*id, current.clone()));
                    }
                }
            }
        }
        moving.sort_by_key(|(_, path)| Reverse(depth(path)));
        for (id, current) in moving {
            if !self.unchanged(&current) {
                self.failed.insert(id);
                self.report.retry = true;
                continue;
            }
            std::fs::create_dir_all(&staging)
                .map_err(|error| format!("could not make {}: {error}", staging.display()))?;
            let to = staging.join(id.simple().to_string());
            match std::fs::rename(self.absolute(&current), &to) {
                Ok(()) => {
                    staged.insert(id, to);
                }
                Err(error) => {
                    self.report
                        .problems
                        .push(format!("could not move {current}: {error}"));
                    self.failed.insert(id);
                }
            }
        }
        removing.sort_by_key(|(_, path)| Reverse(depth(path)));
        for (id, current) in removing {
            let absolute = self.absolute(&current);
            let removed = match self.disk.found.get(&current).map(|found| found.kind) {
                Some(Kind::Directory) => match std::fs::remove_dir(&absolute) {
                    Ok(()) => true,
                    Err(_) => {
                        self.report.problems.push(format!(
                            "{current} was left behind: it still holds files be sync does not keep"
                        ));
                        false
                    }
                },
                _ => {
                    if !self.unchanged(&current) {
                        self.failed.insert(id);
                        self.report.retry = true;
                        continue;
                    }
                    std::fs::remove_file(&absolute).is_ok()
                }
            };
            if removed {
                self.report.removed += 1;
            }
        }
        let mut placing: Vec<(&Uuid, &(String, Kind))> = layout
            .placed
            .iter()
            .filter(|(id, _)| **id != root && !self.failed.contains(id))
            .collect();
        placing.sort_by_key(|(_, (path, _))| depth(path));
        for (id, (path, kind)) in placing {
            let absolute = self.absolute(path);
            let current = self.located.get(id).cloned();
            if let Some(from) = staged.get(id) {
                if let Err(error) = std::fs::rename(from, &absolute) {
                    self.report
                        .problems
                        .push(format!("could not move a file to {path}: {error}"));
                    self.failed.insert(*id);
                    continue;
                }
                self.report.moved += 1;
            }
            let wanted = &target.entries[id];
            match kind {
                Kind::Directory => {
                    if current.is_none() {
                        if let Err(error) = std::fs::create_dir(&absolute)
                            && error.kind() != std::io::ErrorKind::AlreadyExists
                        {
                            self.report
                                .problems
                                .push(format!("could not make {path}: {error}"));
                            self.failed.insert(*id);
                            continue;
                        }
                        self.report.written += 1;
                    }
                    if let Some(stat) = std::fs::metadata(&absolute)
                        .ok()
                        .map(|metadata| disk::stat_of(&metadata))
                    {
                        stats.insert(*id, stat);
                    }
                }
                Kind::File => {
                    let have = ours
                        .entries
                        .get(id)
                        .and_then(|entry| current.as_ref().map(|_| entry.content.clone()));
                    let write = have.as_ref() != Some(&wanted.content);
                    if write {
                        if current.is_some()
                            && !staged.contains_key(id)
                            && !self.unchanged(current.as_deref().unwrap_or_default())
                        {
                            self.failed.insert(*id);
                            self.report.retry = true;
                            continue;
                        }
                        let bytes = self
                            .merged
                            .get(id)
                            .or_else(|| self.contents.get(id).map(|(_, plain)| plain));
                        let Some(bytes) = bytes else {
                            self.report
                                .problems
                                .push(format!("{path} has no content to write"));
                            self.failed.insert(*id);
                            continue;
                        };
                        if let Err(error) = disk::write_atomically(&scratch, &absolute, bytes) {
                            self.report.problems.push(error);
                            self.failed.insert(*id);
                            continue;
                        }
                        self.report.written += 1;
                    }
                    let executable = executable_in(wanted);
                    let had = current
                        .as_ref()
                        .and_then(|current| self.disk.found.get(current))
                        .is_some_and(|found| found.executable);
                    if (write || had != executable)
                        && let Err(error) = disk::set_executable(&absolute, executable)
                    {
                        self.report.problems.push(error);
                    }
                    let walked = current
                        .as_ref()
                        .and_then(|current| self.disk.found.get(current))
                        .map(|found| found.stat);
                    match (write, walked) {
                        (false, Some(stat)) => {
                            stats.insert(*id, stat);
                        }
                        _ => match disk::stat(&absolute) {
                            Some(stat) => {
                                stats.insert(*id, stat);
                            }
                            None => {
                                self.failed.insert(*id);
                            }
                        },
                    }
                }
            }
        }
        let _ = std::fs::remove_dir_all(&staging);
        Ok(stats)
    }

    fn unchanged(&self, path: &str) -> bool {
        let Some(found) = self.disk.found.get(path) else {
            return false;
        };
        match found.kind {
            Kind::Directory => true,
            Kind::File => disk::stat(&self.absolute(path)) == Some(found.stat),
        }
    }

    fn record(&mut self, target: &Tree, layout: &Layout, stats: &HashMap<Uuid, Stat>) {
        let root = self.root();
        let mut records = BTreeMap::new();
        let root_block = &self.remote[&root];
        records.insert(
            root,
            Record {
                parent: None,
                name: String::new(),
                block_type: root_block.block_type,
                executable: false,
                head: root_block.head,
                content: None,
                placed: Placed::Directory {
                    path: String::new(),
                    inode: 0,
                },
            },
        );
        for (id, entry) in &target.entries {
            if *id == root {
                continue;
            }
            if self.failed.contains(id) {
                if let Some(old) = self.index.records.get(id) {
                    records.insert(*id, old.clone());
                }
                continue;
            }
            let Some(block) = self.remote.get(id) else {
                continue;
            };
            let head = self.heads.get(id).copied().unwrap_or(block.head);
            let placed = match (layout.placed.get(id), layout.skipped.get(id)) {
                (Some((path, Kind::Directory)), _) => Placed::Directory {
                    path: path.clone(),
                    inode: stats.get(id).map_or(0, |stat| stat.inode),
                },
                (Some((path, Kind::File)), _) => Placed::File {
                    path: path.clone(),
                    stat: stats.get(id).copied().unwrap_or_default(),
                },
                (None, Some((reason, why))) => Placed::Skipped {
                    reason: *reason,
                    why: why.clone(),
                },
                (None, None) => continue,
            };
            let content = match plain_of(block.block_type) {
                Plain::Directory => None,
                Plain::None => head.map(|head| head.hash()),
                Plain::File => identity_hash(&entry.content),
            };
            records.insert(
                *id,
                Record {
                    parent: entry.parent,
                    name: name_in(entry),
                    block_type: block.block_type,
                    executable: executable_in(entry),
                    head,
                    content,
                    placed,
                },
            );
        }
        for id in &self.failed {
            if !records.contains_key(id)
                && let Some(old) = self.index.records.get(id)
            {
                records.insert(*id, old.clone());
                let mut pending = vec![*id];
                while let Some(parent) = pending.pop() {
                    for (child, record) in &self.index.records {
                        if record.parent == Some(parent) && !records.contains_key(child) {
                            records.insert(*child, record.clone());
                            pending.push(*child);
                        }
                    }
                }
            }
        }
        self.index.records = records;
    }

    fn explain_skipped(&mut self, target: &Tree, layout: &Layout) -> Result<(), String> {
        let mut fixable = Vec::new();
        let mut unexported = Vec::new();
        let mut skipped: Vec<(&Uuid, &(Skip, String))> = layout.skipped.iter().collect();
        skipped.sort_by_key(|(id, _)| self.label(target, **id));
        for (id, (reason, why)) in skipped {
            let line = format!("{} (block {id}): {why}", self.label(target, *id));
            match reason {
                Skip::NoFileForm => unexported.push(line),
                Skip::Inside => {}
                _ => fixable.push(line),
            }
        }
        let errors = self.directory.join(ERRORS_FILE);
        if fixable.is_empty() {
            let _ = std::fs::remove_file(&errors);
        } else {
            let text = format!(
                "be sync could not write these blocks as files here, so they are only in block-app.\n\
                 Rename each one in block-app (or rename or move the other file in the way) and run be sync again.\n\
                 A name must be unique among its siblings ignoring letter case, must not hold / \\ < > : \" | ? * or\n\
                 control characters, must not end with a dot or a space, and must not be a Windows device name\n\
                 such as CON, NUL, COM1 or LPT1. A file made here with the same name becomes a new block; it\n\
                 never replaces the one listed. be sync never writes this file back as a block.\n\n{}\n",
                fixable.join("\n")
            );
            if std::fs::read(&errors).ok().as_deref() != Some(text.as_bytes()) {
                let scratch = disk::temporary(self.directory)?;
                disk::write_atomically(&scratch, &errors, text.as_bytes())?;
            }
        }
        let listed = self.directory.join(STATE_DIRECTORY).join(UNEXPORTED_FILE);
        let text = format!("{}\n", unexported.join("\n"));
        if std::fs::read(&listed).ok().as_deref() != Some(text.as_bytes()) {
            let scratch = disk::temporary(self.directory)?;
            disk::write_atomically(&scratch, &listed, text.as_bytes())?;
        }
        self.report.unexported = unexported.len();
        self.report.skipped = fixable;
        Ok(())
    }
}

enum Failure {
    Retry,
    Refused(String),
}

use Failure::{Refused, Retry};

struct FileBlock {
    block_type: Uuid,
    content: Vec<u8>,
    references: Vec<Uuid>,
    derived: Entries,
}

struct Change {
    id: Uuid,
    file: Option<FileBlock>,
    defined: Entries,
    named: bool,
    parent: Option<Uuid>,
    head: Option<CommitId>,
    name: String,
    executable: bool,
    label: String,
}

impl Change {
    async fn apply<S: ObjectStore>(&self, peer: &Peer<S>) -> Result<Option<CommitId>, Failure> {
        let mut head = self.head;
        if let Some(file) = &self.file {
            match peer
                .save_encoded(
                    self.id,
                    file.block_type,
                    &file.content,
                    file.references.clone(),
                    file.derived.clone(),
                    self.head,
                    Some(self.defined.clone()),
                )
                .await
            {
                Ok(Published::Published(new) | Published::Unchanged(new)) => head = Some(new),
                Ok(Published::Rejected { .. }) => return Err(Retry),
                Err(error) => return Err(Refused(explain(error))),
            }
        } else if self.named {
            let (name, executable) = (self.name.clone(), self.executable);
            match peer
                .define(self.id, move |metadata| {
                    metadata.insert(&NAME, &name);
                    metadata.put(&EXECUTABLE, executable.then_some(&true));
                })
                .await
            {
                Ok(summary) => head = summary.head,
                Err(error) => return Err(Refused(explain(error))),
            }
        }
        if let Some(parent) = self.parent {
            peer.set_parent(self.id, BlockParent::Block(parent))
                .await
                .map_err(|error| Refused(explain(error)))?;
        }
        Ok(head)
    }
}

async fn list_children<S: ObjectStore>(
    peer: &Peer<S>,
    types: &mut Modules,
    folder: Uuid,
    block_type: Uuid,
    gained: &[Uuid],
    lost: &[Uuid],
) -> Result<(), String> {
    for _ in 0..ATTEMPTS {
        let (head, current) = match remote::read(peer, &[(folder, None)]).await?.pop() {
            Some((_, head, current)) => (head, current),
            None => (None, None),
        };
        let mut content = current.clone();
        for (children, change) in [
            (gained, ChildChange::Add as fn(Uuid) -> ChildChange),
            (lost, ChildChange::Delete),
        ] {
            for child in children {
                let edit = Edit::Child(change(*child));
                if let Some(changed) = types
                    .apply(block_type, content.clone(), CLIENT, edit)
                    .await?
                {
                    content = Some(changed);
                }
            }
        }
        let Some(content) = content.filter(|content| Some(content) != current.as_ref()) else {
            return Ok(());
        };
        let description = types
            .describe(block_type, &content, peer.organization())
            .await?;
        match peer
            .publish_encoded(
                folder,
                block_type,
                &content,
                description.references,
                description.derived,
                head,
                None,
            )
            .await
            .map_err(explain)?
        {
            Published::Rejected { .. } => continue,
            Published::Published(_) | Published::Unchanged(_) => return Ok(()),
        }
    }
    Err("it kept changing".to_owned())
}
