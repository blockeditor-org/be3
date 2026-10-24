use lucide::Glyph;
use block_plugin_api::{
    Catalog, ChildStatus, EditorInstanceId, EditorMessage, EditorRegion, Edits, FrameCommit,
    Message, ScreenId, ScreenLayout, ScreenRequest, SurfaceSpec, TemplateCategory,
};
use block_ui::{BlockCatalog, BlockTypeEntry, EditorEntry, TemplateEntry};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::{
    Waker,
    editor_session::{EditorSession, Open},
    host::{BlockDrag, HostContent},
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
    laid_out: HashSet<EditorInstanceId>,
    serial: u64,
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
            laid_out: HashSet::new(),
            serial: 0,
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
        self.laid_out.insert(instance);
    }

    pub(crate) fn commit(&mut self) -> FrameCommit {
        let mut commit = FrameCommit {
            serial: self.serial,
            sizes: Vec::new(),
            reports: Vec::new(),
            windows: Vec::new(),
            children: Vec::new(),
            repaint_after_micros: None,
            animating: false,
            presented: None,
        };
        let mut laid_out: Vec<_> = self.laid_out.drain().collect();
        laid_out.sort_by_key(|instance| instance.0);
        for instance in laid_out {
            if let Some(session) = self.sessions.get_mut(&instance) {
                session.commit(&mut commit);
            }
        }
        commit
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

    fn open(&mut self, instance: EditorInstanceId, editor: Uuid) -> &mut EditorSession {
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
                .find(|(declared, _)| *declared == editor)
                .map(|(_, open)| *open)
                .unwrap_or_else(|| panic!("this plugin has no editor {editor}"));
            EditorSession::new(instance, waker, open)
        })
    }

    pub(crate) fn paintings(&mut self) -> Message {
        let mut paintings = paint_format::ScreenPaintings::default();
        for placement in self.layout.screens.clone() {
            let Some(session) = self.sessions.get_mut(&placement.instance) else {
                continue;
            };
            let overflows = session.overflows(placement.region);
            if !overflows.is_empty() {
                paintings.overflows.push((placement.screen.0, overflows));
            }
            if let Some(painting) = session.painting(placement.region) {
                paintings.screens.push((placement.screen.0, painting));
                if paintings.encode().len() > block_plugin_api::MAX_BLOB_BYTES {
                    paintings.screens.pop();
                }
            }
        }
        Message::Extension {
            key: paint_format::PAINTING_EXTENSION,
            payload: paintings.encode(),
        }
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn surface(&self) -> Option<SurfaceSpec> {
        self.surface
    }

    pub(crate) fn receive(&mut self, message: &Message) -> bool {
        match message {
            Message::HelloAccepted(accepted) => {
                self.surface = accepted.surface;
                crate::preferences::receive(accepted.preferences);
            }
            Message::Preferences(preferences) => crate::preferences::receive(*preferences),
            Message::Fonts(fonts) => crate::fonts::receive(fonts),
            Message::UtcOffset(seconds) => crate::clock::receive(*seconds),
            Message::Describe(on) => DESCRIBE.store(*on, std::sync::atomic::Ordering::Relaxed),
            Message::Configure { serial } => {
                self.serial = *serial;
                return false;
            }
            Message::DrawFrame { now_micros, seen } => {
                crate::clock::set_frame_time(std::time::Duration::from_micros(*now_micros));
                SEEN.store(*seen, std::sync::atomic::Ordering::Relaxed);
                return false;
            }
            Message::Editor(EditorMessage::Open {
                instance,
                editor,
                block_id,
                block_type,
                view_block,
                account_id,
                organization_id,
                client_id,
                editable,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*editor));
                session.set_block_types(block_types);
                session.set_view_block(view_block.map(Uuid::from_bytes));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_organization_id(Uuid::from_bytes(*organization_id));
                session.set_editable(*editable);
                session.connect(Uuid::from_bytes(*block_id), Uuid::from_bytes(*block_type));
            }
            Message::Editor(EditorMessage::OpenCreation {
                instance,
                editor,
                template,
                account_id,
                organization_id,
                client_id,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*editor));
                session.set_block_types(block_types);
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_organization_id(Uuid::from_bytes(*organization_id));
                session.connect_creation(template.clone());
            }
            Message::Editor(EditorMessage::OpenArtifact {
                instance,
                editor,
                block_id,
                block_type,
                account_id,
                organization_id,
                client_id,
                data,
            }) => {
                let block_types = Rc::clone(&self.block_types);
                let session = self.open(*instance, Uuid::from_bytes(*editor));
                session.set_block_types(block_types);
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_organization_id(Uuid::from_bytes(*organization_id));
                session.connect_artifact(
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*block_type),
                    data.clone(),
                );
            }
            Message::Editor(EditorMessage::Resized { instance, size }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.resized(geometry::vec2(size.width, size.height));
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
                session: state,
                unsettled,
                applied,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.set_block_content(
                    Uuid::from_bytes(*block_id),
                    HostContent {
                        content_type: Uuid::from_bytes(*content_type),
                        bytes: bytes.clone(),
                        session: state.clone(),
                        unsettled: unsettled
                            .iter()
                            .map(|operation| operation.to_vec())
                            .collect(),
                        applied: *applied,
                    },
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
            Message::Editor(EditorMessage::BlocksFailed {
                instance,
                query,
                error,
            }) => {
                let Some(session) = self.sessions.get(instance) else {
                    return false;
                };
                session.fail_blocks(crate::BlockQuery::decode(*query), error.clone());
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
                session.push_content_operations(Uuid::from_bytes(*block_id), operations.clone());
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
                editor,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.show_block(crate::ShowRequest {
                        block_id: Uuid::from_bytes(*block_id),
                        block_type: Uuid::from_bytes(*block_type),
                        via: via.map(Uuid::from_bytes),
                        editor: editor.map(Uuid::from_bytes),
                    });
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
            Message::Editor(EditorMessage::ChildFocused {
                instance, child, ..
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.child_focused(*child);
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
                view,
                scale,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_view(*view, *scale);
                }
            }
            Message::Editor(EditorMessage::PickRequested {
                instance,
                pick,
                filter,
                parent,
                within,
                touch,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.pick_requested(
                        *pick,
                        filter.clone(),
                        crate::graph::BlockParent::decode(*parent),
                        within.map(Uuid::from_bytes),
                        *touch,
                    );
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
            Message::Editor(EditorMessage::HostValue {
                instance,
                key,
                value,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.host_value(key.clone(), value.clone());
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
            Message::NodeAction { screen, request } => {
                let Some((instance, region)) = self.screen(*screen) else {
                    return false;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return false;
                };
                session.node_action(region, request.clone());
            }
            Message::RunAction { screen, id } => {
                let Some((instance, region)) = self.screen(*screen) else {
                    return false;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return false;
                };
                session.run_action(region, id);
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
                pos,
                block_id,
                block_type,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(Some((
                        *region,
                        BlockDrag {
                            position: *pos,
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
                pos,
                files,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_files(Some((
                        *region,
                        crate::host::FileDrop {
                            position: *pos,
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
            Message::RunAction { screen, .. } | Message::NodeAction { screen, .. } => {
                match self.screen(*screen) {
                    Some((instance, _)) => self.mark(instance),
                    None => return false,
                }
            }
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
    BlockCatalog::new(catalog.types.iter().map(|descriptor| {
        (
            Uuid::from_bytes(descriptor.block_type),
            BlockTypeEntry {
                display_name: descriptor.display_name.clone(),
                color: descriptor.color,
                icon: Glyph::from_codepoint(&descriptor.icon_codepoint),
                child_edits: block_ui::ChildEdits {
                    add: descriptor.children.add,
                    delete: descriptor.children.delete,
                    replace: descriptor.children.replace,
                },
            },
        )
    }))
    .with_templates(catalog.templates.iter().map(|template| {
        TemplateEntry {
            color: catalog
                .types
                .iter()
                .find(|kind| kind.block_type == template.block_type)
                .and_then(|kind| kind.color),
            editor: Uuid::from_bytes(template.editor),
            template: template.template.clone(),
            block_type: Uuid::from_bytes(template.block_type),
            name: template.name.clone(),
            icon: Glyph::from_codepoint(&template.icon_codepoint),
            category: match template.category {
                TemplateCategory::Important => block_ui::TemplateCategory::Important,
                TemplateCategory::Regular => block_ui::TemplateCategory::Regular,
                TemplateCategory::Debug => block_ui::TemplateCategory::Debug,
                TemplateCategory::Template => block_ui::TemplateCategory::Template,
            },
            dialog: template.dialog,
        }
    }))
    .with_editors(catalog.editors.iter().map(|editor| EditorEntry {
        editor: Uuid::from_bytes(editor.editor),
        edits: match editor.edits {
            Edits::Type(block_type) => block_ui::Edits::Type(Uuid::from_bytes(block_type)),
            Edits::Any => block_ui::Edits::Any,
            Edits::Nothing => block_ui::Edits::Nothing,
        },
        name: editor.name.clone(),
        icon: Glyph::from_codepoint(&editor.icon_codepoint),
    }))
}

static DESCRIBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn describing() -> bool {
    DESCRIBE.load(std::sync::atomic::Ordering::Relaxed)
}

static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub(crate) fn seen() -> bool {
    SEEN.load(std::sync::atomic::Ordering::Relaxed)
}
