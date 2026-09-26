use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block::{
    BlockContent, BlockMetadata, Checkout, CheckoutConflict, ConflictKind, MAIN_BRANCH, Repository,
    Root,
    be_model::{Document, Model},
    version_control::{masked, scope_mask},
};
use be_client::{ClientError, Peer, Saved};
use be_commit::{Commit, CommitId, now_milliseconds};
use be_graph::BlockParent;
use be_protocol::ErrorCode;
use be_store::{Hash, Manifest};
use be_vcs::{Entry, Tree};
use block_plugin_api::{
    BlockIdRole, ConflictSide, VersionBranch, VersionChange, VersionChangeKind, VersionCommand,
    VersionCommit, VersionStatus,
};
use uuid::Uuid;

use super::graph::{Graph, Scope};
use super::worker::{Session, Shared, Store, node_of};

const LOG_LENGTH: usize = 100;

type Sessions = HashMap<Uuid, Box<dyn Session>>;

#[derive(Default)]
pub(crate) struct VersionState {
    pub(crate) revision: u64,
    pub(crate) status: VersionStatus,
}

#[derive(Default)]
pub(super) struct Versions {
    trees: HashMap<CommitId, Tree>,
    commits: HashMap<CommitId, Commit>,
    watching: HashSet<Uuid>,
    computed: HashMap<Uuid, (u64, u64, u64)>,
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

pub(super) struct Context<'a> {
    pub(super) peer: &'a Arc<Peer<Store>>,
    pub(super) shared: &'a Arc<Mutex<Shared>>,
    pub(super) sessions: &'a mut Sessions,
}

fn failed(error: ClientError) -> String {
    error.to_string()
}

fn scope(checkout: Uuid) -> Scope {
    Scope {
        checkout,
        mask: scope_mask(checkout),
    }
}

fn stripped(metadata: &BlockMetadata) -> BlockMetadata {
    BlockMetadata {
        local_id: None,
        ..metadata.clone()
    }
}

