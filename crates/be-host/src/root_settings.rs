use std::cell::RefCell;
use std::collections::HashMap;

use be_block::root_settings::SettingsGraph;
use be_block::settings::{Settings, SettingsProfile};
use be_block::{
    DESKTOP_EDITOR, DeviceClass, DisplaySettingsContent, OfflineSettings, OfflineSettingsContent,
    Resolution, ResolutionLayers, SettingsContent, SettingsProfileContent, UNFILED_TYPE,
    WORKSPACE_EDITOR, WORKSPACE_TYPE, organization_settings,
};
use be_block_content::{BlockContent, BlockMetadata, LiveEdit, NAME, Root, VIEW_EDITOR, VIEW_TYPE};
use be_block_graph::BlockParent;
use uuid::Uuid;

pub(crate) const SESSION_TYPES: [(Uuid, &str); 2] = [
    (WORKSPACE_EDITOR, "Workspace UI"),
    (DESKTOP_EDITOR, "Desktop"),
];

pub(crate) fn session_type_name(editor: Uuid) -> &'static str {
    SESSION_TYPES
        .iter()
        .find(|(id, _)| *id == editor)
        .map_or("Other", |(_, name)| name)
}

pub(crate) struct SessionEntry {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) editor: Uuid,
    pub(crate) current: bool,
}

pub(crate) struct ProfileEntry {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) current: bool,
}

pub(crate) fn device_class() -> DeviceClass {
    DeviceClass::detect(crate::display::epaper(), cfg!(target_os = "android"))
}

fn device_name(class: DeviceClass) -> &'static str {
    match cfg!(target_arch = "wasm32") {
        true => "Browser",
        false => class.name(),
    }
}

fn decode<C: BlockContent>(block: Uuid) -> Option<C> {
    be_host_stack::hold(block, C::CONTENT_TYPE);
    C::decode(be_host_stack::content(block)?.bytes()).ok()
}

/// What was read of the graph and of settings blocks, until the host stack says it changed
/// (`RootSettings::heard`).
#[derive(Default)]
struct Cache {
    /// The settings at the root, with their types and authors.
    roots: Option<Vec<(Uuid, Uuid, Uuid)>>,
    settings: HashMap<Uuid, Settings>,
    profiles: HashMap<Uuid, SettingsProfile>,
    editors: HashMap<Uuid, Uuid>,
    offline: HashMap<Uuid, OfflineSettings>,
}

impl Cache {
    fn read<T: Clone>(
        held: &mut HashMap<Uuid, T>,
        block: Uuid,
        content_type: Uuid,
        decode: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        be_host_stack::hold(block, content_type);
        if let Some(value) = held.get(&block) {
            return Some(value.clone());
        }
        let value = decode()?;
        held.insert(block, value.clone());
        Some(value)
    }

    fn heard(&mut self, changes: &be_host_stack::Changes) {
        if changes.graph {
            self.roots = None;
            // A session's editor is read from its metadata in the graph.
            self.editors.clear();
        }
        self.settings.retain(|block, _| !changes.block(*block));
        self.profiles.retain(|block, _| !changes.block(*block));
        self.editors.retain(|block, _| !changes.block(*block));
        self.offline.retain(|block, _| !changes.block(*block));
    }
}

/// The settings rules apps follow too (`be_block::root_settings`), over the host stack and what
/// be-host read of it.
struct Stack<'a>(&'a RefCell<Cache>);

