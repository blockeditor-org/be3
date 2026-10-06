use block_plugin_api::{
    Catalog, ChildStatus, EditorInstanceId, EditorMessage, EditorRegion,
    Message, ScreenId, ScreenLayout, ScreenRequest, SurfaceSpec, TemplateCategory,
};
use block_ui::{BlockCatalog, BlockTypeEntry, TemplateEntry};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::{
    Waker,
    editor_session::{EditorSession, Open},
    host::BlockDrag,
};

pub(crate) type Opener = Open;

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EditorSession>,
    apps: Vec<(Uuid, Opener)>,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    surface: Option<SurfaceSpec>,
    next_surface: u32,
    dirty: Arc<Mutex<HashSet<EditorInstanceId>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    everything: bool,
    reporting: HashSet<EditorInstanceId>,
    reporting_everything: bool,
}

#[cfg(target_arch = "wasm32")]
pub(crate) enum Dirty {
    Everything,
    Instances(HashSet<EditorInstanceId>),
}

#[cfg(target_arch = "wasm32")]
impl Dirty {
    pub(crate) fn contains(&self, instance: EditorInstanceId) -> bool {
        match self {
            Self::Everything => true,
            Self::Instances(instances) => instances.contains(&instance),
        }
    }
}

impl Screens {
    pub(crate) fn new(apps: Vec<(Uuid, Opener)>, waker: Waker) -> Self {
        Self {
            sessions: HashMap::new(),
            apps,
            waker,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
            surface: None,
            next_surface: 0,
            dirty: Arc::default(),
            everything: true,
            reporting: HashSet::new(),
            reporting_everything: true,
        }
    }

    pub(crate) fn adopt(&mut self, instance: EditorInstanceId, session: EditorSession) {
        self.sessions.insert(instance, session);
        self.touch_everything();
    }

    fn touch_everything(&mut self) {
        self.everything = true;
        self.reporting_everything = true;
    }

