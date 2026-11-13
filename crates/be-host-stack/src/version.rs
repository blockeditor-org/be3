use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{Arc, PoisonError},
};

use be_wasm_threads::Mutex;

use be_block::{
    BlockContent, BlockMetadata, Checkout, CheckoutConflict, ConflictKind, MAIN_BRANCH, Repository,
    Root,
    be_block_model::{Document, Model},
    version_control::{Action, Kept, Operation, View, masked, scope_mask},
};
use be_block_client::{ClientError, Peer, Saved, Write};
use be_block_commit::{ChangeId, Commit, CommitId, CommitKind, now_milliseconds};
use be_block_graph::BlockParent;
use be_block_protocol::ErrorCode;
use be_block_store::{Hash, Manifest};
use be_block_vcs::{Entry, Side, Tree};
use be_sdk_api::{
    BlockIdRole, ConflictSide, VersionBranch, VersionChange, VersionChangeKind, VersionCommand,
    VersionCommit, VersionOperation, VersionStatus,
};
use uuid::Uuid;

use super::graph::{Graph, Owner, Query, Scope};
mod reach;

use super::worker::{Shared, Store, node_of};
pub(super) use reach::{LoopCall, LoopCalls, Reach};

const LOG_LENGTH: usize = 100;

#[derive(Default)]
pub struct VersionState {
    pub revision: u64,
    pub status: VersionStatus,
}

#[derive(Default)]
pub(super) struct Versions {
    trees: HashMap<CommitId, Tree>,
    commits: HashMap<CommitId, Commit>,
    /// The blocks this connection watches for statuses: what `followed` names.
    watching: Arc<Mutex<HashSet<Uuid>>>,
    /// The roots whose subtrees the graph lists for statuses (`Owner::Version`).
    roots: BTreeSet<Uuid>,
    /// What each watched checkout's status reads: the checkout, its repository and its root's
    /// subtree. Released when the checkout is no longer watched, and recomputed with its
    /// status, so blocks that left the subtree are let go (`reconcile`).
    followed: HashMap<Uuid, Followed>,
    statuses: Statuses,
}

struct Followed {
    root: Option<Uuid>,
    blocks: HashSet<Uuid>,
}

/// Which watched statuses are current, and which could not be computed, by the inputs they
/// failed with: tried again when those inputs change, or, for a transient failure, when the
/// session's backoff comes due (`refresh`'s `retry`).
#[derive(Default)]
pub(crate) struct Statuses {
    computed: HashMap<Uuid, Signature>,
    failed: HashMap<Uuid, Failed>,
}

/// What `refresh` does with a watched block's status.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Due {
    /// It is current.
    Current,
    /// It failed with these inputs and is not tried again yet; `transient` when the backoff
    /// will try it.
    Waiting { transient: bool },
    /// Compute it.
    Compute,
}

impl Statuses {
    pub(crate) fn due(&self, block: Uuid, signature: Signature, retry: bool) -> Due {
        if self.computed.get(&block) == Some(&signature) {
            return Due::Current;
        }
        match self.failed.get(&block) {
            Some(failed)
                if failed.signature == signature && !(failed.failure.transient() && retry) =>
            {
                Due::Waiting {
                    transient: failed.failure.transient(),
                }
            }
            _ => Due::Compute,
        }
    }

    /// Records a status computed with `signature`; answers the error this bookkeeping showed
    /// for it, which the status should no longer carry.
    pub(crate) fn computed(&mut self, block: Uuid, signature: Signature) -> Option<String> {
        self.computed.insert(block, signature);
        self.failed.remove(&block).and_then(|failed| failed.shown)
    }

    /// Records a status that failed with `signature`; answers the error to show in it once
    /// the failure is permanent or has repeated `SHOWN_AFTER` times.
    pub(crate) fn failed(
        &mut self,
        block: Uuid,
        signature: Signature,
        failure: Failure,
    ) -> Option<String> {
        self.computed.remove(&block);
        let (attempts, previous) = self
            .failed
            .get(&block)
            .map_or((0, None), |failed| (failed.attempts, failed.shown.clone()));
        let attempts = attempts.saturating_add(1);
        let shown = match !failure.transient() || attempts >= SHOWN_AFTER {
            true => Some(format!(
                "Could not read this block's versions: {}",
                failure.message
            )),
            false => previous,
        };
        self.failed.insert(
            block,
            Failed {
                signature,
                failure,
                attempts,
                shown: shown.clone(),
            },
        );
        shown
    }

    /// Forgets which statuses failed for want of the server, once it can be reached again,
    /// and which were computed: the new connection watches nothing, and computing a status
    /// is what watches the blocks it reads.
    pub(crate) fn reconnected(&mut self) {
        self.failed.retain(|_, failed| !failed.failure.transient());
        self.computed.clear();
    }

    /// Forgets which statuses failed for want of an object, once a block's head is
    /// announced: the object may be what was uploaded.
    pub(crate) fn announced(&mut self) {
        self.failed
            .retain(|_, failed| failed.failure.kind != FailureKind::Object);
    }

    /// Forgets the statuses of blocks no longer watched.
    fn keep_only(&mut self, watched: &HashSet<Uuid>) {
        self.computed.retain(|block, _| watched.contains(block));
        self.failed.retain(|block, _| watched.contains(block));
    }
}

/// What a watched block's status is computed from: the graph's revision, the revisions of
/// the block's and its repository's content, and the keyring's (`KeyBook::revision`), so a
/// read that failed for want of a key is tried again once keys arrive.
pub(crate) type Signature = (u64, u64, u64, u64);

struct Failed {
    signature: Signature,
    failure: Failure,
    attempts: u32,
    /// The message this failure put in the status's `error`, to clear once it computes.
    shown: Option<String>,
}

/// How many transient failures in a row a status takes before it shows the error.
const SHOWN_AFTER: u32 = 3;

/// Why a status could not be computed, and what could make trying again succeed.
#[derive(Clone, Debug)]
pub(crate) struct Failure {
    message: String,
    kind: FailureKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureKind {
    /// A request that broke off, timed out or found the server or the disk busy: nothing
    /// announces that it would now succeed, so it is tried again on the session's backoff.
    Transient,
    /// An object neither the store nor the server has yet: tried again when a block's head
    /// is announced (`Versions::announced`), which is how an upload of it arrives.
    Object,
    /// Anything else: a missing key (keys arriving move the signature), bytes that do not
    /// decode. Tried again only when its inputs change.
    Lasting,
}

impl Failure {
    pub(crate) fn client(error: ClientError) -> Self {
        let kind = match &error {
            ClientError::Disconnected(_)
            | ClientError::TimedOut(_)
            | ClientError::Contended
            | ClientError::Refused(ErrorCode::Storage | ErrorCode::SlowDown, _) => {
                FailureKind::Transient
            }
            ClientError::Store(stored) => return Self::stored_as(stored, error.to_string()),
            ClientError::MissingObject(_) | ClientError::Refused(ErrorCode::ObjectNotFound, _) => {
                FailureKind::Object
            }
            _ => FailureKind::Lasting,
        };
        Self {
            message: error.to_string(),
            kind,
        }
    }

    fn stored(error: be_block_store::StoreError) -> Self {
        let message = error.to_string();
        Self::stored_as(&error, message)
    }

    fn stored_as(error: &be_block_store::StoreError, message: String) -> Self {
        let kind = match error {
            be_block_store::StoreError::Io(_) => FailureKind::Transient,
            be_block_store::StoreError::MissingObject(_) => FailureKind::Object,
            _ => FailureKind::Lasting,
        };
        Self { message, kind }
    }

    fn transient(&self) -> bool {
        self.kind == FailureKind::Transient
    }
}

impl From<Failure> for String {
    fn from(failure: Failure) -> String {
        failure.message
    }
}

struct Scanned {
    local: Uuid,
    real: Uuid,
    parent: Option<Uuid>,
    content_type: Uuid,
    metadata: BlockMetadata,
    head: Option<CommitId>,
    clean: bool,
}

#[derive(Default)]
struct Working {
    tree: Tree,
    real: HashMap<Uuid, Uuid>,
    heads: HashMap<Uuid, Option<CommitId>>,
}

struct Snapshot {
    id: CommitId,
    commit: Commit,
    working: Working,
}

impl Snapshot {
    fn change(&self) -> ChangeId {
        self.commit.change_id(self.id)
    }

    fn description(&self) -> String {
        be_block_vcs::message_of(&self.commit)
            .unwrap_or_default()
            .to_owned()
    }

    fn parent(&self) -> Option<CommitId> {
        self.commit.parents.first().copied()
    }
}

struct Step {
    checkout: Option<Uuid>,
    action: Action,
    before: (Option<CommitId>, String),
    after: (Option<CommitId>, String),
}

pub(super) struct Context<'a> {
    pub(super) peer: &'a Arc<Peer<Store>>,
    pub(super) shared: &'a Arc<Mutex<Shared>>,
    pub(super) replicas: Reach<'a>,
}

fn failed(error: ClientError) -> String {
    error.to_string()
}