fn named(metadata: &BlockMetadata) -> Option<&str> {
    metadata
        .named_by_hand
        .then_some(metadata.name.as_deref())
        .flatten()
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
            VersionCommand::Commit { message } => self.commit(context, block, &message).await,
            VersionCommand::Update => self.update(context, block).await,
            VersionCommand::Switch { branch } => self.switch(context, block, &branch).await,
            VersionCommand::CreateBranch { name } => {
                self.create_branch(context, block, &name).await
            }
            VersionCommand::NewCheckout { branch } => {
                self.new_checkout(context, block, &branch).await.map(|_| ())
            }
            VersionCommand::Resolve { block_id, take } => {
                self.resolve(context, block, Uuid::from_bytes(block_id), take)
                    .await
            }
            VersionCommand::Fork => self.fork(context, block).await.map(|_| ()),
            VersionCommand::PullUpstream => self.pull(context, block).await,
            VersionCommand::PushUpstream => self.push(context, block).await,
        }
    }

    async fn tree(&mut self, peer: &Peer<Store>, commit: Option<CommitId>) -> Result<Tree, String> {
        let Some(commit) = commit else {
            return Ok(Tree::default());
        };
        if let Some(tree) = self.trees.get(&commit) {
            return Ok(tree.clone());
        }
        let manifest = self.commit_of(peer, commit).await?.manifest;
        peer.fetch(&manifest.chunk_hashes()).await.map_err(failed)?;
        let tree =
            Tree::read(peer.commits().vault(), &manifest).map_err(|error| error.to_string())?;
        self.trees.insert(commit, tree.clone());
        Ok(tree)
    }

    async fn commit_of(&mut self, peer: &Peer<Store>, commit: CommitId) -> Result<Commit, String> {
        if let Some(found) = self.commits.get(&commit) {
            return Ok(found.clone());
        }
        let loaded = peer.load_commit(commit).await.map_err(failed)?;
        self.commits.insert(commit, loaded.clone());
        Ok(loaded)
    }

    async fn working(
        &mut self,
        context: &Context<'_>,
        checkout: Uuid,
        scanned: &[Scanned],
        base: &Tree,
        root: Option<Uuid>,
    ) -> Result<Working, String> {
        let mut working = Working::default();
        working.tree.root = root;
        let scope = scope(checkout);
        for block in scanned {
            let (content, references) = match (block.clean, base.entries.get(&block.local)) {
                (true, Some(entry)) => (entry.content.clone(), entry.references.clone()),
                _ => match block.head {
                    Some(head) => {
                        let commit = self.commit_of(context.peer, head).await?;
                        let references = with_graph(context.shared, |graph| {
                            commit
                                .references
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
        Ok(working)
    }

    async fn working_copy(
        &mut self,
        context: &Context<'_>,
        checkout: Uuid,
        state: &Checkout,
    ) -> Result<(Tree, Working), String> {
        let base = self.tree(context.peer, state.base).await?;
        let scanned = with_graph(context.shared, |graph| scan(graph, checkout, state, &base));
        let root = state.root.map(|root| {
            with_graph(context.shared, |graph| {
                graph.to_local(scope(checkout), root)
            })
        });
        let working = self
            .working(context, checkout, &scanned, &base, root)
            .await?;
        Ok((base, working))
    }

    async fn commit(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        message: &str,
    ) -> Result<(), String> {
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = state
            .repository
            .ok_or("This checkout is not attached to a repository.")?;
        let branch_head = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(&state.branch);
        if branch_head != state.base {
            return Err(
                "The branch has moved on since this checkout's base. Bring its changes in first."
                    .into(),
            );
        }
        if !state.conflicts.is_empty() {
            return Err("Resolve the conflicts before committing.".into());
        }
        let (base, working) = self.working_copy(context, checkout, &state).await?;
        if state.base.is_some() && working.tree == base {
            return Err("There is nothing to commit.".into());
        }
        let vault = context.peer.commits().vault();
        let manifest = working
            .tree
            .write(vault)
            .map_err(|error| error.to_string())?;
        let commit = be_vcs::snapshot(
            context.peer.commits(),
            manifest.clone(),
            state.base.into_iter().collect(),
            context.peer.account(),
            now_milliseconds(),
            message,
        )
        .map_err(|error| error.to_string())?;
        let held = be_vcs::held_objects(commit, &manifest, &working.tree, &base);
        context.peer.hold(repository, &held).await.map_err(failed)?;
        let branch = state.branch.clone();
        let expected = state.base;
        change::<Repository>(context, repository, |repository| {
            if repository.branch(&branch) != expected {
                return Err(
                    "The branch moved while committing. Bring its changes in first.".into(),
                );
            }
            *repository = repository.with_branch(&branch, Some(commit));
            Ok(())
        })
        .await?;
        self.trees.insert(commit, working.tree.clone());
        let clean = clean_heads(&working, &working.tree);
        change::<Checkout>(context, checkout, |state| {
            state.base = Some(commit);
            state.clean = clean.clone().into_iter().collect();
            Ok(())
        })
        .await?;
        Ok(())
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
            return Err("That block is not in this workspace.".into());
        };
        if node.content_type == Checkout::CONTENT_TYPE
            || node.content_type == Repository::CONTENT_TYPE
        {
            return Err("A repository or a checkout cannot be versioned itself.".into());
        }
        if with_graph(context.shared, |graph| graph.scope_of(root)).is_some() {
            return Err("That block is already inside a checkout.".into());
        }
        let checkout = Uuid::new_v4();
        let name = node
            .metadata
            .name
            .clone()
            .map_or_else(|| "Checkout".to_owned(), |name| format!("{name} checkout"));
        create(
            context,
            checkout,
            Checkout::CONTENT_TYPE,
            node.parent,
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
        self.commit(context, checkout, "Started versioning").await
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
            graph.get(repository).map(|node| node.parent)
        })
        .unwrap_or(BlockParent::Root);
        let checkout = Uuid::new_v4();
        create(
            context,
            checkout,
            Checkout::CONTENT_TYPE,
            parent,
            BlockMetadata::named(format!("{branch} checkout")),
        )
        .await?;
        let (heads, real) = materialize(context, checkout, &tree, &Working::default()).await?;
        let state = Checkout {
            repository: Some(repository),
            branch: branch.to_owned(),
            base: Some(head),
            root: tree.root.and_then(|root| real.get(&root).copied()),
            clean: heads.into_iter().collect(),
            ..Checkout::default()
        };
        save_new(context, checkout, &state).await?;
        Ok(checkout)
    }

    async fn switch(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        branch: &str,
    ) -> Result<(), String> {
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = state
            .repository
            .ok_or("This checkout is not attached to a repository.")?;
        let head = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(branch)
            .ok_or_else(|| format!("There is no branch named {branch}."))?;
        let (base, working) = self.working_copy(context, checkout, &state).await?;
        if working.tree != base {
            return Err("Commit your changes before switching branches.".into());
        }
        let target = self.tree(context.peer, Some(head)).await?;
        let (heads, real) = materialize(context, checkout, &target, &working).await?;
        let branch = branch.to_owned();
        change::<Checkout>(context, checkout, |state| {
            state.branch = branch.clone();
            state.base = Some(head);
            state.root = target.root.and_then(|root| real.get(&root).copied());
            state.clean = heads.clone().into_iter().collect();
            Ok(())
        })
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
        let repository = state
            .repository
            .ok_or("This checkout is not attached to a repository.")?;
        let base = state.base.ok_or("Commit before making a branch.")?;
        change::<Repository>(context, repository, |repository| {
            if repository.branch(&name).is_some() {
                return Err(format!("There is already a branch named {name}."));
            }
            *repository = repository.with_branch(&name, Some(base));
            Ok(())
        })
        .await?;
        change::<Checkout>(context, checkout, |state| {
            state.branch = name.clone();
            Ok(())
        })
        .await
    }

    async fn update(&mut self, context: &mut Context<'_>, checkout: Uuid) -> Result<(), String> {
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let repository = state
            .repository
            .ok_or("This checkout is not attached to a repository.")?;
        let theirs = read::<Repository>(context.peer, repository)
            .await?
            .0
            .branch(&state.branch)
            .ok_or("The branch this checkout follows is gone.")?;
        if state.base == Some(theirs) {
            return Ok(());
        }
        if !state.conflicts.is_empty() {
            return Err("Resolve the conflicts before bringing more changes in.".into());
        }
        let (base, working) = self.working_copy(context, checkout, &state).await?;
        let theirs_tree = self.tree(context.peer, Some(theirs)).await?;
        let mut plan = be_vcs::merge(&base, &working.tree, &theirs_tree);
        let mut versions = Vec::new();
        for merge in std::mem::take(&mut plan.contents) {
            let (manifest, references, conflicted) = merge_content(context.peer, &merge).await?;
            if let Some(entry) = plan.tree.entries.get_mut(&merge.block) {
                entry.content = manifest;
                entry.references = references;
            }
            if conflicted {
                plan.conflicts.push(be_vcs::Conflict {
                    block: merge.block,
                    kind: ConflictKind::Content,
                });
                versions.push(merge);
            }
        }
        let (heads, real) = materialize(context, checkout, &plan.tree, &working).await?;
        let mut conflicts = Vec::new();
        for conflict in &plan.conflicts {
            let Some(block) = real.get(&conflict.block).copied() else {
                continue;
            };
            let mut recorded = CheckoutConflict {
                block,
                content_type: plan
                    .tree
                    .entries
                    .get(&conflict.block)
                    .map(|entry| entry.content_type)
                    .unwrap_or_default(),
                kind: conflict.kind,
                ..CheckoutConflict::default()
            };
            if let Some(merge) = versions.iter().find(|merge| merge.block == conflict.block) {
                let name = plan
                    .tree
                    .entries
                    .get(&conflict.block)
                    .and_then(|entry| entry.metadata.name.clone())
                    .unwrap_or_else(|| "Block".to_owned());
                let sides = [
                    (&merge.base, &base, "base"),
                    (&merge.ours, &working.tree, "yours"),
                    (&merge.theirs, &theirs_tree, "theirs"),
                ];
                let mut made = Vec::new();
                for (manifest, tree, label) in sides {
                    let references = tree
                        .entries
                        .get(&conflict.block)
                        .map(|entry| entry.references.clone())
                        .unwrap_or_default();
                    made.push(
                        version_block(
                            context,
                            checkout,
                            merge.content_type,
                            manifest.clone(),
                            references,
                            &real,
                            format!("{name} ({label})"),
                        )
                        .await?,
                    );
                }
                recorded.base = made[0];
                recorded.ours = made[1];
                recorded.theirs = made[2];
            }
            conflicts.push(recorded);
        }
        self.trees.insert(theirs, theirs_tree.clone());
        let local_of: HashMap<Uuid, Uuid> =
            real.iter().map(|(local, block)| (*block, *local)).collect();
        let clean: BTreeMap<Uuid, CommitId> = heads
            .into_iter()
            .filter(|(block, _)| {
                local_of.get(block).is_some_and(|local| {
                    plan.tree.entries.get(local).map(|entry| &entry.content)
                        == theirs_tree.entries.get(local).map(|entry| &entry.content)
                })
            })
            .collect();
        let root = plan.tree.root.and_then(|root| real.get(&root).copied());
        change::<Checkout>(context, checkout, |state| {
            state.base = Some(theirs);
            state.clean = clean.clone().into_iter().collect();
            state.root = root.or(state.root);
            state.conflicts = conflicts.iter().cloned().collect();
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
        let state = read::<Checkout>(context.peer, checkout).await?.0;
        let Some(conflict) = state
            .conflicts
            .iter()
            .find(|conflict| conflict.block == block)
            .map(|conflict| (**conflict).clone())
        else {
            return Err("That conflict is already resolved.".into());
        };
        let side = match take {
            ConflictSide::Base => conflict.base,
            ConflictSide::Ours => conflict.ours,
            ConflictSide::Theirs => conflict.theirs,
            ConflictSide::Merged => None,
        };
        if let Some(side) = side
            && let Some(head) = context.peer.remote_head(side).await.map_err(failed)?
        {
            let commit = self.commit_of(context.peer, head).await?;
            publish(context, block, Some(commit.manifest), commit.references).await?;
        }
        for version in conflict.versions() {
            set_parent(context, version, BlockParent::Detached).await?;
        }
        change::<Checkout>(context, checkout, |state| {
            state.conflicts = state
                .conflicts
                .iter()
                .filter(|held| held.block != block)
                .map(|held| (**held).clone())
                .collect();
            Ok(())
        })
        .await
    }

    async fn fork(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<Uuid, String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let node = with_graph(context.shared, |graph| graph.get(repository).cloned())
            .ok_or("That repository is not in this workspace.")?;
        let heads: Vec<CommitId> = held.branches.values().copied().collect();
        let objects = self
            .reachable(context.peer, &heads, &HashSet::new())
            .await?;
        let fork = Uuid::new_v4();
        let name = node
            .metadata
            .name
            .clone()
            .map_or_else(|| "Fork".to_owned(), |name| format!("{name} fork"));
        create(
            context,
            fork,
            Repository::CONTENT_TYPE,
            node.parent,
            BlockMetadata::named(name),
        )
        .await?;
        context.peer.hold(fork, &objects).await.map_err(failed)?;
        let state = Repository {
            branches: held
                .branches
                .iter()
                .map(|(name, head)| (name.clone(), *head))
                .collect(),
            upstream: Some(repository),
        };
        save_new(context, fork, &state).await?;
        Ok(fork)
    }

    async fn pull(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<(), String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let upstream = held.upstream.ok_or("This repository has no upstream.")?;
        let theirs = read::<Repository>(context.peer, upstream).await?.0;
        self.carry(context, &theirs, repository).await
    }

    async fn push(&mut self, context: &mut Context<'_>, repository: Uuid) -> Result<(), String> {
        let held = read::<Repository>(context.peer, repository).await?.0;
        let upstream = held.upstream.ok_or("This repository has no upstream.")?;
        self.carry(context, &held, upstream).await
    }

    async fn carry(
        &mut self,
        context: &mut Context<'_>,
        from: &Repository,
        into: Uuid,
    ) -> Result<(), String> {
        let current = read::<Repository>(context.peer, into).await?.0;
        let mut known = HashSet::new();
        for head in current.branches.values() {
            context.peer.fetch_history(*head).await.map_err(failed)?;
            known.extend(
                context
                    .peer
                    .commits()
                    .ancestry(*head)
                    .map_err(|error| error.to_string())?,
            );
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
                        .map_err(|error| error.to_string())? =>
                {
                    diverged.push(name.clone());
                }
                existing => moved.push((name.clone(), existing, *head)),
            }
        }
        let heads: Vec<CommitId> = moved.iter().map(|(_, _, head)| *head).collect();
        let objects = self.reachable(context.peer, &heads, &known).await?;
        context.peer.hold(into, &objects).await.map_err(failed)?;
        change::<Repository>(context, into, |repository| {
            for (name, expected, head) in &moved {
                if repository.branch(name) == *expected {
                    *repository = repository.with_branch(name, Some(*head));
                }
            }
            Ok(())
        })
        .await?;
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
            objects.extend(loaded.manifest.chunk_hashes());
            objects.extend(self.tree(peer, Some(commit)).await?.objects());
            frontier.extend(loaded.parents);
        }
        objects.sort_unstable();
        objects.dedup();
        Ok(objects)
    }

    pub(super) fn watch(shared: &Arc<Mutex<Shared>>, block: Uuid) {
        shared.lock().unwrap().version_watch.insert(block);
    }

    pub(super) async fn refresh(&mut self, context: &mut Context<'_>) {
        let watched: Vec<Uuid> = context
            .shared
            .lock()
            .unwrap()
            .version_watch
            .iter()
            .copied()
            .collect();
        for block in watched {
            let signature = {
                let held = context.shared.lock().unwrap();
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
                )
            };
            if self.computed.get(&block) == Some(&signature) {
                continue;
            }
            self.computed.insert(block, signature);
            let status = self.status(context, block).await;
            let mut held = context.shared.lock().unwrap();
            let entry = held.versions.entry(block).or_default();
            let status = VersionStatus {
                busy: entry.status.busy,
                error: entry.status.error.clone(),
                ..status
            };
            if entry.status != status {
                entry.status = status;
                entry.revision += 1;
            }
        }
    }

    async fn status(&mut self, context: &mut Context<'_>, block: Uuid) -> VersionStatus {
        let (checkout, repository) = {
            let held = context.shared.lock().unwrap();
            (
                checkout_of(&held, block),
                content_of::<Repository>(&held, block),
            )
        };
        if let Some(repository) = repository {
            return VersionStatus {
                branches: branches(&repository),
                log: self
                    .log(
                        context.peer,
                        repository.branches.values().copied().collect(),
                    )
                    .await,
                ..VersionStatus::default()
            };
        }
        let Some(state) = checkout else {
            return VersionStatus::default();
        };
        let Some(repository_id) = state.repository else {
            return VersionStatus::default();
        };
        self.follow(context, block, &state, repository_id).await;
        let repository = {
            let held = context.shared.lock().unwrap();
            content_of::<Repository>(&held, repository_id)
        };
        let Ok(base) = self.tree(context.peer, state.base).await else {
            return VersionStatus::default();
        };
        let changes = with_graph(context.shared, |graph| {
            changes(graph, &scan(graph, block, &state, &base), &base, block)
        });
        let head = repository
            .as_ref()
            .and_then(|repository| repository.branch(&state.branch));
        VersionStatus {
            behind: repository.is_some() && head != state.base,
            branches: repository.as_ref().map(branches).unwrap_or_default(),
            changes,
            log: self.log(context.peer, head.into_iter().collect()).await,
            ..VersionStatus::default()
        }
    }

    async fn follow(
        &mut self,
        context: &mut Context<'_>,
        checkout: Uuid,
        state: &Checkout,
        repository: Uuid,
    ) {
        if !context.sessions.contains_key(&repository)
            && let Some(join) = super::join_for(Repository::CONTENT_TYPE)
            && let Ok(session) = join(context.peer, repository).await
        {
            context.sessions.insert(repository, session);
        }
        let mut blocks = vec![checkout, repository];
        if let Some(root) = state.root {
            blocks.extend(with_graph(context.shared, |graph| graph.subtree(root)));
        }
        for block in blocks {
            if self.watching.insert(block) && context.peer.watch(block).await.is_err() {
                self.watching.remove(&block);
            }
        }
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
            found.push(VersionCommit {
                id: *commit.hash().as_bytes(),
                parents: loaded
                    .parents
                    .iter()
                    .map(|parent| *parent.hash().as_bytes())
                    .collect(),
                author: loaded.author.into_bytes(),
                time: loaded.time,
                message: be_vcs::message_of(&loaded).unwrap_or_default().to_owned(),
            });
        }
        found.sort_by_key(|commit| std::cmp::Reverse(commit.time));
        found
    }
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
    Document::<R>::decode(&content.bytes)
        .ok()
        .map(|document| document.root())
}

fn with_graph<T>(shared: &Arc<Mutex<Shared>>, read: impl FnOnce(&Graph) -> T) -> T {
    read(&shared.lock().unwrap().graph)
}

fn scan(graph: &Graph, checkout: Uuid, state: &Checkout, base: &Tree) -> Vec<Scanned> {
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
                    .block()
                    .map(|parent| graph.to_local(scope, parent)),
            };
            let clean = match node.head {
                Some(head) => state.clean.get(&real) == Some(&head),
                None => base
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

fn changes(graph: &Graph, scanned: &[Scanned], base: &Tree, checkout: Uuid) -> Vec<VersionChange> {
    let mut changes = Vec::new();
    let present: HashSet<Uuid> = scanned.iter().map(|block| block.local).collect();
    for block in scanned {
        let kind = match base.entries.get(&block.local) {
            None => Some(VersionChangeKind::Added),
            Some(_) if !block.clean => Some(VersionChangeKind::Modified),
            Some(entry)
                if entry.parent != block.parent
                    || named(&entry.metadata) != named(&block.metadata) =>
            {
                Some(VersionChangeKind::Moved)
            }
            Some(_) => None,
        };
        if let Some(kind) = kind {
            changes.push(VersionChange {
                block_id: block.real.into_bytes(),
                block_type: block.content_type.into_bytes(),
                name: block.metadata.name.clone(),
                kind,
            });
        }
    }
    let scope = scope(checkout);
    for (local, entry) in &base.entries {
        if !present.contains(local) {
            changes.push(VersionChange {
                block_id: graph
                    .to_real(scope, *local, BlockIdRole::Existing)
                    .into_bytes(),
                block_type: entry.content_type.into_bytes(),
                name: entry.metadata.name.clone(),
                kind: VersionChangeKind::Removed,
            });
        }
    }
    changes
}

fn clean_heads(working: &Working, tree: &Tree) -> BTreeMap<Uuid, CommitId> {
    working
        .real
        .iter()
        .filter(|(local, _)| tree.entries.contains_key(*local))
        .filter_map(|(_, real)| Some((*real, (*working.heads.get(real)?)?)))
        .collect()
}

async fn refresh_heads(context: &mut Context<'_>) -> Result<(), String> {
    let blocks = context.peer.list_blocks().await.map_err(failed)?;
    let mut held = context.shared.lock().unwrap();
    for block in blocks {
        held.graph.set_head(block.id, block.head);
    }
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
    if let Some(session) = context.sessions.get_mut(&block) {
        session.published_elsewhere(head).await.map_err(failed)?;
    }
    context
        .shared
        .lock()
        .unwrap()
        .graph
        .set_head(block, Some(head));
    Ok(())
}

async fn create(
    context: &mut Context<'_>,
    block: Uuid,
    content_type: Uuid,
    parent: BlockParent,
    metadata: BlockMetadata,
) -> Result<(), String> {
    let summary = match context
        .peer
        .create_block(block, content_type, parent, &metadata)
        .await
    {
        Ok(summary) => summary,
        Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
            context
                .peer
                .set_parent(block, parent)
                .await
                .map_err(failed)?;
            context
                .peer
                .set_metadata(block, &metadata)
                .await
                .map_err(failed)?
        }
        Err(error) => return Err(failed(error)),
    };
    let node = node_of(context.peer, &summary);
    context.shared.lock().unwrap().graph.put(node);
    Ok(())
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
    context.shared.lock().unwrap().graph.put(node);
    Ok(())
}

async fn publish(
    context: &mut Context<'_>,
    block: Uuid,
    manifest: Option<Manifest>,
    references: Vec<Uuid>,
) -> Result<Option<CommitId>, String> {
    let mut expected = context.peer.remote_head(block).await.map_err(failed)?;
    let Some(manifest) = manifest else {
        return Ok(expected);
    };
    for _ in 0..4 {
        match context
            .peer
            .publish_manifest(
                block,
                manifest.clone(),
                references.clone(),
                expected,
                now_milliseconds(),
                false,
                Vec::new(),
            )
            .await
            .map_err(failed)?
        {
            Saved::Published(head) | Saved::Unchanged(head) => {
                settle(context, block, head).await?;
                if !references.is_empty() {
                    let summary = context.peer.summary(block).await.map_err(failed)?;
                    let node = node_of(context.peer, &summary);
                    context.shared.lock().unwrap().graph.put(node);
                }
                return Ok(Some(head));
            }
            Saved::Rejected { head } => expected = head,
        }
    }
    Err("Someone else kept changing a block at the same time. Try again.".into())
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
                if named(&current.metadata) != named(&entry.metadata) {
                    let metadata = BlockMetadata {
                        local_id: (block != local).then_some(local),
                        ..entry.metadata.clone()
                    };
                    context
                        .peer
                        .set_metadata(block, &metadata)
                        .await
                        .map_err(failed)?;
                }
                match current.content == entry.content {
                    true => working.heads.get(&block).copied().flatten(),
                    false => publish(context, block, entry.content.clone(), references).await?,
                }
            }
            None => {
                let metadata = BlockMetadata {
                    local_id: (block != local).then_some(local),
                    ..entry.metadata.clone()
                };
                create(context, block, entry.content_type, parent, metadata).await?;
                publish(context, block, entry.content.clone(), references).await?
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
    peer: &Peer<Store>,
    merge: &be_vcs::ContentMerge,
) -> Result<(Option<Manifest>, Vec<Uuid>, bool), String> {
    let mut sides = Vec::new();
    for manifest in [&merge.base, &merge.ours, &merge.theirs] {
        sides.push(match manifest {
            Some(manifest) => {
                peer.fetch(&manifest.chunk_hashes()).await.map_err(failed)?;
                Some(
                    peer.commits()
                        .vault()
                        .read(manifest)
                        .map_err(|error| error.to_string())?,
                )
            }
            None => None,
        });
    }
    let merged = super::merge_for(merge.content_type).and_then(|merge| {
        merge(
            sides[0].as_deref(),
            sides[1].as_deref(),
            sides[2].as_deref(),
        )
    });
    let Some((bytes, conflicted)) = merged else {
        return Ok((merge.ours.clone(), Vec::new(), true));
    };
    let manifest = peer
        .commits()
        .vault()
        .write(merge.content_type, &bytes)
        .map_err(|error| error.to_string())?;
    let references = super::references_for(merge.content_type)
        .map(|references| references(&bytes, peer.workspace()))
        .unwrap_or_default();
    Ok((Some(manifest), references, conflicted))
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
    let block = Uuid::new_v4();
    create(
        context,
        block,
        content_type,
        BlockParent::Block(checkout),
        BlockMetadata::named(name),
    )
    .await?;
    let references = references
        .iter()
        .map(|reference| real.get(reference).copied().unwrap_or(*reference))
        .collect();
    publish(context, block, manifest, references).await?;
    Ok(Some(block))
}