impl SettingsGraph for Stack<'_> {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>> {
        be_host_stack::account()?;
        let mut cache = self.0.borrow_mut();
        let roots = cache.roots.get_or_insert_with(|| {
            // Only the settings among them, all the rules read, so a read copies a few. A
            // listing answered offline from what the device keeps carries each root's author
            // too, as the server gave it or as this device made the block (be-host-stack's
            // `roots_listed_offline_after_a_restart_keep_their_author`).
            be_host_stack::query(be_host_stack::Query::Roots)
                .into_iter()
                .filter(|node| node.content_type == Settings::CONTENT_TYPE)
                .filter_map(|node| Some((node.id, node.content_type, node.author?)))
                .collect()
        });
        Some(roots.clone())
    }

    fn listed(&self) -> bool {
        // The stack answers `ready` as it is now, while what was read of the roots may be from
        // before they were listed, until the host hears that the graph changed.
        let listed = be_host_stack::ready(be_host_stack::Query::Roots);
        if listed {
            self.0.borrow_mut().roots = None;
        }
        listed
    }

    fn account(&self) -> Uuid {
        be_host_stack::account().unwrap_or_default()
    }

    fn settings(&self, block: Uuid) -> Option<Settings> {
        let mut cache = self.0.borrow_mut();
        Cache::read(&mut cache.settings, block, Settings::CONTENT_TYPE, || {
            decode::<SettingsContent>(block).map(|document| document.root())
        })
    }

    fn settings_profile(&self, block: Uuid) -> Option<SettingsProfile> {
        let mut cache = self.0.borrow_mut();
        Cache::read(
            &mut cache.profiles,
            block,
            SettingsProfile::CONTENT_TYPE,
            || decode::<SettingsProfileContent>(block).map(|document| document.root()),
        )
    }

    fn create(
        &self,
        block: Uuid,
        block_type: Uuid,
        content: Vec<u8>,
        parent: BlockParent,
        owner_only: bool,
    ) -> Uuid {
        // A guest's settings only it reads (`be_host_stack::guest`).
        let owner_only = owner_only || (parent == BlockParent::Root && be_host_stack::guest());
        be_host_stack::create(
            block,
            block_type,
            parent,
            BlockMetadata::default(),
            Some(content),
            owner_only,
        );
        block
    }

    fn edit_settings(&self, block: Uuid, edit: be_block_content::Edit) {
        be_host_stack::operate(block, SettingsContent::encode_operation(&edit));
    }

    fn edit_settings_profile(&self, block: Uuid, edit: be_block_content::Edit) {
        be_host_stack::operate(block, SettingsProfileContent::encode_operation(&edit));
    }
}

pub(crate) struct RootSettings {
    shell: Uuid,
    root: be_block::root_settings::RootSettings,
    created_files: Option<Uuid>,
    created_unfiled: Option<Uuid>,
    created_session: Option<Uuid>,
    chosen_session: Option<Uuid>,
    created_profile: Option<Uuid>,
    chosen_profile: Option<Uuid>,
    set_up: Option<Uuid>,
    cache: RefCell<Cache>,
    /// Each reads kind's layer blocks as `reads_moved` last read them, and whether it sent
    /// their content (none of a layer was loaded yet, otherwise).
    reads: RefCell<HashMap<Uuid, (Vec<Uuid>, bool)>>,
}

impl RootSettings {
    pub(crate) fn new(shell: Uuid) -> Self {
        Self {
            shell,
            root: be_block::root_settings::RootSettings::default(),
            created_files: None,
            created_unfiled: None,
            created_session: None,
            chosen_session: None,
            created_profile: None,
            chosen_profile: None,
            set_up: None,
            cache: RefCell::default(),
            reads: RefCell::default(),
        }
    }

    /// Forgets what it read of what the host stack changed.
    pub(crate) fn heard(&self, changes: &be_host_stack::Changes) {
        self.cache.borrow_mut().heard(changes);
    }

    pub(crate) fn shell(&self) -> Uuid {
        self.shell
    }

