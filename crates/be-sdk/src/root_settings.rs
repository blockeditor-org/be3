use be_block::SettingsContent;
use be_block::settings::{Settings, SettingsProfile};
use be_block_content::{BlockContent, Edit, Root, SettingsLayer};
use uuid::Uuid;

use crate::BlockParent;

pub trait SettingsGraph {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>>;
    fn account(&self) -> Uuid;
    fn settings(&self, block: Uuid) -> Option<Settings>;
    fn settings_profile(&self, block: Uuid) -> Option<SettingsProfile>;
    /// Makes `block`; `owner_only` makes one only this account reads (`KindVisibility::Owner`).
    fn create(
        &self,
        block: Uuid,
        block_type: Uuid,
        content: Vec<u8>,
        parent: BlockParent,
        owner_only: bool,
    ) -> Uuid;
    fn edit_settings(&self, block: Uuid, edit: Edit);
    fn edit_settings_profile(&self, block: Uuid, edit: Edit);
}

pub fn own_settings_blocks(graph: &dyn SettingsGraph) -> Option<Vec<Uuid>> {
    let account = graph.account();
    let mut blocks: Vec<Uuid> = graph
        .roots()?
        .into_iter()
        .filter(|(_, block_type, author)| {
            *block_type == Settings::CONTENT_TYPE && *author == account
        })
        .map(|(id, _, _)| id)
        .collect();
    blocks.sort_unstable();
    Some(blocks)
}

pub fn is_own_settings(graph: &dyn SettingsGraph, block: Uuid) -> bool {
    own_settings_blocks(graph).is_some_and(|blocks| blocks.contains(&block))
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeldSettings {
    pub block: Uuid,
    pub settings: Settings,
}

#[derive(Default)]
pub struct RootSettings {
    created: Option<Uuid>,
}

impl RootSettings {
    pub fn read(&self, graph: &dyn SettingsGraph) -> Option<HeldSettings> {
        let blocks = own_settings_blocks(graph)?;
        let Some(&block) = blocks.first() else {
            let block = self.created?;
            return Some(HeldSettings {
                block,
                settings: graph.settings(block)?,
            });
        };
        let held: Vec<Settings> = blocks
            .iter()
            .map(|block| graph.settings(*block))
            .collect::<Option<_>>()?;
        Some(HeldSettings {
            block,
            settings: Settings::merged(held),
        })
    }

    pub fn find(&self, graph: &dyn SettingsGraph) -> Option<Uuid> {
        own_settings_blocks(graph)?
            .first()
            .copied()
            .or(self.created)
    }

    pub fn ensure(&mut self, graph: &dyn SettingsGraph) -> Option<Uuid> {
        self.ensure_with(graph, SettingsContent::default())
            .map(|(block, _)| block)
    }

    /// The account's `Settings` block, made with `content` when it has none, and whether it
    /// was made here: a block just made can't be read back yet, so whatever it should hold
    /// goes in `content` rather than in an edit after it.
    pub fn ensure_with(
        &mut self,
        graph: &dyn SettingsGraph,
        content: SettingsContent,
    ) -> Option<(Uuid, bool)> {
        if let Some(block) = self.find(graph) {
            return Some((block, false));
        }
        graph.roots()?;
        let block = Uuid::now_v7();
        graph.create(
            block,
            SettingsContent::CONTENT_TYPE,
            content.encode(),
            BlockParent::Root,
            false,
        );
        self.created = Some(block);
        Some((block, true))
    }
}

pub use be_block_content::Layers;

pub fn layers(graph: &dyn SettingsGraph, held: &HeldSettings, kind: Uuid, client: Uuid) -> Layers {
    let own = held
        .settings
        .profile(client)
        .and_then(|profile| graph.settings_profile(profile))
        .and_then(|profile| profile.kind(kind));
    Layers {
        every_device: held.settings.kind(kind),
        this_device: own,
    }
}

pub struct RootSetting<C> {
    settings: RootSettings,
    created: Option<Uuid>,
    created_here: Option<Uuid>,
    block: Option<Uuid>,
    owner_only: bool,
    content: std::marker::PhantomData<C>,
}

impl<C> Default for RootSetting<C> {
    fn default() -> Self {
        Self {
            settings: RootSettings::default(),
            created: None,
            created_here: None,
            block: None,
            owner_only: false,
            content: std::marker::PhantomData,
        }
    }
}

impl<C> RootSetting<C> {
    /// A kind whose blocks only this account reads (`KindVisibility::Owner`).
    pub fn owner_only() -> Self {
        Self {
            owner_only: true,
            ..Self::default()
        }
    }
}

impl<C: BlockContent + Default> RootSetting<C> {
    pub fn block(&self) -> Option<Uuid> {
        self.block
    }

    pub fn layers(&self, graph: &dyn SettingsGraph, client: Uuid) -> Option<Layers> {
        let held = self.settings.read(graph)?;
        Some(layers(graph, &held, C::CONTENT_TYPE, client))
    }

    pub fn find(&mut self, graph: &dyn SettingsGraph, client: Uuid) -> Option<Uuid> {
        self.block = self
            .layers(graph, client)
            .and_then(Layers::top)
            .or(self.created);
        self.block
    }

    pub fn ensure(&mut self, graph: &dyn SettingsGraph, client: Uuid) -> Option<Uuid> {
        if self.find(graph, client).is_some() {
            return self.block;
        }
        let block = self.create_layer(graph, client, SettingsLayer::EveryDevice, C::default())?;
        self.block = Some(block);
        self.block
    }

    pub fn layer_blocks(&self, graph: &dyn SettingsGraph, client: Uuid) -> Layers {
        let layers = self.layers(graph, client).unwrap_or_default();
        Layers {
            every_device: layers.every_device.or(self.created),
            this_device: layers.this_device.or(self.created_here),
        }
    }

    pub fn create_layer(
        &mut self,
        graph: &dyn SettingsGraph,
        client: Uuid,
        layer: SettingsLayer,
        content: C,
    ) -> Option<Uuid> {
        let block = Uuid::now_v7();
        let set_kind = Settings::set_kind(C::CONTENT_TYPE, block);
        let parent = match layer {
            SettingsLayer::EveryDevice => {
                let mut made = SettingsContent::default();
                made.apply(&set_kind);
                let (settings, created) = self.settings.ensure_with(graph, made)?;
                if !created {
                    graph.settings(settings)?;
                    graph.edit_settings(settings, set_kind);
                }
                settings
            }
            SettingsLayer::ThisDevice => {
                let settings = self.settings.ensure(graph)?;
                graph.settings(settings)?;
                let profile = self.settings.read(graph)?.settings.profile(client)?;
                graph.settings_profile(profile)?;
                graph.edit_settings_profile(
                    profile,
                    SettingsProfile::set_kind(C::CONTENT_TYPE, block),
                );
                profile
            }
        };
        graph.create(
            block,
            C::CONTENT_TYPE,
            content.encode(),
            BlockParent::Block(parent),
            self.owner_only,
        );
        match layer {
            SettingsLayer::EveryDevice => self.created = Some(block),
            SettingsLayer::ThisDevice => self.created_here = Some(block),
        }
        Some(block)
    }
}

#[cfg(test)]
mod tests;