    pub(crate) fn ran(&mut self, instance: EditorInstanceId) {
        self.reporting.insert(instance);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_dirty(&mut self) -> Dirty {
        let dirty =
            std::mem::take(&mut *self.dirty.lock().unwrap_or_else(|held| held.into_inner()));
        match std::mem::take(&mut self.everything) {
            true => Dirty::Everything,
            false => Dirty::Instances(dirty),
        }
    }

    fn mark(&mut self, instance: EditorInstanceId) {
        if self.sessions.contains_key(&instance) {
            self.dirty
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .insert(instance);
            self.reporting.insert(instance);
        }
    }

    fn open(&mut self, instance: EditorInstanceId, block_type: Uuid) -> &mut EditorSession {
        let apps = &self.apps;
        let marks = Arc::clone(&self.dirty);
        let inner = self.waker.clone();
        let waker = Waker::default();
        waker.install(move || {
            marks
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .insert(instance);
            inner.wake();
        });
        self.sessions.entry(instance).or_insert_with(|| {
            let open = apps
                .iter()
                .find(|(declared, _)| *declared == block_type)
                .map(|(_, open)| *open)
                .unwrap_or_else(|| panic!("this plugin has no editor for block type {block_type}"));
            EditorSession::new(instance, waker, open)
        })
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn surface(&self) -> Option<SurfaceSpec> {
        self.surface
    }

    pub(crate) fn set_generation(&mut self, generation: u64) {
        self.layout.generation = generation;
    }

    pub(crate) fn receive(&mut self, message: &Message) -> bool {
        match message {
            Message::HelloAccepted(accepted) => {
                self.surface = accepted.surface;
            }
            Message::Fonts(fonts) => crate::fonts::receive(fonts),
            Message::Editor(EditorMessage::Open {
                instance,
                block_id,
                block_type,
                view_block,
                account_id,
                workspace_id,
                client_id,
                editable,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*block_type));
                session.set_block_types(block_types);
                session.set_view_block(view_block.map(Uuid::from_bytes));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_workspace_id(Uuid::from_bytes(*workspace_id));
                session.set_editable(*editable);
                session.connect(Uuid::from_bytes(*block_id), Uuid::from_bytes(*block_type));
            }
            Message::Editor(EditorMessage::OpenCreation {
                instance,
                block_type,
                template,
                account_id,
                workspace_id,
                client_id,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*block_type));
                session.set_block_types(block_types);
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_workspace_id(Uuid::from_bytes(*workspace_id));
                session.connect_creation(template.clone());
            }
            Message::Editor(EditorMessage::OpenArtifact {
                instance,
                source_type,
                block_id,
                block_type,
                account_id,
                workspace_id,
                client_id,
                data,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*source_type));
                session.set_block_types(block_types);
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_workspace_id(Uuid::from_bytes(*workspace_id));
                session.connect_artifact(
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*block_type),
                    data.clone(),
                );
            }
            Message::Editor(EditorMessage::Resized {
                instance,
                width,
                height,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.resized(geometry::vec2(*width, *height));
                }
            }
            Message::Editor(EditorMessage::AudioStatus { instance, status }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_audio(status.clone());
                }
            }
            Message::Editor(EditorMessage::ArtifactSettings { instance, data }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.artifact_settings(data.clone());
                }
            }
            Message::Editor(EditorMessage::RegenerateArtifact { instance, data }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.regenerate_artifact(data);
                }
            }
            Message::Editor(EditorMessage::CommitCreation { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.commit_creation();
                }
            }
            Message::Editor(EditorMessage::EditabilityChanged { instance, editable }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_editable(*editable);
                }
            }
            Message::Editor(EditorMessage::Content {
                instance,
                block_id,
                content_type,
                bytes,
                applied,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.set_block_content(
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*content_type),
                    bytes.clone(),
                    *applied,
                );
            }
            Message::Editor(EditorMessage::Blocks {
                instance,
                query,
                blocks,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.set_blocks(
                    crate::BlockQuery::decode(*query),
                    blocks
                        .iter()
                        .cloned()
                        .map(crate::BlockInfo::decode)
                        .collect(),
                );
            }
            Message::Editor(EditorMessage::PeerPresence {
                instance,
                block_id,
                peers,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.set_peers(
                    Uuid::from_bytes(*block_id),
                    peers
                        .iter()
                        .map(|peer| crate::PeerPresence {
                            client: peer.client,
                            kind: Uuid::from_bytes(peer.kind),
                            value: peer.value.clone(),
                        })
                        .collect(),
                );
            }
            Message::Editor(EditorMessage::ContentOperations {
                instance,
                block_id,
                operations,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.push_content_operations(
                    Uuid::from_bytes(*block_id),
                    operations
                        .iter()
                        .map(|operation| (operation.operation.clone(), operation.mine))
                        .collect(),
                );
            }
            Message::Editor(EditorMessage::FocusChanged {
                instance,
                block_id,
                block_type,
                via,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_focused_block(crate::host::FocusedBlock {
                        block_id: block_id.map(Uuid::from_bytes),
                        block_type: Uuid::from_bytes(*block_type),
                        via: via.iter().copied().map(Uuid::from_bytes).collect(),
                    });
                }
            }
            Message::Editor(EditorMessage::ShowBlock {
                instance,
                block_id,
                block_type,
                via,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.show_block(
                        Uuid::from_bytes(*block_id),
                        Uuid::from_bytes(*block_type),
                        via.map(Uuid::from_bytes),
                    );
                }
            }
            Message::Editor(EditorMessage::HistoryStates { instance, states }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_histories(states);
                }
            }
            Message::Editor(EditorMessage::VersionStatus {
                instance, status, ..
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_version_status(status);
                }
            }
            Message::Editor(EditorMessage::ArtifactStates { instance, states }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_artifacts(
                        states
                            .iter()
                            .map(|state| crate::host::ArtifactState {
                                block_id: Uuid::from_bytes(state.block_id),
                                source_type: Uuid::from_bytes(state.source_type),
                                source: state.source.map(Uuid::from_bytes),
                                summary: state.summary.clone(),
                                error: state.error.clone(),
                                regenerating: state.regenerating,
                            })
                            .collect(),
                    );
                }
            }
            Message::Editor(EditorMessage::PresentingChanged {
                instance,
                presenting,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_presenting(*presenting);
                }
            }
            Message::Editor(EditorMessage::Presence { instance, visible }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.presence_visible(*visible);
                }
            }
            Message::Editor(EditorMessage::ChildView {
                instance,
                child,
                change,
                ..
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.child_view_change(*child, *change);
                }
            }
            Message::Editor(EditorMessage::ChildBar {
                instance,
                child,
                action,
                ..
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.child_bar_action(*child, *action);
                }
            }
            Message::Editor(EditorMessage::ReplaceChild {
                instance,
                request_id,
                old,
                new,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.replace_child(
                        *request_id,
                        Uuid::from_bytes(*old),
                        Uuid::from_bytes(*new),
                    );
                }
            }
            Message::Editor(EditorMessage::ViewChanged {
                instance,
                x,
                y,
                width,
                height,
                scale,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_view(
                        geometry::Rect::from_min_size(
                            geometry::pos2(*x, *y),
                            geometry::vec2(*width, *height),
                        ),
                        *scale,
                    );
                }
            }
            Message::Editor(EditorMessage::PickRequested {
                instance,
                pick,
                filter,
                parent,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.pick_requested(crate::host::PickRequest {
                        pick: *pick,
                        filter: filter.clone(),
                        parent: crate::graph::BlockParent::decode(*parent),
                    });
                }
            }
            Message::Editor(EditorMessage::ShowDialog {
                instance,
                block_id,
                dialog,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.show_dialog(Uuid::from_bytes(*block_id), *dialog);
                }
            }
            Message::Editor(EditorMessage::ShowPanel { instance, panel }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.show_panel(*panel);
                }
            }
            Message::Editor(EditorMessage::Windows { instance, windows }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_windows(windows.clone());
                }
            }
            Message::Editor(EditorMessage::MenuPick { instance, id }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.pick_menu(id.clone());
                }
            }
            Message::Editor(EditorMessage::Close { instance }) => {
                self.sessions.remove(instance);
                self.requests
                    .retain(|request| request.instance != *instance);
                self.relayout();
            }
            Message::Screens(set) => {
                self.requests = set.screens.clone();
                self.relayout();
            }
            Message::Input(batch) => {
                let Some((instance, region)) = self.screen(batch.screen) else {
                    return false;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return false;
                };
                for event in &batch.events {
                    session.input(region, event);
                }
            }
            Message::BlockTypes(descriptors) => {
                self.block_types = Rc::new(catalog(descriptors));
                for session in self.sessions.values() {
                    session.set_block_types(Rc::clone(&self.block_types));
                }
            }
            Message::Editor(EditorMessage::DragOver {
                instance,
                region,
                x,
                y,
                block_id,
                block_type,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(Some((
                        *region,
                        BlockDrag {
                            position: geometry::pos2(*x, *y),
                            block_id: Uuid::from_bytes(*block_id),
                            block_type: Uuid::from_bytes(*block_type),
                            dropped: *dropped,
                        },
                    )));
                }
            }
            Message::Editor(EditorMessage::Replied {
                instance,
                request_id,
                reply,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.replied(*request_id, reply.clone());
                }
            }
            Message::Editor(EditorMessage::WebViewEvent {
                instance,
                web_view,
                event,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.web_view_event(*web_view, event.clone());
                }
            }
            Message::ChildStatuses(statuses) => {
                let mut grouped: HashMap<EditorInstanceId, Vec<ChildStatus>> = HashMap::new();
                for status in statuses {
                    grouped
                        .entry(status.instance)
                        .or_default()
                        .push(status.clone());
                }
                for (instance, statuses) in grouped {
                    if let Some(session) = self.sessions.get(&instance) {
                        session.set_child_statuses(statuses);
                    }
                }
            }
            Message::Editor(EditorMessage::DragLeft { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(None);
                }
            }
            Message::Editor(EditorMessage::FileDrop {
                instance,
                region,
                x,
                y,
                files,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_files(Some((
                        *region,
                        crate::host::FileDrop {
                            position: geometry::pos2(*x, *y),
                            files: files
                                .iter()
                                .map(|file| crate::PickedFile {
                                    name: file.name.clone(),
                                    data: file.data.clone(),
                                })
                                .collect(),
                            dropped: *dropped,
                        },
                    )));
                }
            }
            Message::Editor(EditorMessage::FileDropLeft { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_files(None);
                }
            }
            _ => return false,
        }
        match message {
            Message::Input(batch) => match self.screen(batch.screen) {
                Some((instance, _)) => self.mark(instance),
                None => return false,
            },
            Message::ChildStatuses(statuses) => {
                for status in statuses {
                    self.mark(status.instance);
                }
            }
            Message::Editor(EditorMessage::Close { .. }) | Message::Screens(_) => {
                self.touch_everything();
            }
            Message::Editor(editor) => {
                if !self.sessions.contains_key(&editor.instance()) {
                    return false;
                }
                self.mark(editor.instance());
            }
            _ => self.touch_everything(),
        }
        true
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::from_iter(crate::fonts::take_missing());
        let woken = self
            .dirty
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone();
        let everything = std::mem::take(&mut self.reporting_everything);
        let reporting = std::mem::take(&mut self.reporting);
        for (instance, session) in &mut self.sessions {
            if everything || reporting.contains(instance) || woken.contains(instance) {
                messages.extend(session.outbound());
            }
        }
        messages
    }

    pub(crate) fn get(&self, instance: EditorInstanceId) -> Option<&EditorSession> {
        self.sessions.get(&instance)
    }

    pub(crate) fn session(&mut self, instance: EditorInstanceId) -> Option<&mut EditorSession> {
        self.sessions.get_mut(&instance)
    }

    fn screen(&self, screen: ScreenId) -> Option<(EditorInstanceId, EditorRegion)> {
        self.layout
            .placement(screen)
            .map(|placement| (placement.instance, placement.region))
    }

    fn relayout(&mut self) {
        let previous = std::mem::take(&mut self.layout);
        let next = &mut self.next_surface;
        self.layout = ScreenLayout::placed(&self.requests, |screen| {
            previous
                .placement(screen)
                .map(|placement| placement.surface)
                .unwrap_or_else(|| {
                    *next += 1;
                    *next
                })
        });
        self.layout.generation = previous.generation;
        let mut placements: HashMap<EditorInstanceId, Vec<_>> = HashMap::new();
        for placement in &self.layout.screens {
            placements
                .entry(placement.instance)
                .or_default()
                .push(*placement);
        }
        for (instance, session) in &mut self.sessions {
            session.place(
                placements.get(instance).map_or(&[], Vec::as_slice),
                &self.requests,
            );
        }
    }
}

