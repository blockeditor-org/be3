use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{ReadSignal, WriteSignal, create_effect, create_signal, untrack};
use beui::unstyled::TextAreaState;
use beui::{Rect, Vec2};
use block_editor_plugin::BlockQuery;
use block_editor_plugin::be_block::presence::pick_free_color;
use block_editor_plugin::be_block::{ImageContent, TextContent};
use block_editor_plugin::{BlockList, Blocks};
use block_editor_plugin::{ChildState, ContentProjection, Editor, EditorHost, ImagePaster};
use text_editor_core::{EditorCommand, TextLanguage};
use uuid::Uuid;

use crate::document::{BlockDocument, inside_block_url};
use crate::presence::TextCursor;

use super::embeds::{ResolvedEmbed, image_embed_directive, resolve_embeds};

pub(crate) const DIRECT_EDITOR_WIDTH: f32 = 600.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FocusedEmbed {
    pub id: Uuid,
    pub source_start: usize,
}

pub(crate) struct State {
    pub editor: Editor,
    pub client: Blocks,
    pub block_id: Uuid,
    content: Rc<ContentProjection<TextContent>>,
    adopted: Cell<Option<u64>>,
    pub document: Arc<BlockDocument>,
    pub workspace_id: Uuid,
    pub text: TextAreaState,
    pub dependencies: BlockList,
    pub paster: RefCell<ImagePaster>,
    pub embed_sizes: ReadSignal<HashMap<Uuid, Vec2>>,
    pub set_embed_sizes: WriteSignal<HashMap<Uuid, Vec2>>,
    pub embed_children: RefCell<HashMap<FocusedEmbed, ChildState>>,
    pub published_cursor: Cell<Option<TextCursor>>,
    pub focused_embed: ReadSignal<Option<FocusedEmbed>>,
    pub set_focused_embed: WriteSignal<Option<FocusedEmbed>>,
    pub focus_confirmed: Cell<bool>,

    pub embeds: ReadSignal<Vec<ResolvedEmbed>>,
    pub set_embeds: WriteSignal<Vec<ResolvedEmbed>>,
    pub hex_view: ReadSignal<bool>,
    pub set_hex_view: WriteSignal<bool>,
    pub hex_insert_mode: ReadSignal<bool>,
    pub set_hex_insert_mode: WriteSignal<bool>,
    pub hex_pending_nibble: Cell<Option<u8>>,
    pub hex_selection_anchor: Cell<Option<usize>>,
    pub import_error: ReadSignal<Option<String>>,
    pub set_import_error: WriteSignal<Option<String>>,
    pub presence_revision: ReadSignal<u64>,
    set_presence_revision: WriteSignal<u64>,
    presence_counter: Cell<u64>,
    pub peers: ReadSignal<Vec<(u64, TextCursor)>>,
    remote_cursors: RefCell<Vec<(u64, TextCursor)>>,
}

pub(crate) type Shared = Rc<State>;

impl State {
    pub fn new(editor: Editor) -> Shared {
        let client = editor.blocks();
        let block_id = editor.block_id();
        let content = editor.block_content::<TextContent>();
        let document = Arc::new(BlockDocument::new(editor.host().waker()));
        let text = TextAreaState::new(Arc::clone(&document) as Arc<dyn text_editor_core::Document>);
        text.core_mut().config.inside_atomic_unit = inside_block_url;
        let dependencies = client.watch(BlockQuery::References(block_id));
        let (embeds, set_embeds) = create_signal(Vec::new());
        let (hex_view, set_hex_view) = create_signal(false);
        let (hex_insert_mode, set_hex_insert_mode) = create_signal(false);
        let (import_error, set_import_error) = create_signal(None);
        let (presence_revision, set_presence_revision) = create_signal(0);
        let (focused_embed, set_focused_embed) = create_signal(None);
        let (embed_sizes, set_embed_sizes) = create_signal(HashMap::new());
        let state = Rc::new(Self {
            workspace_id: editor.host().workspace_id(),
            peers: editor.peers::<TextCursor>(),
            editor,
            client,
            block_id,
            content,
            adopted: Cell::new(None),
            document,
            text,
            dependencies,
            paster: RefCell::new(ImagePaster::default()),
            embed_sizes,
            set_embed_sizes,
            embed_children: RefCell::new(HashMap::new()),
            published_cursor: Cell::new(None),
            focused_embed,
            set_focused_embed,
            focus_confirmed: Cell::new(false),
            embeds,
            set_embeds,
            hex_view,
            set_hex_view,
            hex_insert_mode,
            set_hex_insert_mode,
            hex_pending_nibble: Cell::new(None),
            hex_selection_anchor: Cell::new(None),
            import_error,
            set_import_error,
            presence_revision,
            set_presence_revision,
            presence_counter: Cell::new(0),
            remote_cursors: RefCell::new(Vec::new()),
        });
        let pumped = Rc::downgrade(&state);
        let edited = state.text.content();
        create_effect(move || {
            edited.get();
            let Some(state) = pumped.upgrade() else {
                return;
            };
            state.content.revision();
            untrack(|| state.pump());
        });
        state
    }