fn stored(error: be_block_store::StoreError) -> String {
    error.to_string()
}

/// Writes a watched block's status, made from its current one, telling its watchers when it
/// changed.
fn show_status(
    context: &Context<'_>,
    block: Uuid,
    status: impl FnOnce(&mut VersionStatus) -> VersionStatus,
) {
    let mut held = context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let entry = held.versions.entry(block).or_default();
    let mut current = entry.status.clone();
    let status = status(&mut current);
    if entry.status != status {
        entry.status = status;
        entry.revision += 1;
        held.touch(block);
    }
}

fn scope(checkout: Uuid) -> Scope {
    Scope {
        checkout,
        mask: scope_mask(checkout),
    }
}

fn stripped(metadata: &BlockMetadata) -> BlockMetadata {
    let mut metadata = metadata.clone();
    metadata.remove(&be_block::LOCAL_ID);
    metadata
}

fn tagged(metadata: &BlockMetadata, block: Uuid, local: Uuid) -> BlockMetadata {
    let mut metadata = metadata.clone();
    metadata.put(&be_block::LOCAL_ID, (block != local).then_some(&local));
    metadata
}

fn attached(state: &Checkout) -> Result<Uuid, String> {
    state
        .repository
        .ok_or_else(|| "This checkout is not attached to a repository.".to_owned())
}

fn pick<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> T {
    if ours == base {
        theirs.clone()
    } else {
        ours.clone()
    }
}

impl Versions {
    pub(super) async fn run(
        &mut self,
        context: &mut Context<'_>,
        block: Uuid,
        command: VersionCommand,
    ) -> Result<(), String> {
        refresh_heads(context).await?;
        match command {
            VersionCommand::Adopt { block_id } => {
                self.adopt(context, block, Uuid::from_bytes(block_id)).await
            }
            VersionCommand::Finish { description } => {
                let description = description.trim().to_owned();
                if description.is_empty() {
                    return Err("Say what this change does before saving it.".into());
                }
                self.finish(
                    context,
                    block,
                    &description,
                    Action::Finished {
                        description: description.clone(),
                    },
                )
                .await
            }
            VersionCommand::Update => self.update(context, block).await,
            VersionCommand::Switch { branch } => self.switch(context, block, &branch).await,
            VersionCommand::CreateBranch { name } => {
                self.create_branch(context, block, &name).await
            }
            VersionCommand::Merge { branch } => self.merge_branch(context, block, &branch).await,
            VersionCommand::NewCheckout { branch } => {
                self.new_checkout(context, block, &branch).await.map(|_| ())
            }
            VersionCommand::Resolve { block_id, take } => {
                self.resolve(context, block, Uuid::from_bytes(block_id), take)
                    .await
            }
            VersionCommand::Undo { operation } => {
                self.undo(context, block, Uuid::from_bytes(operation)).await
            }
            VersionCommand::Fork => self.fork(context, block).await.map(|_| ()),
            VersionCommand::PullUpstream => self.pull(context, block).await,
            VersionCommand::PushUpstream => self.push(context, block).await,
        }
    }

    async fn tree(
        &mut self,
        peer: &Peer<Store>,
        commit: Option<CommitId>,
    ) -> Result<Tree, Failure> {
        let Some(commit) = commit else {
            return Ok(Tree::default());
        };
        if let Some(tree) = self.trees.get(&commit) {
            return Ok(tree.clone());
        }
        let manifest = self.commit_of(peer, commit).await?.manifest;
        let tree: Tree = be_block_store::decode_value(
            &peer
                .read_manifest(&manifest)
                .await
                .map_err(Failure::client)?,
        )
        .map_err(Failure::stored)?;
        self.trees.insert(commit, tree.clone());
        Ok(tree)
    }

    async fn commit_of(&mut self, peer: &Peer<Store>, commit: CommitId) -> Result<Commit, Failure> {
        if let Some(found) = self.commits.get(&commit) {
            return Ok(found.clone());
        }
        let loaded = peer.load_commit(commit).await.map_err(Failure::client)?;
        self.commits.insert(commit, loaded.clone());
        Ok(loaded)
    }

    async fn working(
        &mut self,
        context: &Context<'_>,
        checkout: Uuid,
        scanned: &[Scanned],
        saved: &Tree,
        root: Option<Uuid>,
    ) -> Result<Working, Failure> {
        let mut working = Working::default();
        working.tree.root = root;
        let scope = scope(checkout);
        for block in scanned {
            let (content, references) = match (block.clean, saved.entries.get(&block.local)) {
                (true, Some(entry)) => (entry.content.clone(), entry.references.clone()),
                _ => match block.head {
                    Some(head) => {
                        let commit = self.commit_of(context.peer, head).await?;
                        let listed = context
                            .peer
                            .references_of(block.real, &commit)
                            .await
                            .map_err(Failure::client)?;
                        let references = with_graph(context.shared, |graph| {
                            listed
                                .iter()
                                .map(|reference| graph.to_local(scope, *reference))
                                .collect()
                        });
                        (Some(commit.manifest), references)
                    }
                    None => (None, Vec::new()),
                },
            };
            working.real.insert(block.local, block.real);
            working.heads.insert(block.real, block.head);
            working.tree.entries.insert(
                block.local,
                Entry {
                    content_type: block.content_type,
                    parent: block.parent,
                    metadata: block.metadata.clone(),
                    content,
                    references,
                },
            );
        }
        working.tree.conflicts = saved
            .conflicts
            .iter()
            .filter(|(block, _)| working.tree.entries.contains_key(block))
            .map(|(block, conflict)| (*block, conflict.clone()))
            .collect();
        Ok(working)
    }

    async fn live(
        &mut self,
        context: &Context<'_>,
        checkout: Uuid,
        state: &Checkout,
    ) -> Result<(Tree, Working), Failure> {
        let saved = self.tree(context.peer, state.working).await?;
        let scanned = with_graph(context.shared, |graph| scan(graph, checkout, state, &saved));
        let root = state.root.map(|root| {
            with_graph(context.shared, |graph| {
                graph.to_local(scope(checkout), root)
            })
        });
        let working = self
            .working(context, checkout, &scanned, &saved, root)
            .await?;
        Ok((saved, working))
    }

