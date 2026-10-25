use be_block::{BlockContent, PREFIXES, PrefixRegistry, PrefixRegistryContent};
use be_client::Saved;
use be_graph::{Access, BlockParent};
use be_block::be_model::BlockRef;
use be_store::ObjectStore;
use database_types::database::DatabaseContent;
use uuid::Uuid;

use super::{ATTEMPTS, Tracker};
use crate::session::explain;

pub struct Registry {
    pub owner: Uuid,
    pub kind: Option<Uuid>,
}

impl<S: ObjectStore> Tracker<S> {
    async fn registry_of(&self, block: Uuid) -> Result<Option<Uuid>, String> {
        let summary = self.peer.summary(block).await.map_err(explain)?;
        if !summary.access.can_view() {
            return Ok(None);
        }
        let metadata = self.peer.metadata(&summary).map_err(explain)?;
        Ok(metadata.get(&PREFIXES).map(|kind| kind.0))
    }

    pub async fn registry_for(&self, database: Uuid) -> Result<Option<Registry>, String> {
        let mut current = self.peer.summary(database).await.map_err(explain)?.parent;
        let mut workspace = None;
        let mut outermost = None;
        while let Some(BlockParent::Block(block)) = current {
            let Ok(summary) = self.peer.summary(block).await else {
                break;
            };
            if !summary.access.can_view() {
                break;
            }
            let metadata = self.peer.metadata(&summary).map_err(explain)?;
            if let Some(kind) = metadata.get(&PREFIXES) {
                return Ok(Some(Registry {
                    owner: block,
                    kind: Some(kind.0),
                }));
            }
            if summary.content_type == be_block::WORKSPACE_TYPE && workspace.is_none() {
                workspace = Some(block);
            }
            if summary.content_type != database_types::database_view::DatabaseViewContent::CONTENT_TYPE {
                outermost = Some(block);
            }
            current = summary.parent;
        }
        Ok(workspace.or(outermost).map(|owner| Registry { owner, kind: None }))
    }

    async fn change_registry(&self, kind: Uuid, edit: &be_block::Edit) -> Result<(), String> {
        for _ in 0..ATTEMPTS {
            let head = self.peer.remote_head(kind).await.map_err(explain)?;
            let mut content = match head {
                Some(_) => self
                    .peer
                    .open::<PrefixRegistryContent>(kind)
                    .await
                    .map_err(explain)?
                    .unwrap_or_default(),
                None => PrefixRegistryContent::default(),
            };
            let before = content.root();
            content.apply(edit);
            if content.root() == before {
                return Ok(());
            }
            match self.peer.save(kind, &content, head).await.map_err(explain)? {
                Saved::Rejected { .. } => continue,
                Saved::Published(_) | Saved::Unchanged(_) => return Ok(()),
            }
        }
        Err("the database prefixes kept changing while this was being saved; try again".into())
    }

    pub async fn register(&self, database: Uuid, prefix: &str) -> Result<(), String> {
        let Some(registry) = self.registry_for(database).await? else {
            return Ok(());
        };
        let edit = PrefixRegistry::register(database, Some(prefix));
        let kind = match registry.kind {
            Some(kind) => kind,
            None => {
                let kind = Uuid::now_v7();
                self.peer
                    .create_block(
                        kind,
                        PrefixRegistryContent::CONTENT_TYPE,
                        BlockParent::Block(registry.owner),
                        &be_block::BlockMetadata::default(),
                    )
                    .await
                    .map_err(explain)?;
                self.peer
                    .open_inside(kind, Access::View)
                    .await
                    .map_err(explain)?;
                self.peer
                    .define(registry.owner, |metadata| {
                        metadata.insert(&PREFIXES, &BlockRef(kind));
                    })
                    .await
                    .map_err(explain)?;
                kind
            }
        };
        self.change_registry(kind, &edit).await?;
        self.invalidate().await;
        Ok(())
    }

    pub async fn register_uploaded(&self, blocks: &[be_client::transfer::Saved]) -> Result<(), String> {
        for block in blocks {
            if block.block_type != DatabaseContent::CONTENT_TYPE {
                continue;
            }
            let prefix = block
                .content
                .as_deref()
                .and_then(|bytes| DatabaseContent::decode(bytes).ok())
                .map(|content| content.root().prefix)
                .unwrap_or_default();
            if !prefix.trim().is_empty() {
                self.register(block.id, &prefix).await?;
            }
        }
        Ok(())
    }

    async fn registries(&self) -> Result<Vec<Uuid>, String> {
        let mut found = Vec::new();
        for entry in self.top().await? {
            let kinds = match entry.parent {
                BlockParent::Root => self.registry_of(entry.id).await?.into_iter().collect(),
                BlockParent::Block(_) | BlockParent::Detached => self
                    .peer
                    .open_settings(entry.id)
                    .await
                    .map_err(explain)?
                    .into_iter()
                    .filter(|summary| summary.content_type == PrefixRegistryContent::CONTENT_TYPE)
                    .map(|summary| summary.id)
                    .collect::<Vec<_>>(),
            };
            for kind in kinds {
                if !found.contains(&kind) {
                    found.push(kind);
                }
            }
        }
        Ok(found)
    }

    pub async fn databases_with(&self, prefix: &str) -> Result<Vec<Uuid>, String> {
        let mut databases = Vec::new();
        for kind in self.registries().await? {
            let Ok(Some(content)) = self.peer.open::<PrefixRegistryContent>(kind).await else {
                continue;
            };
            for database in content.root().databases_with(prefix) {
                if databases.contains(&database) {
                    continue;
                }
                if self
                    .entry(database)
                    .await
                    .is_ok_and(|entry| entry.access.can_view() && !entry.deleted)
                {
                    databases.push(database);
                }
            }
        }
        Ok(databases)
    }

    pub async fn database_of_row(&self, id: &str) -> Result<Uuid, String> {
        let (prefix, _) = id
            .trim()
            .rsplit_once('-')
            .ok_or_else(|| format!("{id} is not a row id"))?;
        let databases = self.databases_with(prefix).await?;
        match databases.as_slice() {
            [database] => Ok(*database),
            [] => Err(format!(
                "no database this account can see has the prefix {}; name it: be DATABASE row {id}",
                be_block::normal_prefix(prefix)
            )),
            several => {
                let mut names = Vec::new();
                for database in several {
                    let entry = self.entry(*database).await?;
                    names.push(format!("{} ({})", entry.label(), entry.short));
                }
                Err(format!(
                    "the prefix {} names more than one database: {}; name the one you mean: be DATABASE row {id}",
                    be_block::normal_prefix(prefix),
                    names.join(", ")
                ))
            }
        }
    }
}
