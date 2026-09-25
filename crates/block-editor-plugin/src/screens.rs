use block_plugin_api::{
    BlockTypeDescriptor, ChildStatus, DEFAULT_SURFACE_SIDE, EditorInstanceId, EditorMessage,
    EditorRegion, Message, ScreenId, ScreenLayout, ScreenRequest, SurfaceSpec,
};
use block_ui::{BlockCatalog, BlockTypeEntry};
use std::{collections::HashMap, rc::Rc};
use uuid::Uuid;

use crate::{
    Waker,
    editor_session::{EditorSession, Open},
    host::BlockDrag,
};

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EditorSession>,
    open: Open,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    surface: Option<SurfaceSpec>,
}

impl Screens {
    pub(crate) fn new(open: Open, waker: Waker) -> Self {
        Self {
            sessions: HashMap::new(),
            open,
            waker,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
            surface: None,
        }
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn surface(&self) -> Option<SurfaceSpec> {
        self.surface
    }

    pub(crate) fn set_generation(&mut self, generation: u64) {
        self.layout.generation = generation;
    }

    pub(crate) fn receive(&mut self, message: &Message) -> bool {
        match message {
            Message::HelloAccepted(accepted) => self.surface = accepted.surface,
            Message::Editor(EditorMessage::Open {
                instance,
                block_id,
                block_type,
                account_id,
                workspace_id,
                client_id,
                editable,
            }) => {
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    EditorSession::new(*instance, self.waker.clone(), self.open)
                });
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_workspace_id(Uuid::from_bytes(*workspace_id));
                session.set_editable(*editable);
                session.connect(Uuid::from_bytes(*block_id), Uuid::from_bytes(*block_type));
            }
            Message::Editor(EditorMessage::OpenCreation {
                instance,
                account_id,
                workspace_id,
                client_id,
            }) => {
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    EditorSession::new(*instance, self.waker.clone(), self.open)
                });
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_account_id(Uuid::from_bytes(*account_id));
                session.set_workspace_id(Uuid::from_bytes(*workspace_id));
                session.connect_creation();
            }
            Message::Editor(EditorMessage::OpenArtifact {
                instance,
                block_id,
                block_type,
                account_id,
                workspace_id,
                client_id,
                data,
            }) => {
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    EditorSession::new(*instance, self.waker.clone(), self.open)
                });
                session.set_block_types(Rc::clone(&self.block_types));
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
            Message::Editor(EditorMessage::WebViewEvent { instance, event }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.web_view_event(event.clone());
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
        true
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for session in self.sessions.values_mut() {
            messages.extend(session.outbound());
        }
        messages
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
        let generation = self.layout.generation;
        let max_side = self
            .surface
            .map_or(DEFAULT_SURFACE_SIDE, |surface| surface.max_side);
        self.layout = ScreenLayout::packed(&self.requests, max_side);
        self.layout.generation = generation;
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

fn catalog(descriptors: &[BlockTypeDescriptor]) -> BlockCatalog {
    BlockCatalog::new(descriptors.iter().map(|descriptor| {
        let codepoint: &'static str = Box::leak(descriptor.icon_codepoint.clone().into_boxed_str());
        (
            Uuid::from_bytes(descriptor.block_type),
            BlockTypeEntry {
                display_name: descriptor.display_name.clone(),
                icon: (!codepoint.is_empty()).then_some(codepoint),
                child_edits: block_ui::ChildEdits {
                    add: descriptor.children.add,
                    delete: descriptor.children.delete,
                    replace: descriptor.children.replace,
                },
            },
        )
    }))
}