    async fn snapshot(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        state: &mut Checkout,
        repository: Uuid,
    ) -> Result<Snapshot, String> {
        let (saved, working) = self.live(context, checkout, state).await?;
        if let Some(id) = state.working
            && working.tree == saved
        {
            let commit = self.commit_of(context.peer, id).await?;
            return Ok(Snapshot {
                id,
                commit,
                working,
            });
        }
        let (parents, identity, description) = match state.working {
            Some(id) => {
                let commit = self.commit_of(context.peer, id).await?;
                let description = be_block_vcs::message_of(&commit)
                    .unwrap_or_default()
                    .to_owned();
                (commit.parents.clone(), commit.change_id(id), description)
            }
            None => (Vec::new(), ChangeId::new(), String::new()),
        };
        let id = self
            .write_snapshot(
                context,
                repository,
                &working.tree,
                &saved,
                parents,
                identity,
                &description,
            )
            .await?;
        let clean = clean_heads(&working, &working.tree);
        change::<Checkout>(context, checkout, |held| {
            held.working = Some(id);
            held.clean = clean.clone().into_iter().collect();
            Ok(())
        })
        .await?;
        state.working = Some(id);
        state.clean = clean.into_iter().collect();
        let commit = self.commit_of(context.peer, id).await?;
        Ok(Snapshot {
            id,
            commit,
            working,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_snapshot(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        tree: &Tree,
        known: &Tree,
        parents: Vec<CommitId>,
        change: ChangeId,
        description: &str,
    ) -> Result<CommitId, String> {
        let key = context.peer.sealing_key(repository).await.map_err(failed)?;
        let vault = context.peer.commits().vault();
        let manifest = tree.write(vault, key).map_err(stored)?;
        let commit = be_block_vcs::snapshot(
            context.peer.commits(),
            key,
            manifest.clone(),
            parents,
            change,
            context.peer.account(),
            now_milliseconds(),
            description,
        )
        .map_err(stored)?;
        let mut held = vec![commit.hash()];
        held.extend(context.peer.objects_of(&manifest).await.map_err(failed)?);
        held.extend(tree_objects(context.peer, tree, known).await?);
        context
            .peer
            .hold(repository, &held, be_block_protocol::Hold::Lasting)
            .await
            .map_err(failed)?;
        self.trees.insert(commit, tree.clone());
        Ok(commit)
    }

    async fn put(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        commit: Commit,
    ) -> Result<CommitId, String> {
        let key = context.peer.sealing_key(repository).await.map_err(failed)?;
        let id = context.peer.commits().put(key, &commit).map_err(stored)?;
        context
            .peer
            .hold(repository, &[id.hash()], be_block_protocol::Hold::Lasting)
            .await
            .map_err(failed)?;
        self.commits.insert(id, commit);
        Ok(id)
    }

    async fn start_change(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        parent: CommitId,
    ) -> Result<CommitId, String> {
        let manifest = self.commit_of(context.peer, parent).await?.manifest;
        let commit = Commit::new(manifest, context.peer.account(), now_milliseconds())
            .with_parents(vec![parent])
            .with_change(ChangeId::new())
            .with_kind(CommitKind::Snapshot(String::new()));
        let id = self.put(context, repository, commit).await?;
        if let Some(tree) = self.trees.get(&parent).cloned() {
            self.trees.insert(id, tree);
        }
        Ok(id)
    }

    async fn merged(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        base: &Tree,
        ours: &Tree,
        theirs: &Tree,
    ) -> Result<Tree, String> {
        let mut plan = be_block_vcs::merge(base, ours, theirs);
        for merge in std::mem::take(&mut plan.contents) {
            let (manifest, references, derived, conflicted) =
                merge_content(context, checkout, &merge).await?;
            if let Some(entry) = plan.tree.entries.get_mut(&merge.block) {
                entry.content = manifest;
                entry.references = references;
                if let Some(derived) = derived {
                    entry.metadata.derived = derived;
                }
            }
            if conflicted {
                plan.tree.conflicts.insert(
                    merge.block,
                    be_block_vcs::Conflict::between(
                        ConflictKind::Content,
                        merge.block,
                        base,
                        ours,
                        theirs,
                    ),
                );
            }
        }
        Ok(plan.tree)
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        state: &Checkout,
        from: &Working,
        to: CommitId,
        tree: &Tree,
        branch: &str,
    ) -> Result<(), String> {
        let (heads, real) = materialize(context, checkout, tree, from).await?;
        let conflicts = conflict_views(context, checkout, state, &from.tree, tree, &real).await?;
        let root = tree.root.and_then(|root| real.get(&root).copied());
        let branch = branch.to_owned();
        change::<Checkout>(context, checkout, |held| {
            held.working = Some(to);
            held.branch = branch.clone();
            held.clean = heads.clone().into_iter().collect();
            held.root = root.or(held.root);
            held.conflicts = conflicts.iter().cloned().collect();
            Ok(())
        })
        .await
    }

    async fn operate(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        step: Step,
        edit: impl Fn(&mut Repository) -> Result<(), String>,
    ) -> Result<(), String> {
        let (id, time, author) = (Uuid::now_v7(), now_milliseconds(), context.peer.account());
        change::<Repository>(context, repository, |held| {
            let branches = (*held.branches).clone();
            edit(held)?;
            held.record(Operation {
                id,
                time,
                author,
                action: step.action.clone(),
                checkout: step.checkout,
                before: View {
                    branches,
                    working: step.before.0,
                    branch: step.before.1.clone(),
                },
                after: View {
                    branches: (*held.branches).clone(),
                    working: step.after.0,
                    branch: step.after.1.clone(),
                },
            });
            Ok(())
        })
        .await
    }

    async fn finish(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        description: &str,
        action: Action,
    ) -> Result<(), String> {
        let mut state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let current = self
            .snapshot(context, checkout, &mut state, repository)
            .await?;
        let parent = current.parent();
        let head = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(&state.branch);
        if head != parent {
            return Err(format!(
                "Newer changes arrived on {}. Bring them in first.",
                state.branch
            ));
        }
        let unchanged = current.commit.parents.len() < 2
            && parent.is_some()
            && self.tree(context.peer, parent).await? == current.working.tree;
        if unchanged {
            return Err("Nothing has changed yet.".into());
        }
        let finished = Commit::new(
            current.commit.manifest.clone(),
            context.peer.account(),
            now_milliseconds(),
        )
        .with_parents(current.commit.parents.clone())
        .with_change(current.change())
        .with_kind(CommitKind::Snapshot(description.to_owned()));
        let finished = self.put(context, repository, finished).await?;
        self.trees.insert(finished, current.working.tree.clone());
        let next = self.start_change(context, repository, finished).await?;
        let branch = state.branch.clone();
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action,
                before: (Some(current.id), branch.clone()),
                after: (Some(next), branch.clone()),
            },
            |held| {
                if held.branch(&branch) != parent {
                    return Err(format!(
                        "Newer changes arrived on {branch} while saving. Bring them in first."
                    ));
                }
                *held = held.with_branch(&branch, Some(finished));
                Ok(())
            },
        )
        .await?;
        change::<Checkout>(context, checkout, |held| {
            held.working = Some(next);
            Ok(())
        })
        .await
    }

    async fn adopt(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        root: Uuid,
    ) -> Result<(), String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        if !held.branches.is_empty() {
            return Err("This repository already has history.".into());
        }
        let Some(node) = with_graph(context.shared, |graph| graph.get(root).cloned()) else {
            return Err("That block is not in this organization.".into());
        };
        if node.content_type == Checkout::CONTENT_TYPE
            || node.content_type == Repository::CONTENT_TYPE
        {
            return Err("A repository or a checkout cannot be versioned itself.".into());
        }
        if with_graph(context.shared, |graph| graph.scope_of(root)).is_some() {
            return Err("That block is already inside a checkout.".into());
        }
        let checkout = Uuid::now_v7();
        let name = node
            .metadata
            .name()
            .map_or_else(|| "Checkout".to_owned(), |name| format!("{name} checkout"));
        create(
            context,
            checkout,
            Checkout::CONTENT_TYPE,
            node.parent.ok_or_else(unplaced)?,
            BlockMetadata::named(name),
        )
        .await?;
        let state = Checkout {
            repository: Some(repository),
            branch: MAIN_BRANCH.to_owned(),
            root: Some(root),
            ..Checkout::default()
        };
        save_new(context, checkout, &state).await?;
        set_parent(context, root, BlockParent::Block(checkout)).await?;
        relist(
            context,
            node.parent.ok_or_else(unplaced)?,
            be_block::ChildChange::Replace {
                old: root,
                new: checkout,
            },
        )
        .await?;
        self.finish(
            context,
            checkout,
            "Started keeping versions",
            Action::StartedVersioning,
        )
        .await
    }

