use be_block::settings::{Settings, SettingsProfile};
use be_block::{BlockContent, Edit, Root, SettingsContent, SettingsLayer};
use uuid::Uuid;

use crate::BlockParent;

pub trait SettingsGraph {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>>;
    fn account(&self) -> Uuid;
    fn settings(&self, block: Uuid) -> Option<Settings>;
    fn settings_profile(&self, block: Uuid) -> Option<SettingsProfile>;
    fn create(&self, block_type: Uuid, content: Vec<u8>, parent: BlockParent) -> Uuid;
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
        if let Some(block) = self.find(graph) {
            return Some(block);
        }
        graph.roots()?;
        let block = graph.create(
            SettingsContent::CONTENT_TYPE,
            SettingsContent::default().encode(),
            BlockParent::Root,
        );
        self.created = Some(block);
        Some(block)
    }
}

pub use be_block::Layers;

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
    content: std::marker::PhantomData<C>,
}

impl<C> Default for RootSetting<C> {
    fn default() -> Self {
        Self {
            settings: RootSettings::default(),
            created: None,
            created_here: None,
            block: None,
            content: std::marker::PhantomData,
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
        let settings = self.settings.ensure(graph)?;
        graph.settings(settings)?;
        let block = graph.create(
            C::CONTENT_TYPE,
            C::default().encode(),
            BlockParent::Block(settings),
        );
        graph.edit_settings(settings, Settings::set_kind(C::CONTENT_TYPE, block));
        self.created = Some(block);
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
        let settings = self.settings.ensure(graph)?;
        graph.settings(settings)?;
        let parent = match layer {
            SettingsLayer::EveryDevice => settings,
            SettingsLayer::ThisDevice => {
                let profile = self.settings.read(graph)?.settings.profile(client)?;
                graph.settings_profile(profile)?;
                profile
            }
        };
        let block = graph.create(C::CONTENT_TYPE, content.encode(), BlockParent::Block(parent));
        match layer {
            SettingsLayer::EveryDevice => {
                graph.edit_settings(settings, Settings::set_kind(C::CONTENT_TYPE, block))
            }
            SettingsLayer::ThisDevice => graph.edit_settings_profile(
                parent,
                SettingsProfile::set_kind(C::CONTENT_TYPE, block),
            ),
        }
        match layer {
            SettingsLayer::EveryDevice => self.created = Some(block),
            SettingsLayer::ThisDevice => self.created_here = Some(block),
        }
        Some(block)
    }
}
