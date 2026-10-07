use std::cell::RefCell;
use std::collections::HashMap;

use be_block::settings::Settings;
use be_block::{
    BlockContent, BlockMetadata, EditorView, EditorViewContent, LINUX_DESKTOP_EDITOR, LiveEdit,
    Root, SettingsContent,
};
use be_graph::BlockParent;
use uuid::Uuid;

use crate::be;

pub(crate) fn settings(block: Uuid) -> Option<Settings> {
    be::hold(block, SettingsContent::CONTENT_TYPE);
    let content = be::content(block)?;
    SettingsContent::decode(&content.bytes)
        .ok()
        .map(|document| document.root().clone())
}

pub(crate) fn device_name() -> &'static str {
    if cfg!(target_os = "android") {
        "Phone"
    } else if cfg!(target_arch = "wasm32") {
        "Browser"
    } else {
        "Computer"
    }
}

pub(crate) struct RootSettings {
    shell: Uuid,
    block: Option<Uuid>,
    created_profile: Option<Uuid>,
    chosen_profile: Option<Uuid>,
    decoded: RefCell<Option<(u64, Settings)>>,
    editors: RefCell<HashMap<Uuid, (u64, Uuid)>>,
}

impl RootSettings {
    pub(crate) fn new(shell: Uuid) -> Self {
        Self {
            shell,
            block: None,
            created_profile: None,
            chosen_profile: None,
            decoded: RefCell::default(),
            editors: RefCell::default(),
        }
    }

    pub(crate) fn shell(&self) -> Uuid {
        self.shell
    }

    fn session_document(&self) -> Vec<u8> {
        EditorView::document(self.shell, None).encode()
    }

    fn profile_name(&self) -> &'static str {
        match self.shell == LINUX_DESKTOP_EDITOR {
            true => "Desktop",
            false => device_name(),
        }
    }

    fn editor_of(&self, profile: Uuid) -> Option<Uuid> {
        be::hold(profile, EditorViewContent::CONTENT_TYPE);
        let revision = be::content_revision(profile)?;
        if let Some((held, editor)) = self.editors.borrow().get(&profile)
            && *held == revision
        {
            return Some(*editor);
        }
        let content = be::content(profile)?;
        let editor = EditorViewContent::decode(&content.bytes)
            .ok()?
            .root()
            .editor;
        self.editors
            .borrow_mut()
            .insert(profile, (revision, editor));
        Some(editor)
    }

    pub(crate) fn find(&mut self) -> Option<Uuid> {
        if self.block.is_none() {
            let account = be::account()?;
            self.block = be::query(be::Query::Roots)
                .into_iter()
                .filter(|node| {
                    node.content_type == Settings::CONTENT_TYPE && node.author == account
                })
                .map(|node| node.id)
                .min();
        }
        self.block
    }

    pub(crate) fn ensure(&mut self) -> Option<Uuid> {
        if self.find().is_none() && be::graph_loaded() {
            let block = Uuid::new_v4();
            be::create(
                block,
                SettingsContent::CONTENT_TYPE,
                BlockParent::Root,
                BlockMetadata::default(),
                Some(SettingsContent::default().encode()),
            );
            self.block = Some(block);
        }
        self.block
    }

    pub(crate) fn loaded(&self) -> bool {
        self.block
            .is_some_and(|block| self.decoded(block).is_some())
    }

    fn decoded(&self, block: Uuid) -> Option<Settings> {
        be::hold(block, SettingsContent::CONTENT_TYPE);
        let revision = be::content_revision(block)?;
        let mut decoded = self.decoded.borrow_mut();
        match &*decoded {
            Some((held, settings)) if *held == revision => Some(settings.clone()),
            _ => {
                let settings = settings(block)?;
                *decoded = Some((revision, settings.clone()));
                Some(settings)
            }
        }
    }

    pub(crate) fn profiles(&self, client: Uuid) -> Vec<(Uuid, String, bool)> {
        let Some(settings) = self.block.and_then(|block| self.decoded(block)) else {
            return Vec::new();
        };
        let current = self.chosen_profile.or(settings.profile(self.shell, client));
        let mut profiles: Vec<(Uuid, String, bool)> = settings
            .profiles()
            .into_iter()
            .filter(|profile| self.editor_of(*profile) == Some(self.shell))
            .map(|profile| {
                let name = be::node(profile)
                    .and_then(|node| node.metadata.name)
                    .unwrap_or_else(|| "Profile".to_owned());
                (profile, name, current == Some(profile))
            })
            .collect();
        profiles.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        profiles
    }

    pub(crate) fn use_profile(&mut self, client: Uuid, profile: Uuid) {
        let Some(settings_block) = self.find() else {
            return;
        };
        be::operate_from(
            settings_block,
            be::next_origin(),
            SettingsContent::encode_operation(&Settings::use_profile(self.shell, client, profile)),
        );
        self.chosen_profile = Some(profile);
    }

    pub(crate) fn new_profile(&mut self, client: Uuid) {
        let Some(settings_block) = self.find() else {
            return;
        };
        let count = self
            .decoded(settings_block)
            .map_or(0, |settings| settings.profiles().len());
        let profile = Uuid::new_v4();
        be::create(
            profile,
            EditorViewContent::CONTENT_TYPE,
            BlockParent::Block(settings_block),
            BlockMetadata::named(format!("Profile {}", count + 1)),
            Some(self.session_document()),
        );
        be::operate_from(
            settings_block,
            be::next_origin(),
            SettingsContent::encode_operation(&Settings::add_profile(self.shell, client, profile)),
        );
        self.chosen_profile = Some(profile);
    }

    pub(crate) fn ensure_profile(&mut self, client: Uuid) -> Option<Uuid> {
        let settings_block = self.ensure()?;
        let settings = self.decoded(settings_block)?;
        if let Some(chosen) = self.chosen_profile {
            if settings.profile(self.shell, client) != Some(chosen) {
                return Some(chosen);
            }
            self.chosen_profile = None;
        }
        if let Some(profile) = settings.profile(self.shell, client) {
            self.created_profile = None;
            return Some(profile);
        }
        if self.created_profile.is_some() {
            return self.created_profile;
        }
        let profile = Uuid::new_v4();
        be::create(
            profile,
            EditorViewContent::CONTENT_TYPE,
            BlockParent::Block(settings_block),
            BlockMetadata::named(self.profile_name()),
            Some(self.session_document()),
        );
        be::operate_from(
            settings_block,
            be::next_origin(),
            SettingsContent::encode_operation(&Settings::add_profile(self.shell, client, profile)),
        );
        self.created_profile = Some(profile);
        Some(profile)
    }
}
