use std::cell::RefCell;
use std::collections::HashMap;

use be_block::settings::{Settings, SettingsProfile};
use be_block::{
    BlockContent, BlockMetadata, DeviceClass, DisplaySettingsContent, DESKTOP_EDITOR,
    LiveEdit, NAME, OfflineSettings, OfflineSettingsContent, Resolution, ResolutionLayers, Root,
    SettingsContent, SettingsProfileContent, VIEW_EDITOR, VIEW_TYPE, WORKSPACE_EDITOR,
    WORKSPACE_TYPE, organization_settings,
};
use be_graph::BlockParent;
use uuid::Uuid;

use crate::be;

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
    be::hold(block, C::CONTENT_TYPE);
    C::decode(be::content(block)?.bytes()).ok()
}

#[derive(Default)]
struct Cache {
    owned: Option<(u64, Vec<Uuid>)>,
    settings: HashMap<Uuid, (u64, Settings)>,
    profiles: HashMap<Uuid, (u64, SettingsProfile)>,
    editors: HashMap<Uuid, (u64, Uuid)>,
    offline: HashMap<Uuid, (u64, OfflineSettings)>,
}

impl Cache {
    fn read<T: Clone>(
        held: &mut HashMap<Uuid, (u64, T)>,
        block: Uuid,
        content_type: Uuid,
        decode: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        be::hold(block, content_type);
        let revision = be::content_revision(block)?;
        if let Some((seen, value)) = held.get(&block)
            && *seen == revision
        {
            return Some(value.clone());
        }
        let value = decode()?;
        held.insert(block, (revision, value.clone()));
        Some(value)
    }
}

pub(crate) struct RootSettings {
    shell: Uuid,
    created: Option<Uuid>,
    created_files: Option<Uuid>,
    created_session: Option<Uuid>,
    chosen_session: Option<Uuid>,
    created_profile: Option<Uuid>,
    chosen_profile: Option<Uuid>,
    set_up: Option<Uuid>,
    cache: RefCell<Cache>,
}

impl RootSettings {
    pub(crate) fn new(shell: Uuid) -> Self {
        Self {
            shell,
            created: None,
            created_files: None,
            created_session: None,
            chosen_session: None,
            created_profile: None,
            chosen_profile: None,
            set_up: None,
            cache: RefCell::default(),
        }
    }

    pub(crate) fn shell(&self) -> Uuid {
        self.shell
    }

    fn owned(&self) -> Option<Vec<Uuid>> {
        let account = be::account()?;
        let revision = be::graph_revision();
        let mut cache = self.cache.borrow_mut();
        if let Some((seen, owned)) = &cache.owned
            && *seen == revision
        {
            return Some(owned.clone());
        }
        let mut owned: Vec<Uuid> = be::query(be::Query::Roots)
            .into_iter()
            .filter(|node| {
                node.content_type == Settings::CONTENT_TYPE && node.author == Some(account)
            })
            .map(|node| node.id)
            .collect();
        owned.sort_unstable();
        cache.owned = Some((revision, owned.clone()));
        Some(owned)
    }

    pub(crate) fn find(&self) -> Option<Uuid> {
        self.owned()?.first().copied().or(self.created)
    }

    pub(crate) fn ensure(&mut self) -> Option<Uuid> {
        if self.find().is_none() && be::ready(be::Query::Roots) {
            let block = Uuid::now_v7();
            be::create(
                block,
                SettingsContent::CONTENT_TYPE,
                BlockParent::Root,
                BlockMetadata::default(),
                Some(SettingsContent::default().encode()),
            );
            self.created = Some(block);
        }
        self.find()
    }

    pub(crate) fn my_files(&self) -> Option<Uuid> {
        let (account, organization) = be::identity()?;
        let block = be_block::my_files(organization, account);
        be::node(block)
            .is_some_and(|node| node.parent == Some(BlockParent::Root))
            .then_some(block)
            .or_else(|| self.created_files())
    }

    fn created_files(&self) -> Option<Uuid> {
        self.created_files.filter(|block| {
            be::node(*block).is_none_or(|node| node.parent != Some(BlockParent::Detached))
        })
    }

