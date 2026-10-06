use be_block::settings::Settings;
use be_block::{
    BlockContent, BlockMetadata, EditorView, EditorViewContent, LiveEdit, Root, SettingsContent,
    WORKSPACE_EDITOR,
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

#[derive(Default)]
pub(crate) struct RootSettings {
    desktop: bool,
    block: Option<Uuid>,
    created_profile: Option<Uuid>,
    chosen_profile: Option<Uuid>,
    decoded: std::cell::RefCell<Option<(u64, Settings)>>,
}

impl RootSettings {
    pub(crate) fn new(desktop: bool) -> Self {
        Self {
            desktop,
            ..Self::default()
        }
    }

    fn session_document(&self) -> Vec<u8> {
        EditorView::session_document(WORKSPACE_EDITOR, self.desktop).encode()
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
        let current = self
            .chosen_profile
            .or(settings.profile(client, self.desktop));
        let mut profiles: Vec<(Uuid, String, bool)> = settings
            .profiles()
            .into_iter()
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
            SettingsContent::encode_operation(&Settings::use_profile(
                client,
                self.desktop,
                profile,
            )),
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
            SettingsContent::encode_operation(&Settings::add_profile(
                client,
                self.desktop,
                profile,
            )),
        );
        self.chosen_profile = Some(profile);
    }

    pub(crate) fn ensure_profile(&mut self, client: Uuid) -> Option<Uuid> {
        let settings_block = self.ensure()?;
        let settings = self.decoded(settings_block)?;
        if let Some(chosen) = self.chosen_profile {
            if settings.profile(client, self.desktop) != Some(chosen) {
                return Some(chosen);
            }
            self.chosen_profile = None;
        }
        if let Some(profile) = settings.profile(client, self.desktop) {
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
            BlockMetadata::named(match self.desktop {
                true => "Desktop",
                false => device_name(),
            }),
            Some(self.session_document()),
        );
        be::operate_from(
            settings_block,
            be::next_origin(),
            SettingsContent::encode_operation(&Settings::add_profile(
                client,
                self.desktop,
                profile,
            )),
        );
        self.created_profile = Some(profile);
        Some(profile)
    }
}
