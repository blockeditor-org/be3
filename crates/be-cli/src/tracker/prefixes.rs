use be_block::{BlockContent, PREFIXES, PrefixRegistryContent};
use be_graph::BlockParent;
use be_store::ObjectStore;
use database_types::database::DatabaseContent;
use uuid::Uuid;

use super::Tracker;
use crate::session::explain;

impl<S: ObjectStore> Tracker<S> {
    async fn registry_of(&self, block: Uuid) -> Result<Option<Uuid>, String> {
        let summary = self.peer.summary(block).await.map_err(explain)?;
        if !summary.access.can_view() {
            return Ok(None);
        }
        let metadata = self.peer.metadata(&summary).map_err(explain)?;
        Ok(metadata.get(&PREFIXES).map(|kind| kind.0))
    }

    pub async fn register(&self, database: Uuid, prefix: &str) -> Result<(), String> {
        self.peer
            .register_prefix(database, Some(prefix))
            .await
            .map_err(explain)?;
        self.invalidate().await;
        Ok(())
    }

    pub async fn register_uploaded(
        &self,
        blocks: &[be_client::transfer::Saved],
    ) -> Result<(), String> {
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
            let owner = self
                .peer
                .summary(kind)
                .await
                .ok()
                .and_then(|summary| summary.parent?.block());
            for database in content.root().databases_with(prefix) {
                if databases.contains(&database) {
                    continue;
                }
                let shown = self
                    .entry(database)
                    .await
                    .is_ok_and(|entry| entry.access.can_view() && !entry.deleted);
                if shown && self.lies_below(owner, database).await {
                    databases.push(database);
                }
            }
        }
        Ok(databases)
    }

    pub(super) async fn lies_below(&self, owner: Option<Uuid>, block: Uuid) -> bool {
        let Some(owner) = owner else {
            return true;
        };
        let mut current = block;
        for _ in 0..256 {
            match self.entry(current).await.map(|entry| entry.parent) {
                Ok(BlockParent::Block(parent)) if parent == owner => return true,
                Ok(BlockParent::Block(parent)) => current = parent,
                _ => return false,
            }
        }
        false
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
