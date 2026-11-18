//! Runs a hosted block's host component on this device while it hosts the block
//! (plans/hosted_blocks.md, "Hosting"): an editor of the block open here, this account allowed
//! to edit it, and no other device hosting its session (`be_host_stack::may_host`). Starting the
//! component claims the session's lease (`be_host_stack::restart_host`), which makes this device
//! the host; if another device claims after it, this one stops its own. When the host leaves,
//! the session has no host until a device with an editor open starts its component and claims.
//! Closing the last editor of the block here stops it, so a world left closed uses nothing,
//! unless the person chose Keep hosting in its frame menu (`block-command.keep-hosting`): a kept
//! block's component runs on with no editor open, listed in the host menu with Stop, until
//! Stop, the account may no longer edit it, or be-host quits; meanwhile this device holds its
//! replica open, so a lease lost to a disconnect comes back. Nothing keeps it across a restart.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use be_host_runtime::EditorBlock;
use be_sdk_api::EditorInstanceId;
use be_sdk_api::blocks::HostingHere;
use be_sdk_manifest::AppManifest;
use uuid::Uuid;

/// The host component of each block's type, from the apps' manifests (`hosts`).
#[derive(Default)]
pub(crate) struct HostedTypes(HashMap<Uuid, (Arc<AppManifest>, Uuid)>);

impl HostedTypes {
    pub(crate) fn of(manifests: &[Arc<AppManifest>]) -> Self {
        let mut types = HashMap::new();
        for manifest in manifests {
            for host in &manifest.hosts {
                types
                    .entry(Uuid::from_bytes(host.edits))
                    .or_insert_with(|| (Arc::clone(manifest), Uuid::from_bytes(host.id)));
            }
        }
        Self(types)
    }

    fn of_type(&self, block_type: Uuid) -> Option<&(Arc<AppManifest>, Uuid)> {
        self.0.get(&block_type)
    }
}

/// A host component this device runs.
struct Running {
    app: Arc<AppManifest>,
    host: Uuid,
    block: EditorBlock,
    instance: EditorInstanceId,
    /// Its app opened it: it need not be reported again.
    ready: bool,
}

/// The host components this device runs, by block.
#[derive(Default)]
pub(crate) struct Hosts {
    running: HashMap<Uuid, Running>,
    /// Blocks whose host component failed to start: not started again until the block stops
    /// being one this device should host (its editors close, the lease or the access changes).
    failed: HashSet<Uuid>,
    /// The leases this device held and the instances the runtime ran as of the last look:
    /// looked at again only when either moved, or the graph changed (the account's access).
    seen: Option<(u64, u64)>,
    /// The blocks kept hosting after their editors here close (Keep hosting), in memory only.
    kept: HashMap<Uuid, EditorBlock>,
    /// What the runtime was last told this device hosts (`be_host_runtime::set_hosting`).
    told: HashMap<Uuid, HostingHere>,
}

impl Hosts {
    /// Keeps hosting `block` after its editors here close, or no longer: only a block whose
    /// component runs here may be kept.
    /// While kept, this device holds the block's replica open itself (`be_host_stack::open`),
    /// so a lease lost to a disconnect or a rejoin comes back with the session and the
    /// component starts again; only Stop, or the account no longer allowed to edit, lets it go.
    pub(crate) fn keep(&mut self, block: Uuid, keep: bool) {
        match keep {
            true => {
                let Some(running) = self.running.get(&block) else {
                    return;
                };
                if self.kept.insert(block, running.block).is_none() {
                    be_host_stack::open(block, running.block.block_type);
                }
            }
            false => self.let_go(block),
        }
        self.seen = None;
    }

    fn let_go(&mut self, block: Uuid) {
        if self.kept.remove(&block).is_some() {
            be_host_stack::close(block);
        }
    }

    /// The blocks kept hosting, by name (their type's, for one not named), for the host menu's
    /// Stop.
    pub(crate) fn kept(&self, registry: &crate::editors::EditorRegistry) -> Vec<(Uuid, String)> {
        let mut kept: Vec<(Uuid, String)> = self
            .kept
            .iter()
            .map(|(block, kept)| {
                let type_name = registry.display_name(kept.block_type).unwrap_or("Untitled");
                (*block, crate::name_of(*block, type_name))
            })
            .collect();
        kept.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        kept
    }