    async fn new_checkout(
        &mut self,
        context: &mut Context<'_>,
        repository: Uuid,
        branch: &str,
    ) -> Result<Uuid, String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let head = held
            .branch(branch)
            .ok_or_else(|| format!("There is no branch named {branch}."))?;
        let tree = self.tree(context.peer, Some(head)).await?;
        let parent = with_graph(context.shared, |graph| {
            graph.get(repository).and_then(|node| node.parent)
        })
        .unwrap_or(BlockParent::Root);
        let checkout = Uuid::now_v7();
        create(
            context,
            checkout,
            Checkout::CONTENT_TYPE,
            parent,
            BlockMetadata::named(format!("{branch} checkout")),
        )
        .await?;
        let (heads, real) = materialize(context, checkout, &tree, &Working::default()).await?;
        let working = self.start_change(context, repository, head).await?;
        let state = Checkout {
            repository: Some(repository),
            branch: branch.to_owned(),
            working: Some(working),
            root: tree.root.and_then(|root| real.get(&root).copied()),
            clean: heads.into_iter().collect(),
            ..Checkout::default()
        };
        save_new(context, checkout, &state).await?;
        relist(context, parent, be_block::ChildChange::Add(checkout)).await?;
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action: Action::NewCheckout {
                    branch: branch.to_owned(),
                },
                before: (None, String::new()),
                after: (Some(working), branch.to_owned()),
            },
            |_| Ok(()),
        )
        .await?;
        Ok(checkout)
    }

    async fn update(&mut self, context: &mut Context<'_>, checkout: Uuid) -> Result<(), String> {
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let head = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(&state.branch)
            .ok_or("The branch this checkout follows is gone.")?;
        let branch = state.branch.clone();
        self.rebase(
            context,
            checkout,
            head,
            &branch,
            Action::BroughtIn {
                branch: branch.clone(),
            },
        )
        .await
    }

    async fn switch(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        branch: &str,
    ) -> Result<(), String> {
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let head = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(branch)
            .ok_or_else(|| format!("There is no branch named {branch}."))?;
        self.rebase(
            context,
            checkout,
            head,
            branch,
            Action::Switched {
                branch: branch.to_owned(),
            },
        )
        .await
    }

    async fn rebase(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        onto: CommitId,
        branch: &str,
        action: Action,
    ) -> Result<(), String> {
        let mut state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let current = self
            .snapshot(context, checkout, &mut state, repository)
            .await?;
        let parent = current.parent();
        if parent == Some(onto) && state.branch == branch {
            return Ok(());
        }
        let base = self.tree(context.peer, parent).await?;
        let theirs = self.tree(context.peer, Some(onto)).await?;
        let tree = self
            .merged(context, checkout, &base, &current.working.tree, &theirs)
            .await?;
        let mut parents = current.commit.parents.clone();
        match parents.first_mut() {
            Some(first) => *first = onto,
            None => parents.push(onto),
        }
        parents.dedup();
        let id = self
            .write_snapshot(
                context,
                repository,
                &tree,
                &current.working.tree,
                parents,
                current.change(),
                &current.description(),
            )
            .await?;
        self.apply(
            context,
            checkout,
            &state,
            &current.working,
            id,
            &tree,
            branch,
        )
        .await?;
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action,
                before: (Some(current.id), state.branch.clone()),
                after: (Some(id), branch.to_owned()),
            },
            |_| Ok(()),
        )
        .await
    }

    async fn merge_branch(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        branch: &str,
    ) -> Result<(), String> {
        let mut state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        if branch == state.branch {
            return Err(format!("This checkout is already on {branch}."));
        }
        let other = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(branch)
            .ok_or_else(|| format!("There is no branch named {branch}."))?;
        let current = self
            .snapshot(context, checkout, &mut state, repository)
            .await?;
        let parent = current.parent();
        context.peer.fetch_history(other).await.map_err(failed)?;
        let ancestor = match parent {
            Some(parent) => {
                context.peer.fetch_history(parent).await.map_err(failed)?;
                let commits = context.peer.commits();
                if current.commit.parents.contains(&other)
                    || commits.is_ancestor(other, parent).map_err(stored)?
                {
                    return Err(format!("Everything on {branch} is already here."));
                }
                commits.common_ancestor(parent, other).map_err(stored)?
            }
            None => None,
        };
        let base = self.tree(context.peer, ancestor).await?;
        let theirs = self.tree(context.peer, Some(other)).await?;
        let tree = self
            .merged(context, checkout, &base, &current.working.tree, &theirs)
            .await?;
        let mut parents = current.commit.parents.clone();
        parents.push(other);
        let id = self
            .write_snapshot(
                context,
                repository,
                &tree,
                &current.working.tree,
                parents,
                current.change(),
                &current.description(),
            )
            .await?;
        let on = state.branch.clone();
        self.apply(context, checkout, &state, &current.working, id, &tree, &on)
            .await?;
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action: Action::Merged {
                    branch: branch.to_owned(),
                },
                before: (Some(current.id), on.clone()),
                after: (Some(id), on),
            },
            |_| Ok(()),
        )
        .await
    }

    async fn create_branch(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        name: &str,
    ) -> Result<(), String> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err("A branch needs a name.".into());
        }
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let working = state
            .working
            .ok_or("Save a change before making a branch.")?;
        let base = self
            .commit_of(context.peer, working)
            .await?
            .parents
            .first()
            .copied()
            .ok_or("Save a change before making a branch.")?;
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action: Action::MadeBranch { name: name.clone() },
                before: (Some(working), state.branch.clone()),
                after: (Some(working), name.clone()),
            },
            |held| {
                if held.branch(&name).is_some() {
                    return Err(format!("There is already a branch named {name}."));
                }
                *held = held.with_branch(&name, Some(base));
                Ok(())
            },
        )
        .await?;
        change::<Checkout>(context, checkout, |state| {
            state.branch = name.clone();
            Ok(())
        })
        .await
    }

    async fn resolve(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        block: Uuid,
        take: ConflictSide,
    ) -> Result<(), String> {
        let mut state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = attached(&state)?;
        let current = self
            .snapshot(context, checkout, &mut state, repository)
            .await?;
        let local = with_graph(context.shared, |graph| {
            graph.to_local(scope(checkout), block)
        });
        let mut tree = current.working.tree.clone();
        let name = tree
            .entries
            .get(&local)
            .and_then(|entry| entry.metadata.name())
            .unwrap_or_else(|| "Untitled".to_owned());
        let (side, kept) = match take {
            ConflictSide::Base => (Some(Side::Base), Kept::Base),
            ConflictSide::Ours => (Some(Side::Ours), Kept::Ours),
            ConflictSide::Theirs => (Some(Side::Theirs), Kept::Theirs),
            ConflictSide::Merged => (None, Kept::AsItIs),
        };
        if !tree.resolve(local, side) {
            return Err("That conflict is already sorted out.".into());
        }
        let id = self
            .write_snapshot(
                context,
                repository,
                &tree,
                &current.working.tree,
                current.commit.parents.clone(),
                current.change(),
                &current.description(),
            )
            .await?;
        let branch = state.branch.clone();
        self.apply(
            context,
            checkout,
            &state,
            &current.working,
            id,
            &tree,
            &branch,
        )
        .await?;
        self.operate(
            context,
            repository,
            Step {
                checkout: Some(checkout),
                action: Action::Resolved { name, kept },
                before: (Some(current.id), branch.clone()),
                after: (Some(id), branch),
            },
            |_| Ok(()),
        )
        .await
    }

    async fn undo(
        &mut self,
        context: &mut Context<'_>,
        block: Uuid,
        operation: Uuid,
    ) -> Result<(), String> {
        let is_checkout = with_graph(context.shared, |graph| {
            graph
                .get(block)
                .is_some_and(|node| node.content_type == Checkout::CONTENT_TYPE)
        });
        let repository = match is_checkout {
            true => attached(&read::<Checkout>(context.peer, block).await?.0)?,
            false => block,
        };
        let target = read::<Repository>(context.peer, repository)
            .await?
            .0
            .operation(operation)
            .cloned()
            .ok_or("That step is no longer in the history.")?;
        let mut step = Step {
            checkout: None,
            action: Action::Undid {
                operation,
                what: target.action.describe(),
            },
            before: (None, String::new()),
            after: (None, String::new()),
        };
        if let Some(checkout) = target.checkout
            && target.before != target.after
            && (target.before.working != target.after.working
                || target.before.branch != target.after.branch)
        {
            match target.before.working {
                None => set_parent(context, checkout, BlockParent::Detached).await?,
                Some(_) => {
                    let (from, to) = self.revert(context, checkout, repository, &target).await?;
                    step.checkout = Some(checkout);
                    step.before = from;
                    step.after = to;
                }
            }
        }
        let names: BTreeSet<&String> = target
            .before
            .branches
            .keys()
            .chain(target.after.branches.keys())
            .collect();
        let moved: Vec<(String, Option<CommitId>, Option<CommitId>)> = names
            .into_iter()
            .filter_map(|name| {
                let before = target.before.branches.get(name).copied();
                let after = target.after.branches.get(name).copied();
                (before != after).then(|| (name.clone(), before, after))
            })
            .collect();
        let skipped = RefCell::new(Vec::new());
        self.operate(context, repository, step, |held| {
            skipped.borrow_mut().clear();
            for (name, before, after) in &moved {
                if held.branch(name) == *after {
                    *held = held.with_branch(name, *before);
                } else {
                    skipped.borrow_mut().push(name.clone());
                }
            }
            Ok(())
        })
        .await?;
        let skipped = skipped.into_inner();
        match skipped.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "These branches moved on since, and were left alone: {}.",
                skipped.join(", ")
            )),
        }
    }

    async fn revert(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        repository: Uuid,
        target: &Operation,
    ) -> Result<((Option<CommitId>, String), (Option<CommitId>, String)), String> {
        let (before, after) = (&target.before, &target.after);
        let then_id = before.working.ok_or("That step cannot be undone.")?;
        let mut state = read::<Checkout>(context.peer, checkout).await?.0;
        let current = self
            .snapshot(context, checkout, &mut state, repository)
            .await?;
        let from = (Some(current.id), state.branch.clone());
        let branch = pick(&after.branch, &state.branch, &before.branch);
        let restored = self.tree(context.peer, Some(then_id)).await?;
        let (id, tree) = if Some(current.id) == after.working {
            (then_id, restored)
        } else {
            let undone = self.tree(context.peer, after.working).await?;
            let tree = self
                .merged(context, checkout, &undone, &current.working.tree, &restored)
                .await?;
            let then = self.commit_of(context.peer, then_id).await?;
            let later = match after.working {
                Some(id) => self.commit_of(context.peer, id).await?,
                None => then.clone(),
            };
            let later_change = after
                .working
                .map_or(then.change_id(then_id), |id| later.change_id(id));
            let parents = pick(&later.parents, &current.commit.parents, &then.parents);
            let change = pick(&later_change, &current.change(), &then.change_id(then_id));
            let description = pick(
                &be_block_vcs::message_of(&later)
                    .unwrap_or_default()
                    .to_owned(),
                &current.description(),
                &be_block_vcs::message_of(&then)
                    .unwrap_or_default()
                    .to_owned(),
            );
            let id = self
                .write_snapshot(
                    context,
                    repository,
                    &tree,
                    &current.working.tree,
                    parents,
                    change,
                    &description,
                )
                .await?;
            (id, tree)
        };
        self.apply(
            context,
            checkout,
            &state,
            &current.working,
            id,
            &tree,
            &branch,
        )
        .await?;
        Ok((from, (Some(id), branch)))
    }

    async fn fork(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<Uuid, String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let node = with_graph(context.shared, |graph| graph.get(repository).cloned())
            .ok_or("That repository is not in this organization.")?;
        let heads: Vec<CommitId> = held.branches.values().copied().collect();
        let objects = self
            .reachable(context.peer, &heads, &HashSet::new())
            .await?;
        let fork = Uuid::now_v7();
        let name = node
            .metadata
            .name()
            .map_or_else(|| "Fork".to_owned(), |name| format!("{name} fork"));
        create(
            context,
            fork,
            Repository::CONTENT_TYPE,
            node.parent.ok_or_else(unplaced)?,
            BlockMetadata::named(name),
        )
        .await?;
        context
            .peer
            .hold(fork, &objects, be_block_protocol::Hold::Lasting)
            .await
            .map_err(failed)?;
        let state = Repository {
            branches: held
                .branches
                .iter()
                .map(|(name, head)| (name.clone(), *head))
                .collect(),
            upstream: Some(repository),
            ..Repository::default()
        };
        save_new(context, fork, &state).await?;
        relist(
            context,
            node.parent.ok_or_else(unplaced)?,
            be_block::ChildChange::Add(fork),
        )
        .await?;
        Ok(fork)
    }

    async fn pull(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<(), String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let upstream = held.upstream.ok_or("This repository has no upstream.")?;
        let theirs = read::<Repository>(context.peer, upstream).await?.0;
        self.carry(context, &theirs, repository, Action::Pulled)
            .await
    }

    async fn push(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<(), String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let upstream = held.upstream.ok_or("This repository has no upstream.")?;
        self.carry(context, &held, upstream, Action::Pushed).await
    }

    async fn carry(
        &mut self,
        context: &mut Context<'_>,
        from: &Repository,
        into: Uuid,
        action: Action,
    ) -> Result<(), String> {
        let current = read::<Repository>(context.peer, into).await?.0;
        let mut known = HashSet::new();
        for head in current.branches.values() {
            context.peer.fetch_history(*head).await.map_err(failed)?;
            known.extend(context.peer.commits().ancestry(*head).map_err(stored)?);
        }
        let mut moved = Vec::new();
        let mut diverged = Vec::new();
        for (name, head) in from.branches.iter() {
            context.peer.fetch_history(*head).await.map_err(failed)?;
            match current.branch(name) {
                Some(existing) if existing == *head => {}
                Some(existing)
                    if !context
                        .peer
                        .commits()
                        .is_ancestor(existing, *head)
                        .map_err(stored)? =>
                {
                    diverged.push(name.clone());
                }
                existing => moved.push((name.clone(), existing, *head)),
            }
        }
        let heads: Vec<CommitId> = moved.iter().map(|(_, _, head)| *head).collect();
        let objects = self.reachable(context.peer, &heads, &known).await?;
        context
            .peer
            .hold(into, &objects, be_block_protocol::Hold::Lasting)
            .await
            .map_err(failed)?;
        if !moved.is_empty() {
            let step = Step {
                checkout: None,
                action,
                before: (None, String::new()),
                after: (None, String::new()),
            };
            self.operate(context, into, step, |repository| {
                for (name, expected, head) in &moved {
                    if repository.branch(name) == *expected {
                        *repository = repository.with_branch(name, Some(*head));
                    }
                }
                Ok(())
            })
            .await?;
        }
        match diverged.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "These branches have diverged and were left alone: {}.",
                diverged.join(", ")
            )),
        }
    }

    async fn reachable(
        &mut self,
        peer: &Peer<Store>,
        heads: &[CommitId],
        known: &HashSet<CommitId>,
    ) -> Result<Vec<Hash>, String> {
        let mut objects = Vec::new();
        let mut seen = HashSet::new();
        let mut frontier: Vec<CommitId> = heads.to_vec();
        while let Some(commit) = frontier.pop() {
            if known.contains(&commit) || !seen.insert(commit) {
                continue;
            }
            let loaded = self.commit_of(peer, commit).await?;
            objects.push(commit.hash());
            objects.extend(peer.objects_of(&loaded.manifest).await.map_err(failed)?);
            let tree = self.tree(peer, Some(commit)).await?;
            objects.extend(tree_objects(peer, &tree, &Tree::default()).await?);
            frontier.extend(loaded.parents);
        }
        objects.sort_unstable();
        objects.dedup();
        Ok(objects)
    }

    pub(super) fn watch(shared: &Arc<Mutex<Shared>>, block: Uuid) {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .version_watch
            .insert(block);
    }

    /// Stops computing a block's status. The loop refreshes the statuses each round, so the
    /// round that runs this command lets go of what the status read (`refresh`).
    pub(super) fn unwatch(shared: &Arc<Mutex<Shared>>, block: Uuid) {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .version_watch
            .remove(&block);
    }

    /// Watches what the followed checkouts read and lets go of the rest, and lists the
    /// subtrees of their roots only.
    fn reconcile(&mut self, context: &Context<'_>) {
        let roots: BTreeSet<Uuid> = self
            .followed
            .values()
            .filter_map(|followed| followed.root)
            .collect();
        if roots != self.roots {
            let queries = roots.iter().map(|root| Query::Subtree(*root)).collect();
            self.roots = roots;
            context
                .shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .want(Owner::Version, queries);
        }
        let wanted: HashSet<Uuid> = self
            .followed
            .values()
            .flat_map(|followed| followed.blocks.iter().copied())
            .collect();
        let attached = context.peer.is_attached();
        let connection = context.peer.connection();
        let mut watching = self.watching.lock().unwrap_or_else(PoisonError::into_inner);
        // Detached, what is watched was watched on a connection that is gone, and the next one
        // starts from nothing (`reconnected`): only forget it, or the release would come off
        // another's watch on the next connection.
        watching.retain(|block| {
            let kept = wanted.contains(block);
            if !kept && attached {
                connection.unwatch(*block);
            }
            kept
        });
        if attached {
            for block in wanted {
                if watching.insert(block) {
                    connection.watch(block);
                }
            }
        }
    }

    /// Recomputes the status of every watched block whose inputs changed. A status that
    /// fails is remembered with the inputs it failed with and not computed again until they
    /// change, which their own events announce. A transient failure (a request to the server
    /// that broke off or timed out: nothing announces that it would now succeed) is tried again
    /// when `retry` is set, which the session sets on a backoff; answers whether one awaits it.
    /// A permanent failure, or a transient one that keeps failing, shows its reason in the
    /// status's `error`.
    pub(super) async fn refresh(&mut self, context: &mut Context<'_>, retry: bool) -> bool {
        let mut awaiting = false;
        let watched: HashSet<Uuid> = context
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .version_watch
            .clone();
        let before = self.followed.len();
        self.followed
            .retain(|checkout, _| watched.contains(checkout));
        self.statuses.keep_only(&watched);
        if self.followed.len() != before {
            self.reconcile(context);
        }
        for block in watched {
            let signature = {
                let held = context
                    .shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                let revision_of = |block: Option<Uuid>| {
                    block
                        .and_then(|block| held.blocks.get(&block))
                        .map_or(0, |content| content.revision)
                };
                let repository = checkout_of(&held, block).and_then(|state| state.repository);
                (
                    held.graph.revision,
                    revision_of(Some(block)),
                    revision_of(repository),
                    context.peer.keys().revision(),
                )
            };
            match self.statuses.due(block, signature, retry) {
                Due::Current => continue,
                Due::Waiting { transient } => {
                    awaiting |= transient;
                    continue;
                }
                Due::Compute => {}
            }
            match self.status(context, block).await {
                Ok(status) => {
                    let shown = self.statuses.computed(block, signature);
                    show_status(context, block, |entry| {
                        let error = match entry.error.take() {
                            Some(error) if Some(&error) == shown.as_ref() => None,
                            error => error,
                        };
                        VersionStatus {
                            busy: entry.busy,
                            error,
                            ..status
                        }
                    });
                }
                Err(failure) => {
                    awaiting |= failure.transient();
                    if let Some(message) = self.statuses.failed(block, signature, failure) {
                        show_status(context, block, |entry| VersionStatus {
                            error: Some(message),
                            ..entry.clone()
                        });
                    }
                }
            }
        }
        awaiting
    }

    pub(super) fn reconnected(&mut self) {
        // The new connection watches nothing yet.
        self.watching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.statuses.reconnected();
    }

    pub(super) fn announced(&mut self) {
        self.statuses.announced();
    }

    async fn status(
        &mut self,
        context: &mut Context<'_>,
        block: Uuid,
    ) -> Result<VersionStatus, Failure> {
        let (checkout, repository) = {
            let held = context
                .shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            (
                checkout_of(&held, block),
                content_of::<Repository>(&held, block),
            )
        };
        if let Some(repository) = repository {
            return Ok(VersionStatus {
                branches: branches(&repository),
                log: self
                    .log(
                        context.peer,
                        repository.branches.values().copied().collect(),
                    )
                    .await,
                operations: operations(&repository),
                ..VersionStatus::default()
            });
        }
        let Some((state, repository_id)) = checkout.and_then(|state| {
            let repository = state.repository?;
            Some((state, repository))
        }) else {
            if self.followed.remove(&block).is_some() {
                self.reconcile(context);
            }
            return Ok(VersionStatus::default());
        };
        self.follow(context, block, &state, repository_id).await;
        let repository = {
            let held = context
                .shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            content_of::<Repository>(&held, repository_id)
        };
        let (_, working) = self.live(context, block, &state).await?;
        let commit = match state.working {
            Some(id) => Some(self.commit_of(context.peer, id).await?),
            None => None,
        };
        let parent = commit
            .as_ref()
            .and_then(|commit| commit.parents.first().copied());
        let parent_tree = self.tree(context.peer, parent).await?;
        let changes = with_graph(context.shared, |graph| {
            changes(graph, &working, &parent_tree, block)
        });
        let head = repository
            .as_ref()
            .and_then(|repository| repository.branch(&state.branch));
        let current = state
            .working
            .zip(commit)
            .map(|(id, commit)| version_commit(id, &commit, !working.tree.conflicts.is_empty()));
        Ok(VersionStatus {
            behind: repository.is_some() && head.is_some() && head != parent,
            branches: repository.as_ref().map(branches).unwrap_or_default(),
            changes,
            working: current,
            log: self.log(context.peer, head.into_iter().collect()).await,
            operations: repository.as_ref().map(operations).unwrap_or_default(),
            ..VersionStatus::default()
        })
    }

    async fn follow(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        state: &Checkout,
        repository: Uuid,
    ) {
        context
            .replicas
            .open(
                (context.peer, context.shared),
                repository,
                Repository::CONTENT_TYPE,
                true,
            )
            .await;
        let mut blocks: HashSet<Uuid> = [checkout, repository].into_iter().collect();
        if let Some(root) = state.root {
            blocks.extend(with_graph(context.shared, |graph| graph.subtree(root)));
        }
        self.followed.insert(
            checkout,
            Followed {
                root: state.root,
                blocks,
            },
        );
        self.reconcile(context);
    }

    async fn log(&mut self, peer: &Peer<Store>, heads: Vec<CommitId>) -> Vec<VersionCommit> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut frontier = heads;
        while let Some(commit) = frontier.pop() {
            if found.len() >= LOG_LENGTH || !seen.insert(commit) {
                continue;
            }
            let Ok(loaded) = self.commit_of(peer, commit).await else {
                continue;
            };
            frontier.extend(loaded.parents.iter().copied());
            let conflicted = self
                .tree(peer, Some(commit))
                .await
                .is_ok_and(|tree| !tree.conflicts.is_empty());
            found.push(version_commit(commit, &loaded, conflicted));
        }
        found.sort_by_key(|commit| std::cmp::Reverse(commit.time));
        found
    }
}

