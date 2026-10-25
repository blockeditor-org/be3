use std::sync::{Arc, PoisonError};

use wasi_threads::Mutex;

use be_client::Peer;
use be_commit::CommitId;
use block_plugin_api::{HostReply, HostRequest, Revision};
use block_type_api::Change;
use uuid::Uuid;

use super::{
    Content, Shared,
    replicas::Replicas,
    worker::{Command, Store},
};

#[derive(Clone)]
pub(crate) struct Preview {
    pub(crate) source: Uuid,
    pub(crate) root: Uuid,
    pub(crate) content: Option<Content>,
    pub(crate) released: bool,
    pub(crate) requested: bool,
}

pub(crate) fn source(block: Uuid) -> Option<Uuid> {
    super::with_shared(|shared| {
        shared
            .revision_previews
            .get(&block)
            .map(|preview| preview.source)
    })
    .flatten()
}

pub(crate) fn error(block: Uuid) -> Option<String> {
    super::with_shared(|shared| {
        shared
            .revision_previews
            .contains_key(&block)
            .then(|| shared.unreadable.get(&block).cloned())
            .flatten()
    })
    .flatten()
}

pub(crate) fn released(block: Uuid) -> bool {
    super::with_shared(|shared| {
        shared
            .revision_previews
            .get(&block)
            .is_some_and(|preview| preview.released)
    })
    .unwrap_or(false)
}

pub(crate) fn load(alias: Uuid, source: Uuid, content_type: Uuid) {
    let wanted = super::with_shared_mut(|shared| {
        let preview = shared.revision_previews.get_mut(&alias)?;
        if preview.released || preview.content.is_some() || preview.requested {
            return None;
        }
        preview.requested = true;
        Some(())
    })
    .flatten()
    .is_some();
    if wanted {
        super::send(Command::PreviewChild {
            alias,
            source,
            content_type,
        });
    }
}

pub(super) async fn load_child(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    alias: Uuid,
    source: Uuid,
    content_type: Uuid,
) {
    let result = async {
        let summary = peer
            .summary(source)
            .await
            .map_err(|error| error.to_string())?;
        if !summary.access.can_view() {
            return Err("You do not have permission to view this file.".to_owned());
        }
        let head = summary.head.ok_or("This file has no saved content.")?;
        let commit = peer
            .load_commit(head)
            .await
            .map_err(|error| error.to_string())?;
        let bytes = peer
            .read_manifest(&commit.manifest)
            .await
            .map_err(|error| error.to_string())?;
        super::is_known(content_type)
            .then(|| Content::new(content_type, bytes))
            .ok_or("This file cannot be previewed.".to_owned())
    }
    .await;
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(preview) = held.revision_previews.get_mut(&alias)
        && !preview.released
    {
        match result {
            Ok(content) => preview.content = Some(content),
            Err(error) => {
                held.unreadable.insert(alias, error);
            }
        }
        held.touch(alias);
    }
}

pub(crate) fn alias(parent: Uuid, block: Uuid) -> Uuid {
    super::with_shared_mut(|shared| {
        let Some(root) = shared
            .revision_previews
            .get(&parent)
            .map(|preview| preview.root)
        else {
            return block;
        };
        if let Some(id) = shared.preview_aliases.get(&(root, block)) {
            return *id;
        }
        let id = Uuid::new_v4();
        shared.preview_aliases.insert((root, block), id);
        shared.revision_previews.insert(
            id,
            Preview {
                source: block,
                root,
                content: shared.blocks.get(&block).cloned(),
                released: false,
                requested: false,
            },
        );
        id
    })
    .unwrap_or(block)
}

pub(crate) fn release(root: Uuid) {
    super::with_shared_mut(|shared| {
        for (id, preview) in shared
            .revision_previews
            .iter_mut()
            .filter(|(_, preview)| preview.root == root && !preview.released)
        {
            preview.released = true;
            shared.released_previews.push(*id);
        }
    });
    crate::host::wake();
}