    /// Starts the host component of each block this device should host now and stops each one
    /// that should no longer run. Called every frame, it looks again only when a lease, the
    /// instances running or the graph changed, or a component it started is not open yet.
    pub(crate) fn step(
        &mut self,
        types: &HostedTypes,
        catalog: &Arc<be_host_runtime::AppCatalog>,
        client_id: Uuid,
        graph_changed: bool,
    ) {
        let starting = self.running.values().any(|running| !running.ready);
        let revisions = (
            be_host_stack::lease_revision(),
            be_host_runtime::roles_revision(),
        );
        if !graph_changed && !starting && self.seen == Some(revisions) {
            return;
        }
        let (editing, hosting) = be_host_runtime::editing_and_hosting();
        // One its app no longer runs (it crashed or stopped) failed.
        let ended: Vec<Uuid> = self
            .running
            .iter()
            .filter(|(_, running)| {
                running.ready
                    && !hosting
                        .iter()
                        .any(|(_, instance, _)| *instance == running.instance)
            })
            .map(|(block, _)| *block)
            .collect();
        for block in ended {
            self.running.remove(&block);
            self.failed.insert(block);
        }
        let mut wanted: HashMap<Uuid, EditorBlock> = HashMap::new();
        // A kept block counts as edited here, so its component runs on with no editor open.
        let kept: Vec<EditorBlock> = self.kept.values().copied().collect();
        for block in editing.into_iter().chain(kept) {
            if types.of_type(block.block_type).is_some()
                && be_host_stack::access(block.id).can_edit()
                && be_host_stack::may_host(block.id)
            {
                wanted.insert(block.id, block);
            }
        }
        // One the account may no longer edit is kept no longer. One whose lease went (a
        // disconnect, a rejoin, another device's turn) stays kept: its replica stays open, and
        // the component starts again once the lease comes back.
        let unkept: Vec<Uuid> = self
            .kept
            .keys()
            .filter(|block| !be_host_stack::access(**block).can_edit())
            .copied()
            .collect();
        for block in unkept {
            self.let_go(block);
        }
        self.failed.retain(|block| wanted.contains_key(block));
        self.running.retain(|block, running| {
            let keep = wanted.contains_key(block);
            if !keep {
                be_host_runtime::close(&running.app.identity.id, running.instance);
                be_host_stack::release_host(*block);
            }
            keep
        });
        for (id, block) in wanted {
            if self.running.contains_key(&id) || self.failed.contains(&id) {
                continue;
            }
            let Some((app, host)) = types.of_type(block.block_type) else {
                continue;
            };
            // It hears the members already admitted join, and nothing older.
            be_host_stack::restart_host(id);
            self.running.insert(
                id,
                Running {
                    app: Arc::clone(app),
                    host: *host,
                    block,
                    instance: crate::editors::app::next_instance(),
                    ready: false,
                },
            );
        }
        // Reported, as an editor's creation is, until its app opens it.
        let mut failed = Vec::new();
        for (id, running) in &mut self.running {
            if running.ready {
                continue;
            }
            match be_host_runtime::host(
                &running.app,
                catalog,
                client_id,
                running.instance,
                running.host,
                running.block,
            ) {
                be_host_runtime::CreationState::Ready => running.ready = true,
                be_host_runtime::CreationState::Failed(_) => failed.push(*id),
                _ => {}
            }
        }
        for id in failed {
            if let Some(running) = self.running.remove(&id) {
                be_host_runtime::close(&running.app.identity.id, running.instance);
            }
            self.failed.insert(id);
        }
        self.tell_runtime();
        self.seen = Some((
            be_host_stack::lease_revision(),
            be_host_runtime::roles_revision(),
        ));
    }

    /// Tells the runtime which blocks this device hosts and keeps, so their editors show it.
    fn tell_runtime(&mut self) {
        let now: HashMap<Uuid, HostingHere> = self
            .running
            .keys()
            .map(|block| {
                let hosting = match self.kept.contains_key(block) {
                    true => HostingHere::Kept,
                    false => HostingHere::Yes,
                };
                (*block, hosting)
            })
            .collect();
        for (block, _) in self.told.iter().filter(|(block, _)| !now.contains_key(block)) {
            be_host_runtime::set_hosting(*block, HostingHere::No);
        }
        for (block, hosting) in &now {
            if self.told.get(block) != Some(hosting) {
                be_host_runtime::set_hosting(*block, *hosting);
            }
        }
        self.told = now;
    }
}