fn version_commit(id: CommitId, commit: &Commit, conflicted: bool) -> VersionCommit {
    VersionCommit {
        id: *id.hash().as_bytes(),
        change: commit.change_id(id).uuid().into_bytes(),
        parents: commit
            .parents
            .iter()
            .map(|parent| *parent.hash().as_bytes())
            .collect(),
        author: commit.author.into_bytes(),
        time: commit.time,
        message: be_block_vcs::message_of(commit)
            .unwrap_or_default()
            .to_owned(),
        conflicted,
    }
}

fn operations(repository: &Repository) -> Vec<VersionOperation> {
    let undone = repository.undone();
    repository
        .operations
        .iter()
        .rev()
        .take(LOG_LENGTH)
        .map(|item| &item.value)
        .map(|operation| VersionOperation {
            id: operation.id.into_bytes(),
            time: operation.time,
            author: operation.author.into_bytes(),
            description: operation.action.describe(),
            undone: undone.contains(&operation.id),
        })
        .collect()
}

fn branches(repository: &Repository) -> Vec<VersionBranch> {
    repository
        .branches
        .iter()
        .map(|(name, head)| VersionBranch {
            name: name.clone(),
            head: *head.hash().as_bytes(),
        })
        .collect()
}

fn checkout_of(shared: &Shared, block: Uuid) -> Option<Checkout> {
    content_of::<Checkout>(shared, block)
}