    pub fn host(&self) -> &EditorHost {
        self.editor.host()
    }

    pub fn pump(&self) {
        for operation in self.document.take_operations() {
            self.content.operate(operation);
        }
        let revision = self.content.revision();
        if revision.is_none() || revision == self.adopted.get() {
            return;
        }
        let first = self.adopted.replace(revision).is_none();
        self.content.read(|content| self.document.adopt(content));
        if self.document.take_external_edit() {
            self.text.external_edit();
        }
        if first {
            let start = self.text.core().position(0);
            self.text.execute(EditorCommand::SetSelection {
                anchor: start,
                focus: start,
            });
        }
    }

    pub fn refresh_embeds(&self) {
        let resolved = resolve_embeds(self);
        if self.embeds.get_untracked() != resolved {
            self.set_embeds.set(resolved);
        }
    }

    pub fn toggle_hex_view(&self) {
        let next = !self.hex_view.get_untracked();
        self.set_hex_view.set(next);
        self.hex_pending_nibble.set(None);
        self.hex_selection_anchor.set(None);
        self.set_focused_embed.set(None);
        self.focus_confirmed.set(false);
        if next {
            self.text.close_find();
        }
    }

    pub fn create_image_block(&self, image: &ImageContent) -> Uuid {
        self.editor.create_child(image)
    }

    pub fn insert_image_embed(&self, id: Uuid, source_name: &str) {
        let markdown = self.text.language() == TextLanguage::Markdown;
        let directive = image_embed_directive(self.workspace_id, &id, source_name, markdown);
        self.text
            .execute(EditorCommand::InsertText(directive.as_bytes()));
        self.text.reveal_cursor();
    }

    pub fn remote_cursors(&self) -> Vec<(u64, TextCursor)> {
        self.remote_cursors.borrow().clone()
    }

    pub fn poll_presence(&self, visible: bool) {
        if !visible {
            if self.published_cursor.take().is_some() {
                self.editor.show::<TextCursor>(None);
            }
            if !self.remote_cursors.borrow().is_empty() {
                self.remote_cursors.borrow_mut().clear();
                self.bump_presence();
            }
            return;
        }
        if let Some(cursor) = self.text.core().cursor_positions().first() {
            let color = self.published_cursor.get().map_or_else(
                || {
                    pick_free_color(
                        self.peers
                            .get_untracked()
                            .iter()
                            .map(|(_, peer)| peer.color),
                    )
                },
                |published| published.color,
            );
            let published = TextCursor {
                anchor: cursor.pos.anchor,
                focus: cursor.pos.focus,
                color,
            };
            if self.published_cursor.get() != Some(published) {
                self.published_cursor.set(Some(published));
                self.editor.show(Some(&published));
            }
        }
        let mut remote = self.peers.get_untracked();
        remote.sort_by_key(|(client_id, _)| *client_id);
        if *self.remote_cursors.borrow() != remote {
            *self.remote_cursors.borrow_mut() = remote;
            self.bump_presence();
        }
    }

    fn bump_presence(&self) {
        self.presence_counter.set(self.presence_counter.get() + 1);
        self.set_presence_revision.set(self.presence_counter.get());
    }

    pub fn presence_cursor_rect(&self, client_id: u64) -> Option<Rect> {
        let cursor = self
            .remote_cursors
            .borrow()
            .iter()
            .find(|(id, _)| *id == client_id)
            .map(|(_, cursor)| *cursor)?;
        let focus = self.text.core().position_index(cursor.focus)?;
        self.text.layout().get_untracked().caret_rect(focus)
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let ranges = self.document.reference_ranges(old);
        self.text.execute(EditorCommand::ReplaceRanges {
            ranges: &ranges,
            replacement: new.to_string().as_bytes(),
        });
        self.pump();
        true
    }
}
