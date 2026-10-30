use stable_hash::HashMap;
use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use be_block::be_model::{BlockRef, Document};
use be_block::{
    ANYONE_INSIDE, BlockContent, EDIT_KINDS, Follows, KindVisibility, Layered, LiveEdit,
    Resolution, ResolutionLayers, Root, SETTINGS, Settings, SettingsContent, WORKSPACE_TYPE,
    edit_kind_key,
};
use beui::reactive::*;
use block_plugin_api::AccessLevel;
use uuid::Uuid;

use super::Editor;
use crate::root_settings::{RootSettings, layers};
use crate::{BlockList, BlockParent, BlockQuery};

const MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OrganizationLayers {
    pub policy: Option<Uuid>,
    pub default: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetKind {
    pub owner: Uuid,
    pub settings: Uuid,
    pub block: Uuid,
}

pub type KindsAbove = BTreeMap<Uuid, SetKind>;

#[derive(Default)]
pub(super) struct SettingsCache {
    walks: HashMap<Uuid, [BlockList; 3]>,
    held: HashMap<Uuid, BlockList>,
    kinds: HashMap<Uuid, Memo<KindsAbove>>,
    resolved: HashMap<(Uuid, TypeId), Box<dyn Any>>,
    created: HashMap<Uuid, Uuid>,
    created_kinds: HashMap<(Uuid, Uuid), Uuid>,
    organization: Option<BlockList>,
}

impl Editor {
    pub fn organization_settings(&self) -> Option<Uuid> {
        let settings = be_block::organization_settings(self.host().organization_id());
        let state = self.state();
        if state.settings.borrow().organization.is_none() {
            let editor = *self;
            let listed = state
                ._scope
                .run(|| editor.blocks().watch(BlockQuery::Block(settings)));
            state.settings.borrow_mut().organization = Some(listed);
        }
        self.blocks()
            .info(settings)
            .filter(|info| info.access.can_view())
            .map(|_| settings)
    }

    pub fn organization_layers(&self, kind: Uuid) -> OrganizationLayers {
        let Some(settings) = self.organization_settings() else {
            return OrganizationLayers::default();
        };
        self.content_of::<SettingsContent>(settings)
            .read(|content| {
                let root = content.root();
                OrganizationLayers {
                    policy: root.policy(kind),
                    default: root.kind(kind),
                }
            })
            .unwrap_or_default()
    }

    pub fn settings_block(&self, owner: Uuid) -> Option<Uuid> {
        self.blocks()
            .info(owner)
            .and_then(|info| info.metadata.get(&SETTINGS))
            .map(|found| found.0)
            .or_else(|| self.state().settings.borrow().created.get(&owner).copied())
    }

    pub fn settings_owner(&self, settings: Uuid) -> Option<Uuid> {
        let info = self.blocks().info(settings)?;
        match (info.block_type == Settings::CONTENT_TYPE, info.parent) {
            (true, Some(BlockParent::Block(owner))) => Some(owner),
            _ => None,
        }
    }

    pub fn edit_kind(&self, owner: Uuid, kind: Uuid) -> Option<Uuid> {
        self.blocks()
            .info(owner)
            .and_then(|info| info.metadata.get(&edit_kind_key(kind)))
            .map(|found| found.0)
            .or_else(|| {
                self.state()
                    .settings
                    .borrow()
                    .created_kinds
                    .get(&(owner, kind))
                    .copied()
            })
    }

    pub fn workspace_of(&self, block: Uuid) -> Option<Uuid> {
        let blocks = self.blocks();
        let mut current = block;
        let mut outermost = None;
        for _ in 0..MAX_DEPTH {
            let Some(info) = blocks.info(current).filter(|info| info.access.can_view()) else {
                break;
            };
            if info.block_type == WORKSPACE_TYPE {
                return Some(current);
            }
            outermost = Some(current);
            let Some(BlockParent::Block(parent)) = info.parent else {
                break;
            };
            current = parent;
        }
        outermost
    }

    pub fn kinds_above(&self, block: Uuid) -> Memo<KindsAbove> {
        if let Some(found) = self.state().settings.borrow().kinds.get(&block) {
            return *found;
        }
        let editor = *self;
        let state = self.state();
        let memo = state._scope.run(|| {
            let blocks = editor.blocks();
            let walk = [
                blocks.watch(BlockQuery::Block(block)),
                blocks.watch(BlockQuery::Parents(block)),
                blocks.watch(BlockQuery::OpenSettings(block)),
            ];
            state.settings.borrow_mut().walks.insert(block, walk);
            create_memo(move || editor.walk(block))
        });
        state.settings.borrow_mut().kinds.insert(block, memo);
        memo
    }

    fn walk(&self, block: Uuid) -> KindsAbove {
        let blocks = self.blocks();
        let mut found = KindsAbove::new();
        let mut current = block;
        for _ in 0..MAX_DEPTH {
            let Some(info) = blocks.info(current).filter(|info| info.access.can_view()) else {
                self.open_above(block, &mut found);
                break;
            };
            for kind in EDIT_KINDS {
                if let Some(set) = self.edit_kind(current, kind)
                    && self.held_by(set, kind, current)
                {
                    found.entry(kind).or_insert(SetKind {
                        owner: current,
                        settings: current,
                        block: set,
                    });
                }
            }
            if let Some(settings) = self.settings_block(current)
                && self.held_by(settings, Settings::CONTENT_TYPE, current)
            {
                let kinds = self
                    .content_of::<SettingsContent>(settings)
                    .read(|content| content.root().kinds.clone())
                    .unwrap_or_default();
                for (kind, set) in kinds.iter() {
                    if !self.held_by(*set, *kind, settings) {
                        continue;
                    }
                    found.entry(*kind).or_insert(SetKind {
                        owner: current,
                        settings,
                        block: *set,
                    });
                }
            }
            let Some(BlockParent::Block(parent)) = info.parent else {
                break;
            };
            current = parent;
        }
        found
    }

    fn held_by(&self, block: Uuid, block_type: Uuid, parent: Uuid) -> bool {
        let blocks = self.blocks();
        if blocks.info(block).is_none() {
            let state = self.state();
            let mut cache = state.settings.borrow_mut();
            if !cache.held.contains_key(&block) {
                let list = state._scope.run(|| blocks.watch(BlockQuery::Block(block)));
                cache.held.insert(block, list);
            }
        }
        blocks.info(block).is_some_and(|info| {
            info.block_type == block_type && info.parent == Some(BlockParent::Block(parent))
        })
    }

    fn open_above(&self, block: Uuid, found: &mut KindsAbove) {
        let open = self
            .state()
            .settings
            .borrow()
            .walks
            .get(&block)
            .map(|[_, _, open]| open.read().unwrap_or_default())
            .unwrap_or_default();
        for info in open {
            let Some(BlockParent::Block(settings)) = info.parent else {
                continue;
            };
            found.entry(info.block_type).or_insert(SetKind {
                owner: settings,
                settings,
                block: info.id,
            });
        }
    }

    pub fn nearest_settings(&self, block: Uuid, kind: Uuid) -> Memo<Option<SetKind>> {
        let kinds = self.kinds_above(block);
        create_memo(move || kinds.with(|kinds| kinds.get(&kind).copied()))
    }

    pub fn resolved_settings<R>(&self, block: Uuid) -> Memo<Resolution<R>>
    where
        R: Layered + PartialEq + 'static,
        Document<R>: LiveEdit + Clone + Default,
    {
        let key = (block, TypeId::of::<R>());
        if let Some(found) = self
            .state()
            .settings
            .borrow()
            .resolved
            .get(&key)
            .and_then(|found| found.downcast_ref::<Memo<Resolution<R>>>())
        {
            return *found;
        }
        let editor = *self;
        let kinds = match R::FOLLOWS {
            Follows::Block => Some(self.kinds_above(block)),
            Follows::Viewer => None,
        };
        let state = self.state();
        let memo = state._scope.run(|| {
            create_memo(move || {
                let read = |block: Option<Uuid>| -> Option<R> {
                    let block = block?;
                    editor
                        .content_of::<Document<R>>(block)
                        .read(|content| content.root())
                        .ok()
                };
                let client = editor.host().client_id();
                let own = RootSettings::default()
                    .read(&editor)
                    .map(|held| layers(&editor, &held, R::CONTENT_TYPE, client))
                    .unwrap_or_default();
                let organization = editor.organization_layers(R::CONTENT_TYPE);
                let set = kinds
                    .and_then(|kinds| kinds.with(|kinds| kinds.get(&R::CONTENT_TYPE).copied()));
                Resolution::resolve(ResolutionLayers {
                    organization_policy: read(organization.policy),
                    organization_default: read(organization.default),
                    block: set.and_then(|set| Some((set.owner, read(Some(set.block))?))),
                    every_device: read(own.every_device),
                    this_device: read(own.this_device),
                })
            })
        });
        state
            .settings
            .borrow_mut()
            .resolved
            .insert(key, Box::new(memo));
        memo
    }

    pub fn effective_settings<R>(&self) -> Memo<R>
    where
        R: Layered + PartialEq + 'static,
        Document<R>: LiveEdit + Clone + Default,
    {
        let resolved = self.resolved_settings::<R>(self.block_id());
        create_memo(move || resolved.with(Resolution::value))
    }

    pub fn inherited_settings<R>(&self) -> Memo<R>
    where
        R: Layered + PartialEq + 'static,
        Document<R>: LiveEdit + Clone + Default,
    {
        let editor = *self;
        create_memo(move || {
            let below = RootSettings::default()
                .read(&editor)
                .map(|held| {
                    layers(&editor, &held, R::CONTENT_TYPE, editor.host().client_id())
                        .below(editor.block_id())
                })
                .unwrap_or_default();
            be_block::layered(below.into_iter().filter_map(|block| {
                editor
                    .content_of::<Document<R>>(block)
                    .read(|content| content.root())
                    .ok()
            }))
        })
    }

    pub fn ensure_settings_block(&self, owner: Uuid) -> Option<Uuid> {
        if let Some(found) = untrack(|| self.settings_block(owner)) {
            return Some(found);
        }
        self.create_settings_block(owner, SettingsContent::default())
    }

    fn create_settings_block(&self, owner: Uuid, content: SettingsContent) -> Option<Uuid> {
        if !untrack(|| self.blocks().access(owner)).can_manage() {
            return None;
        }
        let blocks = self.blocks();
        let settings = blocks.create_with(
            Settings::CONTENT_TYPE,
            Some(content.encode()),
            BlockParent::Block(owner),
            be_block::BlockMetadata::default(),
        );
        blocks.define(owner, SETTINGS.definition(Some(&BlockRef(settings))));
        self.state()
            .settings
            .borrow_mut()
            .created
            .insert(owner, settings);
        Some(settings)
    }

    pub fn ensure_block_kind<R>(&self, owner: Uuid, content: Document<R>) -> Option<Uuid>
    where
        R: Layered + 'static,
        Document<R>: BlockContent,
    {
        let kind = R::CONTENT_TYPE;
        if EDIT_KINDS.contains(&kind) {
            return self.ensure_edit_kind::<R>(owner, content);
        }
        let existing = untrack(|| self.settings_block(owner));
        let block = Uuid::now_v7();
        let settings = match existing {
            Some(settings) => {
                if let Some(found) = self
                    .state()
                    .settings
                    .borrow()
                    .created_kinds
                    .get(&(settings, kind))
                {
                    return Some(*found);
                }
                let held = self.content_of::<SettingsContent>(settings);
                let own = untrack(|| held.read(|content| content.root().kind(kind))).ok()?;
                if let Some(found) = own {
                    return Some(found);
                }
                held.operate(Settings::set_kind(kind, block));
                settings
            }
            None => {
                let mut content = SettingsContent::default();
                content.apply(&Settings::set_kind(kind, block));
                self.create_settings_block(owner, content)?
            }
        };
        self.blocks().create_at(
            block,
            kind,
            Some(content.encode()),
            BlockParent::Block(settings),
            be_block::BlockMetadata::default(),
        );
        if R::VISIBILITY == KindVisibility::Inside {
            self.host()
                .set_access(block, ANYONE_INSIDE, AccessLevel::View);
        }
        self.state()
            .settings
            .borrow_mut()
            .created_kinds
            .insert((settings, kind), block);
        Some(block)
    }

    fn ensure_edit_kind<R>(&self, owner: Uuid, content: Document<R>) -> Option<Uuid>
    where
        R: Layered + 'static,
        Document<R>: BlockContent,
    {
        let kind = R::CONTENT_TYPE;
        if let Some(found) = untrack(|| self.edit_kind(owner, kind)) {
            return Some(found);
        }
        if !untrack(|| self.blocks().access(owner)).can_edit() {
            return None;
        }
        let block = Uuid::now_v7();
        let blocks = self.blocks();
        blocks.create_at(
            block,
            kind,
            Some(content.encode()),
            BlockParent::Block(owner),
            be_block::BlockMetadata::default(),
        );
        blocks.define(
            owner,
            edit_kind_key(kind).definition(Some(&BlockRef(block))),
        );
        if R::VISIBILITY == KindVisibility::Inside {
            self.host()
                .set_access(block, ANYONE_INSIDE, AccessLevel::View);
        }
        self.state()
            .settings
            .borrow_mut()
            .created_kinds
            .insert((owner, kind), block);
        Some(block)
    }

    pub fn change_block_kind<R>(&self, owner: Uuid, edit: be_block::Edit) -> Option<Uuid>
    where
        R: Layered + 'static,
        Document<R>: BlockContent + LiveEdit<Op = be_block::Edit> + Clone + Default,
    {
        let kinds = self.kinds_above(owner);
        let here = untrack(|| {
            kinds.with(|kinds| {
                kinds
                    .get(&R::CONTENT_TYPE)
                    .filter(|set| set.owner == owner)
                    .map(|set| set.block)
            })
        })
        .or_else(|| {
            let settings = untrack(|| self.settings_block(owner))?;
            self.state()
                .settings
                .borrow()
                .created_kinds
                .get(&(settings, R::CONTENT_TYPE))
                .copied()
        });
        if let Some(block) = here {
            self.content_of::<Document<R>>(block).operate(edit);
            return Some(block);
        }
        let mut content = Document::<R>::default();
        content.apply(&edit);
        self.ensure_block_kind::<R>(owner, content)
    }

    pub fn clear_block_kind(&self, settings: Uuid, kind: Uuid) {
        if EDIT_KINDS.contains(&kind) {
            let owner = untrack(|| self.settings_owner(settings)).unwrap_or(settings);
            let Some(block) = untrack(|| self.edit_kind(owner, kind)) else {
                return;
            };
            self.state()
                .settings
                .borrow_mut()
                .created_kinds
                .remove(&(owner, kind));
            self.blocks()
                .define(owner, edit_kind_key(kind).definition(None));
            self.host()
                .delete_block(block, kind, BlockParent::Block(owner));
            return;
        }
        let held = self.content_of::<SettingsContent>(settings);
        let Ok(Some(block)) = untrack(|| held.read(|content| content.root().kind(kind))) else {
            return;
        };
        self.state()
            .settings
            .borrow_mut()
            .created_kinds
            .remove(&(settings, kind));
        self.host()
            .delete_block(block, kind, BlockParent::Block(settings));
    }
}