fn content_of<R: Root + Model>(shared: &Shared, block: Uuid) -> Option<R> {
    let content = shared.blocks.get(&block)?;
    if content.content_type != R::CONTENT_TYPE {
        return None;
    }
    Document::<R>::decode(content.bytes())
        .ok()
        .map(|document| document.root())
}

fn with_graph<T>(shared: &Arc<Mutex<Shared>>, read: impl FnOnce(&Graph) -> T) -> T {
    read(&shared.lock().unwrap_or_else(PoisonError::into_inner).graph)
}

fn scan(graph: &Graph, checkout: Uuid, state: &Checkout, saved: &Tree) -> Vec<Scanned> {
    let Some(root) = state.root else {
        return Vec::new();
    };
    let scope = scope(checkout);
    graph
        .subtree(root)
        .into_iter()
        .filter_map(|real| {
            let node = graph.get(real)?;
            let local = graph.to_local(scope, real);
            let parent = match real == root {
                true => None,
                false => node
                    .parent
                    .and_then(BlockParent::block)
                    .map(|parent| graph.to_local(scope, parent)),
            };
            let clean = match node.head {
                Some(head) => state.clean.get(&real) == Some(&head),
                None => saved
                    .entries
                    .get(&local)
                    .is_some_and(|entry| entry.content.is_none()),
            };
            Some(Scanned {
                local,
                real,
                parent,
                content_type: node.content_type,
                metadata: stripped(&node.metadata),
                head: node.head,
                clean,
            })
        })
        .collect()
}

fn changes(graph: &Graph, working: &Working, parent: &Tree, checkout: Uuid) -> Vec<VersionChange> {
    let scope = scope(checkout);
    be_block_vcs::diff(parent, &working.tree)
        .into_iter()
        .filter_map(|difference| {
            let (kind, entry) = match difference.change {
                be_block_vcs::Change::Added => (
                    VersionChangeKind::Added,
                    working.tree.entries.get(&difference.block)?,
                ),
                be_block_vcs::Change::Removed => (
                    VersionChangeKind::Removed,
                    parent.entries.get(&difference.block)?,
                ),
                be_block_vcs::Change::Changed { content, .. } => (
                    match content {
                        true => VersionChangeKind::Modified,
                        false => VersionChangeKind::Moved,
                    },
                    working.tree.entries.get(&difference.block)?,
                ),
            };
            let real = working
                .real
                .get(&difference.block)
                .copied()
                .unwrap_or_else(|| graph.to_real(scope, difference.block, BlockIdRole::Existing));
            Some(VersionChange {
                block_id: real.into_bytes(),
                block_type: entry.content_type.into_bytes(),
                name: entry.metadata.name(),
                kind,
            })
        })
        .collect()
}

fn clean_heads(working: &Working, tree: &Tree) -> BTreeMap<Uuid, CommitId> {
    working
        .real
        .iter()
        .filter(|(local, _)| tree.entries.contains_key(*local))
        .filter_map(|(_, real)| Some((*real, (*working.heads.get(real)?)?)))
        .collect()
}

async fn conflict_views(
    context: &mut Context<'_>,
    checkout: Uuid,
    state: &Checkout,
    from: &Tree,
    to: &Tree,
    real: &HashMap<Uuid, Uuid>,
) -> Result<Vec<CheckoutConflict>, String> {
    let mut views = Vec::new();
    let mut kept = HashSet::new();
    for (local, conflict) in &to.conflicts {
        let Some(block) = real.get(local).copied() else {
            continue;
        };
        let existing = state.conflicts.iter().find(|held| held.block == block);
        if let Some(existing) = existing
            && from.conflicts.get(local) == Some(conflict)
        {
            kept.insert(block);
            views.push((**existing).clone());
            continue;
        }
        let mut recorded = CheckoutConflict {
            block,
            content_type: conflict
                .sides()
                .next()
                .map(|entry| entry.content_type)
                .unwrap_or_default(),
            kind: conflict.kind,
            ..CheckoutConflict::default()
        };
        if conflict.kind == ConflictKind::Content {
            let name = to
                .entries
                .get(local)
                .and_then(|entry| entry.metadata.name())
                .unwrap_or_else(|| "Block".to_owned());
            let mut made = Vec::new();
            for (side, label) in [
                (&conflict.base, "before"),
                (&conflict.ours, "yours"),
                (&conflict.theirs, "incoming"),
            ] {
                made.push(match side {
                    Some(entry) => {
                        version_block(
                            context,
                            checkout,
                            entry.content_type,
                            entry.content.clone(),
                            entry.references.clone(),
                            real,
                            format!("{name} ({label})"),
                        )
                        .await?
                    }
                    None => None,
                });
            }
            recorded.base = made[0];
            recorded.ours = made[1];
            recorded.theirs = made[2];
        }
        views.push(recorded);
    }
    for held in state.conflicts.iter() {
        if !kept.contains(&held.block) {
            for version in held.versions() {
                set_parent(context, version, BlockParent::Detached).await?;
            }
        }
    }
    Ok(views)
}

async fn refresh_heads(context: &mut Context<'_>) -> Result<(), String> {
    context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .relist_owner(Owner::Version);
    super::worker::fetch_listings(context.peer, context.shared).await;
    Ok(())
}