pub(crate) fn collect_released() {
    let ids = super::with_shared_mut(|shared| std::mem::take(&mut shared.released_previews))
        .unwrap_or_default();
    if ids.is_empty() {
        return;
    }
    crate::compositor::release_revision_editors(&ids);
    super::with_shared_mut(|shared| {
        for id in ids {
            if let Some(preview) = shared.revision_previews.get_mut(&id) {
                preview.content = None;
            }
            shared.unreadable.remove(&id);
        }
    });
}

pub(crate) fn request(
    block: Uuid,
    request: HostRequest,
    reply: impl FnOnce(HostReply) + Send + 'static,
) {
    super::send(Command::Revision {
        block,
        request,
        reply: Box::new(reply),
    });
}

pub(super) async fn run(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    request: HostRequest,
) -> HostReply {
    let outcome = perform(peer, replicas, shared, block, &request).await;
    match request {
        HostRequest::ListRevisions(_) => HostReply::RevisionsListed(outcome.map(|result| result.0)),
        HostRequest::PreviewRevision { .. } => {
            HostReply::RevisionPreviewed(outcome.map(|result| result.1.into_bytes()))
        }
        _ => HostReply::RevisionRestored(outcome.map(|_| ())),
    }
}

async fn perform(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    request: &HostRequest,
) -> Result<(Vec<Revision>, Uuid), String> {
    let summary = peer
        .summary(block)
        .await
        .map_err(|error| error.to_string())?;
    if !summary.access.can_view() {
        return Err("You do not have permission to view this file.".into());
    }
    let commit = match request {
        HostRequest::PreviewRevision { commit, .. }
        | HostRequest::RestoreRevision { commit, .. } => {
            Some(CommitId::from_hash(be_store::Hash::from_bytes(*commit)))
        }
        _ => None,
    };
    let conflicted = commit.is_some_and(|commit| {
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        super::conflicts::is_side(&held, block, commit)
    });
    let entries = match conflicted {
        true => Vec::new(),
        false => peer
            .history(block)
            .await
            .map_err(|error| error.to_string())?,
    };
    let Some(commit) = commit else {
        let mut revisions: Vec<_> = entries
            .into_iter()
            .map(|entry| Revision {
                commit: *entry.commit.hash().as_bytes(),
                time: entry.time,
                current: Some(entry.commit) == summary.head,
            })
            .collect();
        revisions.sort_by(|a, b| {
            b.time
                .cmp(&a.time)
                .then_with(|| b.current.cmp(&a.current))
                .then_with(|| b.commit.cmp(&a.commit))
        });
        return Ok((revisions, Uuid::nil()));
    };
    if !conflicted && !entries.iter().any(|entry| entry.commit == commit) {
        return Err("This revision is no longer available.".into());
    }
    let saved = peer
        .load_commit(commit)
        .await
        .map_err(|error| error.to_string())?;
    if saved.manifest.content_type != summary.content_type {
        return Err("The revision has a different file type.".into());
    }
    let bytes = peer
        .read_manifest(&saved.manifest)
        .await
        .map_err(|error| error.to_string())?;
    if matches!(request, HostRequest::PreviewRevision { .. }) {
        let content = super::is_known(summary.content_type)
            .then(|| Content::new(summary.content_type, bytes))
            .ok_or("This revision cannot be previewed.")?;
        let id = Uuid::new_v4();
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.preview_aliases.insert((id, block), id);
        held.revision_previews.insert(
            id,
            Preview {
                source: block,
                root: id,
                content: Some(content),
                released: false,
                requested: false,
            },
        );
        return Ok((Vec::new(), id));
    }
    if !summary.access.can_edit() {
        return Err("You do not have permission to restore this file.".into());
    }
    if !replicas.is_open(block) {
        return Err("Open this file before restoring a revision.".into());
    }
    replicas.seal((peer, shared), block, false).await;
    if !replicas.status(block).is_some_and(|status| status.clean) {
        return Err(
            "The current edits are still being saved. Try restoring again once they are saved."
                .into(),
        );
    }
    replicas
        .change((peer, shared), block, Change::Replace(bytes))
        .await;
    Ok((Vec::new(), Uuid::nil()))
}