    fn graph(&self) -> Stack<'_> {
        Stack(&self.cache)
    }

    pub(crate) fn find(&self) -> Option<Uuid> {
        self.root.find(&self.graph())
    }

    pub(crate) fn ensure(&mut self) -> Option<Uuid> {
        // Not `self.graph()`, which borrows all of `self` while `root` is borrowed mutably.
        self.root.ensure(&Stack(&self.cache))
    }

    pub(crate) fn my_files(&self) -> Option<Uuid> {
        let (account, organization) = be_host_stack::identity()?;
        let block = be_block::my_files(organization, account);
        be_host_stack::node(block)
            .is_some_and(|node| node.parent == Some(BlockParent::Root))
            .then_some(block)
            .or_else(|| self.created_files())
    }

    fn created_files(&self) -> Option<Uuid> {
        self.created_files.filter(|block| {
            be_host_stack::node(*block)
                .is_none_or(|node| node.parent != Some(BlockParent::Detached))
        })
    }

    pub(crate) fn ensure_my_files(&mut self) -> Option<Uuid> {
        // A guest keeps no files of its own here (`be_host_stack::guest`).
        if be_host_stack::guest() {
            return None;
        }
        if self.my_files().is_none()
            && be_host_stack::ready(be_host_stack::Query::Roots)
            && let Some((account, organization)) = be_host_stack::identity()
            && be_host_stack::ready(be_host_stack::Query::Block(be_block::my_files(
                organization,
                account,
            )))
        {
            let block = be_block::my_files(organization, account);
            if let Some(deleted) = be_host_stack::node(block) {
                if deleted.parent == Some(BlockParent::Detached) {
                    be_host_stack::set_parent(block, BlockParent::Root);
                    self.created_files = Some(block);
                }
                return self.my_files();
            }
            let mut metadata = BlockMetadata::default();
            metadata.insert(&NAME, &"My files".to_owned());
            be_host_stack::create(
                block,
                WORKSPACE_TYPE,
                BlockParent::Root,
                metadata,
                None,
                false,
            );
            self.created_files = Some(block);
        }
        self.my_files()
    }

    /// The account's Unfiled once the graph has it at the root.
    pub(crate) fn unfiled(&self) -> Option<Uuid> {
        let block = be_host_stack::unfiled()?;
        be_host_stack::node(block)
            .is_some_and(|node| node.parent == Some(BlockParent::Root))
            .then_some(block)
    }

    /// Makes the account's Unfiled once the server has said it has none. It has no content:
    /// it holds every block whose parent names it.
    pub(crate) fn ensure_unfiled(&mut self) -> Option<Uuid> {
        // A guest keeps no files of its own here (`be_host_stack::guest`).
        if be_host_stack::guest() {
            return None;
        }
        if self.unfiled().is_none()
            && let Some(block) = be_host_stack::unfiled()
            && self.created_unfiled != Some(block)
            && be_host_stack::ready(be_host_stack::Query::Block(block))
            && be_host_stack::node(block).is_none()
        {
            let mut metadata = BlockMetadata::default();
            metadata.insert(&NAME, &"Unfiled".to_owned());
            be_host_stack::create(
                block,
                UNFILED_TYPE,
                BlockParent::Root,
                metadata,
                None,
                false,
            );
            self.created_unfiled = Some(block);
        }
        self.unfiled()
    }

    pub(crate) fn settings(&self) -> Option<Settings> {
        self.root.read(&self.graph()).map(|held| held.settings)
    }

    pub(crate) fn loaded(&self) -> bool {
        self.settings().is_some()
    }

    pub(crate) fn session_editor(&self, session: Uuid) -> Option<Uuid> {
        let mut cache = self.cache.borrow_mut();
        Cache::read(&mut cache.editors, session, VIEW_TYPE, || {
            Some(
                be_host_stack::node(session)?
                    .metadata
                    .get(&VIEW_EDITOR)
                    .filter(|editor| !editor.is_nil())
                    .unwrap_or(self.shell),
            )
        })
    }

    fn offline_layer(&self, block: Option<Uuid>) -> Option<OfflineSettings> {
        let block = block?;
        let mut cache = self.cache.borrow_mut();
        Cache::read(
            &mut cache.offline,
            block,
            OfflineSettingsContent::CONTENT_TYPE,
            || decode::<OfflineSettingsContent>(block).map(|document| document.root()),
        )
    }

    pub(crate) fn offline_settings(
        &self,
        client: Uuid,
        organization: Option<Uuid>,
    ) -> Resolution<OfflineSettings> {
        let kind = OfflineSettingsContent::CONTENT_TYPE;
        let block = organization.map(organization_settings);
        be_host_stack::want(
            be_host_stack::Owner::Organization,
            block.into_iter().map(be_host_stack::Query::Block).collect(),
        );
        let held = block
            .filter(|block| be_host_stack::node(*block).is_some_and(|node| node.access.can_view()));
        let held = held.and_then(|block| self.graph().settings(block));
        let own = self.layers(kind, client);
        Resolution::resolve(ResolutionLayers {
            organization_policy: self
                .offline_layer(held.as_ref().and_then(|held| held.policy(kind))),
            organization_default: self
                .offline_layer(held.as_ref().and_then(|held| held.kind(kind))),
            block: None,
            every_device: self.offline_layer(own.every_device),
            this_device: self.offline_layer(own.this_device),
        })
    }

    pub(crate) fn workspaces(&self) -> Vec<Uuid> {
        be_host_stack::query(be_host_stack::Query::Roots)
            .into_iter()
            .filter(|node| node.content_type == WORKSPACE_TYPE)
            .map(|node| node.id)
            .collect()
    }

    fn edit(&self, edit: &be_block_content::Edit) {
        let Some(block) = self.find() else {
            return;
        };
        self.graph().edit_settings(block, edit.clone());
    }

    fn named(block: Uuid, fallback: &str) -> String {
        be_host_stack::node(block)
            .and_then(|node| node.metadata.name())
            .unwrap_or_else(|| fallback.to_owned())
    }

    pub(crate) fn layers(&self, kind: Uuid, client: Uuid) -> be_block_content::Layers {
        let Some(settings) = self.settings() else {
            return be_block_content::Layers::default();
        };
        let profile = self.current_profile(&settings, client);
        be_block::root_settings::layers(&self.graph(), &settings, kind, profile)
    }

    /// The account's layers of each reads kind of `kinds` (`be_host_stack::unread`) whose layers
    /// `changes` moved: each its content's bytes, lowest first (every device, then this
    /// device). Reads nothing when nothing changed, and only the kinds whose layer blocks
    /// changed or whose content did; a kind waits until each layer it has is loaded.
    pub(crate) fn reads_moved(
        &self,
        kinds: impl FnOnce() -> Vec<Uuid>,
        changes: &be_host_stack::Changes,
        client: Uuid,
    ) -> Vec<(Uuid, Vec<Vec<u8>>)> {
        // Which blocks hold the layers moves with the graph (the settings at the root), the
        // settings and the profile, whose content are block changes; the modules' kinds with
        // `stale`.
        if !(changes.graph || changes.stale || changes.every_block || !changes.blocks.is_empty()) {
            return Vec::new();
        }
        let mut read = self.reads.borrow_mut();
        let mut moved = Vec::new();
        for kind in kinds() {
            let layers = self.layers(kind, client);
            let blocks: Vec<Uuid> = [layers.every_device, layers.this_device]
                .into_iter()
                .flatten()
                .collect();
            let unchanged = read.get(&kind).is_some_and(|(held, sent)| {
                *sent && *held == blocks && !blocks.iter().any(|block| changes.block(*block))
            });
            if unchanged {
                continue;
            }
            let contents: Option<Vec<Vec<u8>>> = blocks
                .iter()
                .map(|block| {
                    be_host_stack::hold(*block, kind);
                    be_host_stack::content(*block).map(|content| content.bytes().to_vec())
                })
                .collect();
            read.insert(kind, (blocks, contents.is_some()));
            moved.extend(contents.map(|contents| (kind, contents)));
        }
        moved
    }

    fn current_profile(&self, settings: &Settings, client: Uuid) -> Option<Uuid> {
        self.chosen_profile
            .or(settings.profile(client))
            .or(self.created_profile)
    }

    pub(crate) fn sessions(&self, client: Uuid, every_type: bool) -> Vec<SessionEntry> {
        let Some(settings) = self.settings() else {
            return Vec::new();
        };
        let current = self.chosen_session.or(settings.session(self.shell, client));
        let mut sessions: Vec<SessionEntry> = settings
            .sessions()
            .into_iter()
            .filter_map(|session| {
                let editor = self.session_editor(session)?;
                if !every_type && editor != self.shell {
                    return None;
                }
                Some(SessionEntry {
                    id: session,
                    name: Self::named(session, "Session"),
                    editor,
                    current: current == Some(session),
                })
            })
            .collect();
        sessions.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        sessions
    }

    pub(crate) fn profiles(&self, client: Uuid) -> Vec<ProfileEntry> {
        let Some(settings) = self.settings() else {
            return Vec::new();
        };
        let current = self.current_profile(&settings, client);
        let mut profiles: Vec<ProfileEntry> = settings
            .profiles()
            .into_iter()
            .map(|profile| ProfileEntry {
                id: profile,
                name: Self::named(profile, "Profile"),
                current: current == Some(profile),
            })
            .collect();
        profiles.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        profiles
    }

    pub(crate) fn use_session(&mut self, client: Uuid, session: Uuid) {
        self.edit(&Settings::use_session(self.shell, client, session));
        self.chosen_session = Some(session);
    }

    pub(crate) fn use_profile(&mut self, client: Uuid, profile: Uuid) {
        self.edit(&Settings::use_profile(client, profile));
        self.chosen_profile = Some(profile);
    }

    fn create_session(&self, editor: Uuid, name: String) -> Option<Uuid> {
        let settings = self.find()?;
        let session = Uuid::new_v4();
        be_host_stack::create(
            session,
            VIEW_TYPE,
            BlockParent::Block(settings),
            BlockMetadata::view(editor, Some(name)),
            None,
            false,
        );
        Some(session)
    }

    fn create_profile(&self, name: String, content: &SettingsProfileContent) -> Option<Uuid> {
        let settings = self.find()?;
        let profile = Uuid::new_v4();
        be_host_stack::create(
            profile,
            SettingsProfileContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            BlockMetadata::named(name),
            Some(content.encode()),
            false,
        );
        Some(profile)
    }

    pub(crate) fn new_session(&mut self, client: Uuid, editor: Uuid) {
        let count = self
            .settings()
            .map_or(0, |settings| settings.sessions().len());
        let Some(session) = self.create_session(editor, format!("Session {}", count + 1)) else {
            return;
        };
        self.edit(&Settings::add_session(self.shell, client, session));
        self.chosen_session = Some(session);
    }

    pub(crate) fn new_profile(&mut self, client: Uuid) {
        let count = self
            .settings()
            .map_or(0, |settings| settings.profiles().len());
        let Some(profile) = self.create_profile(
            format!("Profile {}", count + 1),
            &SettingsProfileContent::default(),
        ) else {
            return;
        };
        self.edit(&Settings::add_profile(client, profile));
        self.chosen_profile = Some(profile);
    }

    pub(crate) fn ensure_profile(&mut self, client: Uuid, class: DeviceClass) -> Option<Uuid> {
        self.ensure()?;
        let settings = self.settings()?;
        let stored = settings.profile(client);
        if let Some(chosen) = self.chosen_profile {
            if stored != Some(chosen) {
                return Some(chosen);
            }
            self.chosen_profile = None;
        }
        if stored.is_some() {
            self.created_profile = None;
            return stored;
        }
        if self.created_profile.is_some() {
            return self.created_profile;
        }
        let display = class.display_settings();
        let defaults = display != be_block::DisplaySettings::default();
        let display_block = Uuid::new_v4();
        let mut content = SettingsProfileContent::new(&SettingsProfile {
            class: Some(class),
            ..SettingsProfile::default()
        });
        if defaults {
            content.apply(&SettingsProfile::set_kind(
                DisplaySettingsContent::CONTENT_TYPE,
                display_block,
            ));
        }
        let profile = self.create_profile(device_name(class).to_owned(), &content)?;
        if defaults {
            be_host_stack::create(
                display_block,
                DisplaySettingsContent::CONTENT_TYPE,
                BlockParent::Block(profile),
                BlockMetadata::default(),
                Some(DisplaySettingsContent::new(&display).encode()),
                false,
            );
            self.set_up = Some(profile);
        }
        self.edit(&Settings::add_profile(client, profile));
        self.created_profile = Some(profile);
        Some(profile)
    }

    pub(crate) fn take_set_up(&mut self) -> Option<Uuid> {
        self.set_up.take()
    }

    pub(crate) fn ensure_session(&mut self, client: Uuid, class: DeviceClass) -> Option<Uuid> {
        self.ensure()?;
        let settings = self.settings()?;
        let stored = settings.session(self.shell, client);
        if let Some(chosen) = self.chosen_session {
            if stored != Some(chosen) {
                return Some(chosen);
            }
            self.chosen_session = None;
        }
        if stored.is_some() {
            self.created_session = None;
            return stored;
        }
        if self.created_session.is_some() {
            return self.created_session;
        }
        let name = match self.shell == DESKTOP_EDITOR {
            true => "Desktop",
            false => device_name(class),
        };
        let session = self.create_session(self.shell, name.to_owned())?;
        self.edit(&Settings::add_session(self.shell, client, session));
        self.created_session = Some(session);
        Some(session)
    }
}

#[cfg(test)]
mod tests;