async fn read<R: Root + Model + Default>(
    peer: &Peer<Store>,
    block: Uuid,
) -> Result<(R, Option<CommitId>), String> {
    let head = peer.remote_head(block).await.map_err(failed)?;
    let content = match head {
        Some(head) => peer
            .open_commit::<Document<R>>(head)
            .await
            .map_err(failed)?
            .root(),
        None => R::default(),
    };
    Ok((content, head))
}

async fn change<R: Root + Model + Default>(
    context: &mut Context<'_>,
    block: Uuid,
    edit: impl Fn(&mut R) -> Result<(), String>,
) -> Result<(), String> {
    for _ in 0..8 {
        let (mut content, head) = read::<R>(context.peer, block).await?;
        edit(&mut content)?;
        let document = Document::new(&content);
        match context
            .peer
            .save(block, &document, head)
            .await
            .map_err(failed)?
        {
            Saved::Published(head) | Saved::Unchanged(head) => {
                settle(context, block, head).await?;
                return Ok(());
            }
            Saved::Rejected { .. } => {}
        }
    }
    Err("Someone else kept changing it at the same time. Try again.".into())
}

async fn relist(
    context: &mut Context<'_>,
    parent: BlockParent,
    change: be_block::ChildChange,
) -> Result<(), String> {
    let BlockParent::Block(parent) = parent else {
        return Ok(());
    };
    let content_type = with_graph(context.shared, |graph| {
        graph.get(parent).map(|node| node.content_type)
    })
    .ok_or("Version control can only put a block somewhere it knows.")?;
    // Whole bytes carry only the first part of a type that keeps parts after it (chat's closed
    // segments), so a place of that kind cannot be rewritten from bytes yet.
    if context
        .replicas
        .kind(content_type)
        .await
        .is_some_and(|kind| !kind.whole_bytes)
        && super::worker::has_later_parts(context.peer, parent)
            .await
            .map_err(failed)?
    {
        return Err("That place keeps parts that version control cannot carry yet.".into());
    }
    let change = be_sdk_type_api::Change::Child(super::api_child(change));
    let ctx = (context.peer, context.shared);
    for _ in 0..8 {
        let head = context.peer.remote_head(parent).await.map_err(failed)?;
        let current = match head {
            Some(head) => {
                let commit = context.peer.load_commit(head).await.map_err(failed)?;
                Some(
                    context
                        .peer
                        .read_manifest(&commit.manifest)
                        .await
                        .map_err(failed)?,
                )
            }
            None => None,
        };
        let bytes = context
            .replicas
            .apply(ctx, content_type, current, 0, change.clone())
            .await?
            .ok_or("That place cannot hold another block.")?;
        let description = context
            .replicas
            .describe(ctx, content_type, &bytes)
            .await
            .ok_or("Version control could not describe that place.")?;
        match context
            .peer
            .write(
                parent,
                Write::encoded(content_type, &bytes, head)
                    .described(description.references, description.derived),
            )
            .await
            .map_err(failed)?
        {
            Saved::Published(head) | Saved::Unchanged(head) => {
                return settle(context, parent, head).await;
            }
            Saved::Rejected { .. } => {}
        }
    }
    Err("Someone else kept changing it at the same time. Try again.".into())
}

async fn save_new<R: Root + Model + Default + Clone>(
    context: &mut Context<'_>,
    block: Uuid,
    content: &R,
) -> Result<(), String> {
    change::<R>(context, block, |held| {
        *held = content.clone();
        Ok(())
    })
    .await
}

async fn settle(context: &mut Context<'_>, block: Uuid, head: CommitId) -> Result<(), String> {
    context.replicas.moved_here(block, head).await;
    context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .set_head(block, Some(head));
    Ok(())
}

fn unplaced() -> String {
    "You cannot see where this block is.".to_owned()
}

/// Makes a block under `parent`, or moves there one already made under the same id: a block
/// coming back into the tree, or one another device's command made first. Answers what such a
/// block holds (its head and defined metadata), which `publish` writes over as read, merging
/// what changed after.
async fn create(
    context: &mut Context<'_>,
    block: Uuid,
    content_type: Uuid,
    parent: BlockParent,
    metadata: BlockMetadata,
) -> Result<Option<(Option<CommitId>, be_block::Entries)>, String> {
    let (summary, found) = match context
        .peer
        .create_block(block, content_type, parent, &metadata, false)
        .await
    {
        Ok(summary) => (summary, false),
        Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
            context
                .peer
                .set_parent(block, parent)
                .await
                .map_err(failed)?;
            (context.peer.summary(block).await.map_err(failed)?, true)
        }
        Err(error) => return Err(failed(error)),
    };
    let node = node_of(context.peer, &summary);
    let existing = found.then(|| (summary.head, node.metadata.defined.clone()));
    context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .put(node, summary.revision);
    Ok(existing)
}

async fn set_parent(
    context: &mut Context<'_>,
    block: Uuid,
    parent: BlockParent,
) -> Result<(), String> {
    context
        .peer
        .set_parent(block, parent)
        .await
        .map_err(failed)?;
    let summary = context.peer.summary(block).await.map_err(failed)?;
    let node = node_of(context.peer, &summary);
    context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .put(node, summary.revision);
    Ok(())
}

/// Holds the next version command's write of a block, once, until the test lets it go: what a
/// test does meanwhile, the loop still running, lands between the command reading the block and
/// writing it (`Shared::before_write`).
#[cfg(test)]
pub(crate) type HeldWrite = (
    tokio::sync::oneshot::Sender<Uuid>,
    tokio::sync::oneshot::Receiver<()>,
);

/// What a block holds when version control writes it.
struct Written<'a> {
    manifest: Option<Manifest>,
    references: Vec<Uuid>,
    metadata: &'a BlockMetadata,
}

/// What the command read of a block before it wrote it: its head and defined metadata. `None`
/// for a block it makes, or one it found already made under the same id, which it writes over:
/// a block coming back into the tree takes what the tree says.
struct Read<'a> {
    head: Option<CommitId>,
    /// The content the command read, the base of a merge when its head is not known (the
    /// mirror does not always know the head of a block not open here).
    content: Option<Manifest>,
    defined: &'a be_block::Entries,
}

/// Writes `written` as `block`'s content and defined metadata. Edits go on while a version
/// command runs in the lane, so the block may have moved on since the command read it, or move
/// on while this writes: what it moved to is merged with what this writes, so no edit is lost.
/// The content merges three ways from the two heads' common ancestor (a conflict stays in the
/// content, as the type merges it), and the defined metadata key by key: the command's value
/// where it changed the key, the block's otherwise.
async fn publish(
    context: &mut Context<'_>,
    block: Uuid,
    written: Written<'_>,
    read: Option<Read<'_>>,
) -> Result<Option<CommitId>, String> {
    let no_entries = be_block::Entries::new();
    let read_defined = read.as_ref().map_or(&no_entries, |read| read.defined);
    let defined =
        |held: &be_block::Entries| merge_defined(read_defined, &written.metadata.defined, held);
    let Some(manifest) = written.manifest else {
        let summary = context
            .peer
            .define(block, |held| held.defined = defined(&held.defined))
            .await
            .map_err(failed)?;
        let node = node_of(context.peer, &summary);
        context
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .graph
            .put(node, summary.revision);
        return Ok(summary.head);
    };
    #[cfg(test)]
    let hook = context
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .before_write
        .take();
    #[cfg(test)]
    if let Some((reached, go)) = hook {
        let _ = reached.send(block);
        let _ = go.await;
    }
    let mut expected = context.peer.remote_head(block).await.map_err(failed)?;
    let mut made_from = read.as_ref().map_or(expected, |read| read.head);
    let mut read_content = read.as_ref().and_then(|read| read.content.clone());
    let (mut manifest, mut references, mut derived) = (
        manifest,
        written.references,
        written.metadata.derived.clone(),
    );
    // Rejected writes and waits for this device's own edit to be sent, each bounded on its own.
    let (mut rejected, mut waited) = (0, 0);
    loop {
        if let Some(theirs) = expected
            && made_from != expected
        {
            match merge_over(
                context,
                block,
                (made_from, read_content.as_ref()),
                &manifest,
                theirs,
            )
            .await?
            {
                Some(merged) => {
                    (manifest, references, derived) = merged;
                    made_from = expected;
                    read_content = None;
                }
                None => {
                    // What this was made from is this device's edit, not sent yet, on top of
                    // `expected`: what this writes holds it already. Once it is sent, it is the
                    // head this writes over, so nothing of it is merged in twice.
                    if waited == 2 {
                        return Err(
                            "This device's edit of a block could not be sent. Try again \
                                    once it is."
                                .into(),
                        );
                    }
                    waited += 1;
                    let ctx = (context.peer, context.shared);
                    context.replicas.content_sent(ctx, block).await;
                    expected = context.peer.remote_head(block).await.map_err(failed)?;
                    continue;
                }
            }
        }
        let held = node_of(
            context.peer,
            &context.peer.summary(block).await.map_err(failed)?,
        )
        .metadata
        .defined;
        match context
            .peer
            .write(
                block,
                Write::written(manifest.clone(), expected)
                    .described(references.clone(), derived.clone())
                    .defined(defined(&held)),
            )
            .await
            .map_err(failed)?
        {
            Saved::Published(head) | Saved::Unchanged(head) => {
                settle(context, block, head).await?;
                let summary = context.peer.summary(block).await.map_err(failed)?;
                let node = node_of(context.peer, &summary);
                context
                    .shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .graph
                    .put(node, summary.revision);
                return Ok(Some(head));
            }
            Saved::Rejected { head } => {
                rejected += 1;
                if rejected == 4 {
                    return Err(
                        "Someone else kept changing a block at the same time. Try again.".into(),
                    );
                }
                expected = head;
            }
        }
    }
}