    pub(crate) fn ensure_my_files(&mut self) -> Option<Uuid> {
        if self.my_files().is_none()
            && be::ready(be::Query::Roots)
            && let Some((account, organization)) = be::identity()
            && be::ready(be::Query::Block(be_block::my_files(organization, account)))
        {
            let block = be_block::my_files(organization, account);
            if let Some(deleted) = be::node(block) {
                if deleted.parent == Some(BlockParent::Detached) {
                    be::set_parent(block, BlockParent::Root);
                    self.created_files = Some(block);
                }
                return self.my_files();
            }
            let mut metadata = BlockMetadata::default();
            metadata.insert(&NAME, &"My files".to_owned());
            be::create(block, WORKSPACE_TYPE, BlockParent::Root, metadata, None);
            self.created_files = Some(block);
        }
        self.my_files()
    }

    pub(crate) fn settings(&self) -> Option<Settings> {
        let mut blocks = self.owned()?;
        if blocks.is_empty() {
            blocks.extend(self.created);
        }
        if blocks.is_empty() {
            return None;
        }
        let mut cache = self.cache.borrow_mut();
        let held = blocks
            .into_iter()
            .map(|block| {
                Cache::read(&mut cache.settings, block, Settings::CONTENT_TYPE, || {
                    decode::<SettingsContent>(block).map(|document| document.root())
                })
            })
            .collect::<Option<Vec<Settings>>>()?;
        Some(Settings::merged(held))
    }

    pub(crate) fn loaded(&self) -> bool {
        self.settings().is_some()
    }

    fn settings_profile(&self, profile: Uuid) -> Option<SettingsProfile> {
        let mut cache = self.cache.borrow_mut();
        Cache::read(
            &mut cache.profiles,
            profile,
            SettingsProfile::CONTENT_TYPE,
            || decode::<SettingsProfileContent>(profile).map(|document| document.root()),
        )
    }

    pub(crate) fn session_editor(&self, session: Uuid) -> Option<Uuid> {
        let mut cache = self.cache.borrow_mut();
        Cache::read(&mut cache.editors, session, VIEW_TYPE, || {
            Some(
                be::node(session)?
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
        be::want(
            be::Owner::Organization,
            block.into_iter().map(be::Query::Block).collect(),
        );
        let held =
            block.filter(|block| be::node(*block).is_some_and(|node| node.access.can_view()));
        let held = held.and_then(|block| {
            let mut cache = self.cache.borrow_mut();
            Cache::read(&mut cache.settings, block, Settings::CONTENT_TYPE, || {
                decode::<SettingsContent>(block).map(|document| document.root())
            })
        });
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
        be::query(be::Query::Roots)
            .into_iter()
            .filter(|node| node.content_type == WORKSPACE_TYPE)
            .map(|node| node.id)
            .collect()
    }

    fn edit(&self, edit: &be_block::Edit) {
        let Some(block) = self.find() else {
            return;
        };
        be::operate(block, SettingsContent::encode_operation(edit));
    }

    fn named(block: Uuid, fallback: &str) -> String {
        be::node(block)
            .and_then(|node| node.metadata.name())
            .unwrap_or_else(|| fallback.to_owned())
    }

    pub(crate) fn layers(&self, kind: Uuid, client: Uuid) -> be_block::Layers {
        let Some(settings) = self.settings() else {
            return be_block::Layers::default();
        };
        let profile = self
            .current_profile(&settings, client)
            .and_then(|profile| self.settings_profile(profile));
        settings.layers(kind, profile.as_ref())
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
        be::create(
            session,
            VIEW_TYPE,
            BlockParent::Block(settings),
            BlockMetadata::view(editor, Some(name)),
            None,
        );
        Some(session)
    }

    fn create_profile(&self, name: String, content: &SettingsProfileContent) -> Option<Uuid> {
        let settings = self.find()?;
        let profile = Uuid::new_v4();
        be::create(
            profile,
            SettingsProfileContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            BlockMetadata::named(name),
            Some(content.encode()),
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
            be::create(
                display_block,
                DisplaySettingsContent::CONTENT_TYPE,
                BlockParent::Block(profile),
                BlockMetadata::default(),
                Some(DisplaySettingsContent::new(&display).encode()),
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