fn catalog(catalog: &Catalog) -> BlockCatalog {
    let leak = |codepoint: &str| -> Option<&'static str> {
        let codepoint: &'static str = Box::leak(codepoint.to_owned().into_boxed_str());
        (!codepoint.is_empty()).then_some(codepoint)
    };
    BlockCatalog::new(catalog.types.iter().map(|descriptor| {
        (
            Uuid::from_bytes(descriptor.block_type),
            BlockTypeEntry {
                display_name: descriptor.display_name.clone(),
                icon: leak(&descriptor.icon_codepoint),
                child_edits: block_ui::ChildEdits {
                    add: descriptor.children.add,
                    delete: descriptor.children.delete,
                    replace: descriptor.children.replace,
                },
            },
        )
    }))
    .with_templates(catalog.templates.iter().map(|template| TemplateEntry {
        editor: Uuid::from_bytes(template.editor),
        template: template.template.clone(),
        block_type: Uuid::from_bytes(template.block_type),
        name: template.name.clone(),
        icon: leak(&template.icon_codepoint),
        category: match template.category {
            TemplateCategory::Important => block_ui::TemplateCategory::Important,
            TemplateCategory::Regular => block_ui::TemplateCategory::Regular,
            TemplateCategory::Debug => block_ui::TemplateCategory::Debug,
            TemplateCategory::Template => block_ui::TemplateCategory::Template,
        },
        dialog: template.dialog,
    }))
}
