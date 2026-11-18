//! Runs a hosted block's host component on this device while it hosts the block
//! (plans/hosted_blocks.md, "Hosting"): an editor of the block open here, this account allowed
//! to edit it, and this device's replica holding its session's lease. Whoever holds the lease
//! hosts and the others stand by; when the holder leaves, the next editor's replica takes the
//! lease by the session's order and starts its own. Closing the last editor of the block here
//! stops it, so the lease goes on to another editor and a world left closed uses nothing.

use std::collections::HashMap;
use std::sync::Arc;

use be_host_runtime::EditorBlock;
use be_sdk_api::EditorInstanceId;
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

/// The host components this device runs, by block.
#[derive(Default)]
pub(crate) struct Hosts {
    running: HashMap<Uuid, (Arc<AppManifest>, Uuid, EditorBlock, EditorInstanceId)>,
}

impl Hosts {
    /// Starts the host component of each block this device should host now and stops each one
    /// that should no longer run; called every frame.
    pub(crate) fn step(&mut self, types: &HostedTypes, catalog: &Arc<be_host_runtime::AppCatalog>, client_id: Uuid) {
        let (editing, _) = be_host_runtime::editing_and_hosting();
        let mut wanted: HashMap<Uuid, EditorBlock> = HashMap::new();
        for block in editing {
            if types.of_type(block.block_type).is_some()
                && be_host_stack::access(block.id).can_edit()
                && be_host_stack::holds_lease(block.id)
            {
                wanted.insert(block.id, block);
            }
        }
        self.running.retain(|block, (app, _, _, instance)| {
            let keep = wanted.contains_key(block);
            if !keep {
                be_host_runtime::close(&app.identity.id, *instance);
            }
            keep
        });
        for (id, block) in wanted {
            if self.running.contains_key(&id) {
                continue;
            }
            let Some((app, host)) = types.of_type(block.block_type) else {
                continue;
            };
            let instance = crate::editors::app::next_instance();
            self.running
                .insert(id, (Arc::clone(app), *host, block, instance));
        }
        // Reported every frame, as an editor's creation is, until the app opens it.
        self.running.retain(|_, (app, host, block, instance)| {
            !matches!(
                be_host_runtime::host(app, catalog, client_id, *instance, *host, *block),
                be_host_runtime::CreationState::Failed(_)
            )
        });
    }
}