/// The command's defined entries over the block's: the command's value for each key it
/// changed from what it read, the block's for the rest.
fn merge_defined(
    read: &be_block::Entries,
    ours: &be_block::Entries,
    held: &be_block::Entries,
) -> be_block::Entries {
    let mut merged = held.clone();
    for key in read.keys().chain(ours.keys()) {
        if read.get(key) == ours.get(key) {
            continue;
        }
        match ours.get(key) {
            Some(value) => merged.insert(*key, value.clone()),
            None => merged.remove(key),
        };
    }
    merged
}

/// `ours`, made from `block`'s content at `made_from` (or from `read_content`, when that head
/// is not known or shares no history with `theirs`), merged with its content at `theirs`:
/// the merge written, its references and derived metadata. `None` when there is nothing to
/// merge: `theirs` is what `made_from` was built on, an edit of this device not sent yet.
async fn merge_over(
    context: &mut Context<'_>,
    block: Uuid,
    (made_from, read_content): (Option<CommitId>, Option<&Manifest>),
    ours: &Manifest,
    theirs: CommitId,
) -> Result<Option<(Manifest, Vec<Uuid>, be_block::Entries)>, String> {
    let content_type = context
        .peer
        .known_summary(block)
        .map(|summary| summary.content_type)
        .ok_or("Version control does not know a block it writes.")?;
    let base = match made_from {
        Some(made_from) => {
            context.peer.fetch_history(theirs).await.map_err(failed)?;
            context
                .peer
                .fetch_history(made_from)
                .await
                .map_err(failed)?;
            let commits = context.peer.commits();
            if commits.is_ancestor(theirs, made_from).map_err(stored)? {
                return Ok(None);
            }
            match commits.common_ancestor(made_from, theirs).map_err(stored)? {
                Some(base) => Some(
                    context
                        .peer
                        .load_commit(base)
                        .await
                        .map_err(failed)?
                        .manifest,
                ),
                None => read_content.cloned(),
            }
        }
        None => read_content.cloned(),
    };
    let theirs = context
        .peer
        .load_commit(theirs)
        .await
        .map_err(failed)?
        .manifest;
    let ctx = (context.peer, context.shared);
    let merged = context
        .replicas
        .merge(
            ctx,
            content_type,
            [base.as_ref(), Some(ours), Some(&theirs)],
        )
        .await
        .map_err(failed)?
        .ok_or("A block changed while version control wrote it, and its type cannot merge them.")?;
    let manifest = context
        .peer
        .write_content(block, content_type, merged.parts())
        .await
        .map_err(failed)?;
    Ok(Some((
        manifest,
        merged.content.references.clone(),
        merged.content.derived.clone(),
    )))
}

async fn materialize(
    context: &mut Context<'_>,
    checkout: Uuid,
    target: &Tree,
    working: &Working,
) -> Result<(BTreeMap<Uuid, CommitId>, HashMap<Uuid, Uuid>), String> {
    let mask = scope_mask(checkout);
    let real: HashMap<Uuid, Uuid> = target
        .entries
        .keys()
        .map(|local| {
            (
                *local,
                working
                    .real
                    .get(local)
                    .copied()
                    .unwrap_or_else(|| masked(*local, mask)),
            )
        })
        .collect();
    let real_of = |local: Uuid| real.get(&local).copied().unwrap_or(local);
    let mut heads = BTreeMap::new();
    for local in placement_order(target) {
        let entry = &target.entries[&local];
        let block = real_of(local);
        let parent = BlockParent::Block(entry.parent.map_or(checkout, real_of));
        let references: Vec<Uuid> = entry
            .references
            .iter()
            .map(|reference| real_of(*reference))
            .collect();
        let head = match working.tree.entries.get(&local) {
            Some(current) => {
                if current.parent != entry.parent {
                    set_parent(context, block, parent).await?;
                }
                let metadata = tagged(&entry.metadata, block, local);
                match current.content == entry.content
                    && current.metadata.defined == entry.metadata.defined
                {
                    true => working.heads.get(&block).copied().flatten(),
                    false => {
                        let read = Read {
                            head: working.heads.get(&block).copied().flatten(),
                            content: current.content.clone(),
                            defined: &current.metadata.defined,
                        };
                        publish(
                            context,
                            block,
                            Written {
                                manifest: entry.content.clone(),
                                references,
                                metadata: &metadata,
                            },
                            Some(read),
                        )
                        .await?
                    }
                }
            }
            None => {
                let metadata = tagged(&entry.metadata, block, local);
                let existing =
                    create(context, block, entry.content_type, parent, metadata.clone()).await?;
                let read = existing.as_ref().map(|(head, defined)| Read {
                    head: *head,
                    content: None,
                    defined,
                });
                publish(
                    context,
                    block,
                    Written {
                        manifest: entry.content.clone(),
                        references,
                        metadata: &metadata,
                    },
                    read,
                )
                .await?
            }
        };
        if let Some(head) = head {
            heads.insert(block, head);
        }
    }
    for (local, block) in &working.real {
        if !target.entries.contains_key(local) {
            set_parent(context, *block, BlockParent::Detached).await?;
        }
    }
    Ok((heads, real))
}

fn placement_order(tree: &Tree) -> Vec<Uuid> {
    let mut order = Vec::with_capacity(tree.entries.len());
    let mut placed = HashSet::new();
    let mut frontier: Vec<Uuid> = tree.root.into_iter().collect();
    while let Some(block) = frontier.pop() {
        if !tree.entries.contains_key(&block) || !placed.insert(block) {
            continue;
        }
        order.push(block);
        frontier.extend(tree.children(block));
    }
    order
}

async fn merge_content(
    context: &mut Context<'_>,
    checkout: Uuid,
    merge: &be_block_vcs::ContentMerge,
) -> Result<(Option<Manifest>, Vec<Uuid>, Option<be_block::Entries>, bool), String> {
    let merged = context
        .replicas
        .merge(
            (context.peer, context.shared),
            merge.content_type,
            [&merge.base, &merge.ours, &merge.theirs].map(Option::as_ref),
        )
        .await
        .map_err(failed)?;
    let Some(merged) = merged else {
        return Ok((merge.ours.clone(), Vec::new(), None, true));
    };
    let manifest = context
        .peer
        .write_content(checkout, merge.content_type, merged.parts())
        .await
        .map_err(failed)?;
    Ok((
        Some(manifest),
        merged.content.references.clone(),
        Some(merged.content.derived.clone()),
        merged.conflicts > 0,
    ))
}

async fn version_block(
    context: &mut Context<'_>,
    checkout: Uuid,
    content_type: Uuid,
    manifest: Option<Manifest>,
    references: Vec<Uuid>,
    real: &HashMap<Uuid, Uuid>,
    name: String,
) -> Result<Option<Uuid>, String> {
    let block = Uuid::now_v7();
    let mut metadata = BlockMetadata::named(name);
    if let Some(manifest) = &manifest {
        let bytes = context.peer.read_manifest(manifest).await.map_err(failed)?;
        metadata.derived = context
            .replicas
            .describe((context.peer, context.shared), content_type, &bytes)
            .await
            .map(|description| description.derived)
            .unwrap_or_default();
    }
    let existing = create(
        context,
        block,
        content_type,
        BlockParent::Block(checkout),
        metadata.clone(),
    )
    .await?;
    let references = references
        .iter()
        .map(|reference| real.get(reference).copied().unwrap_or(*reference))
        .collect();
    let read = existing.as_ref().map(|(head, defined)| Read {
        head: *head,
        content: None,
        defined,
    });
    publish(
        context,
        block,
        Written {
            manifest,
            references,
            metadata: &metadata,
        },
        read,
    )
    .await?;
    Ok(Some(block))
}

/// The objects of the contents `tree` names that `known` does not name.
async fn tree_objects(peer: &Peer<Store>, tree: &Tree, known: &Tree) -> Result<Vec<Hash>, String> {
    let known: Vec<&Manifest> = known.manifests().collect();
    let mut objects = Vec::new();
    for manifest in tree.manifests() {
        if !known.contains(&manifest) {
            objects.extend(peer.objects_of(manifest).await.map_err(failed)?);
        }
    }
    Ok(objects)
}
