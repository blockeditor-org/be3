use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{
    Memo, ReadSignal, Stored, WriteSignal, create_effect, create_signal, store, untrack,
};
use beui::unstyled::TextAreaState;
use beui::{Rect, Vec2};
use block_editor_beui::BlockQuery;
use block_editor_beui::be_block::presence::pick_free_color;
use block_editor_beui::be_block::{ImageContent, TextContent};
use block_editor_beui::{BlockList, Blocks};
use block_editor_beui::{ChildState, ContentProjection, Editor, EditorHost, LinkBase, Projected};
use text_editor_core::{EditorCommand, TextLanguage};
use uuid::Uuid;

use crate::document::{BlockDocument, History, inside_block_url};
use crate::presence::TextCursor;

use super::embeds::{ResolvedEmbed, image_embed_directive, resolve_embeds};

pub(crate) const DIRECT_EDITOR_WIDTH: f32 = 600.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FocusedEmbed {
    pub id: Uuid,
    pub source_start: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct State {
    pub editor: Editor,
    pub block_id: Uuid,
    pub workspace_id: Uuid,
    link_base: Memo<String>,
    pub text: TextAreaState,
    pub embed_sizes: ReadSignal<HashMap<Uuid, Vec2>>,
    pub set_embed_sizes: WriteSignal<HashMap<Uuid, Vec2>>,
    pub focused_embed: ReadSignal<Option<FocusedEmbed>>,
    pub set_focused_embed: WriteSignal<Option<FocusedEmbed>>,

    pub embeds: ReadSignal<Vec<ResolvedEmbed>>,
    pub set_embeds: WriteSignal<Vec<ResolvedEmbed>>,
    pub hex_view: ReadSignal<bool>,
    pub set_hex_view: WriteSignal<bool>,
    pub typing: ReadSignal<bool>,
    pub set_typing: WriteSignal<bool>,
    pub hex_insert_mode: ReadSignal<bool>,
    pub set_hex_insert_mode: WriteSignal<bool>,
    pub import_error: ReadSignal<Option<String>>,
    pub set_import_error: WriteSignal<Option<String>>,
    pub presence_revision: ReadSignal<u64>,
    set_presence_revision: WriteSignal<u64>,
    pub peers: ReadSignal<Vec<(u64, TextCursor)>>,
    inner: Stored<Inner>,
}

pub(crate) struct Inner {
    pub client: Blocks,
    content: Rc<ContentProjection<TextContent>>,
    adopted: Cell<bool>,
    pub document: Arc<BlockDocument>,
    pub dependencies: BlockList,
    pub embed_children: RefCell<HashMap<FocusedEmbed, ChildState>>,
    published_cursor: Cell<Option<TextCursor>>,
    pub focus_confirmed: Cell<bool>,
    pub hex_pending_nibble: Cell<Option<u8>>,
    pub hex_selection_anchor: Cell<Option<usize>>,
    presence_counter: Cell<u64>,
    remote_cursors: RefCell<Vec<(u64, TextCursor)>>,
}

impl State {
    pub fn new(editor: Editor) -> Self {
        let client = editor.blocks();
        let block_id = editor.block_id();
        let content = editor.block_content::<TextContent>();
        content.record();
        let document = Arc::new(BlockDocument::new(editor.host().waker()));
        let text = TextAreaState::new(Arc::clone(&document) as Arc<dyn text_editor_core::Document>);
        text.with_core_mut(|core| core.config.inside_atomic_unit = inside_block_url);
        let dependencies = client.watch(BlockQuery::References(block_id));
        let (embeds, set_embeds) = create_signal(Vec::new());
        let (hex_view, set_hex_view) = create_signal(false);
        let (typing, set_typing) = create_signal(false);
        let (hex_insert_mode, set_hex_insert_mode) = create_signal(false);
        let (import_error, set_import_error) = create_signal(None);
        let (presence_revision, set_presence_revision) = create_signal(0);
        let (focused_embed, set_focused_embed) = create_signal(None);
        let (embed_sizes, set_embed_sizes) = create_signal(HashMap::new());
        let state = Self {
            workspace_id: editor.host().workspace_id(),
            link_base: editor.host_value::<LinkBase>(),
            peers: editor.peers::<TextCursor>(),
            editor,
            block_id,
            text,
            embed_sizes,
            set_embed_sizes,
            focused_embed,
            set_focused_embed,
            embeds,
            set_embeds,
            hex_view,
            set_hex_view,
            typing,
            set_typing,
            hex_insert_mode,
            set_hex_insert_mode,
            import_error,
            set_import_error,
            presence_revision,
            set_presence_revision,
            inner: store(Inner {
                client,
                content,
                adopted: Cell::new(false),
                document,
                dependencies,
                embed_children: RefCell::new(HashMap::new()),
                published_cursor: Cell::new(None),
                focus_confirmed: Cell::new(false),
                hex_pending_nibble: Cell::new(None),
                hex_selection_anchor: Cell::new(None),
                presence_counter: Cell::new(0),
                remote_cursors: RefCell::new(Vec::new()),
            }),
        };
        let edited = state.text.content();
        create_effect(move || {
            edited.get();
            let Some(inner) = state.inner.try_get() else {
                return;
            };
            inner.content.revision();
            untrack(|| state.pump());
        });
        state
    }

    pub fn inner(&self) -> Rc<Inner> {
        self.inner.get()
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    pub fn host(&self) -> EditorHost {
        self.editor.host()
    }

    pub fn pump(&self) {
        let inner = self.inner();
        let operations = inner.document.take_operations();
        let own = !operations.is_empty();
        let refused = own && !self.host().editable();
        for operation in operations {
            inner.content.operate(operation);
        }
        match inner.document.take_history() {
            Some(History::Undo) => self.host().undo(self.block_id),
            Some(History::Redo) => self.host().redo(self.block_id),
            None => {}
        }
        if inner.content.revision().is_none() {
            return;
        }
        let projected = inner.content.take_projected();
        let first = !inner.adopted.replace(true);
        let mut whole = first
            || refused
            || (own
                && projected
                    .iter()
                    .any(|projected| matches!(projected, Projected::Applied(_))));
        if !whole {
            for projected in &projected {
                let applied = match projected {
                    Projected::Applied(edit) => inner.document.apply(edit),
                    Projected::Rebuilt => false,
                };
                if !applied {
                    whole = true;
                    break;
                }
            }
        }
        if whole {
            inner.content.read(|content| inner.document.adopt(content));
        }
        if inner.document.take_external_edit() {
            self.text.external_edit();
        } else {
            self.text.sync();
        }
        if first {
            let start = self.text.with_core(|core| core.position(0));
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
        let inner = self.inner();
        let next = !self.hex_view.get_untracked();
        self.set_hex_view.set(next);
        inner.hex_pending_nibble.set(None);
        inner.hex_selection_anchor.set(None);
        self.set_focused_embed.set(None);
        inner.focus_confirmed.set(false);
        if next {
            self.text.close_find();
        }
    }

    pub fn create_image_block(&self, image: &ImageContent) -> Uuid {
        self.editor.create_child(image)
    }

    pub fn insert_image_embed(&self, id: Uuid, source_name: &str) {
        let markdown = self.text.language() == TextLanguage::Markdown;
        let link_base = self.link_base.get_untracked();
        let directive =
            image_embed_directive(&link_base, self.workspace_id, &id, source_name, markdown);
        self.text
            .execute(EditorCommand::InsertText(directive.as_bytes()));
        self.text.reveal_cursor();
    }

    pub fn remote_cursors(&self) -> Vec<(u64, TextCursor)> {
        let inner = self.inner();
        inner.remote_cursors.borrow().clone()
    }

    pub fn poll_presence(&self, visible: bool) {
        let inner = self.inner();
        if !visible {
            if inner.published_cursor.take().is_some() {
                self.editor.show::<TextCursor>(None);
            }
            if !inner.remote_cursors.borrow().is_empty() {
                inner.remote_cursors.borrow_mut().clear();
                self.bump_presence();
            }
            return;
        }
        if let Some(cursor) = self
            .text
            .with_core(|core| core.cursor_positions().first().copied())
        {
            let color = inner.published_cursor.get().map_or_else(
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
            if inner.published_cursor.get() != Some(published) {
                inner.published_cursor.set(Some(published));
                self.editor.show(Some(&published));
            }
        }
        let mut remote = self.peers.get_untracked();
        remote.sort_by_key(|(client_id, _)| *client_id);
        if *inner.remote_cursors.borrow() != remote {
            *inner.remote_cursors.borrow_mut() = remote;
            self.bump_presence();
        }
    }

    fn bump_presence(&self) {
        let inner = self.inner();
        inner.presence_counter.set(inner.presence_counter.get() + 1);
        self.set_presence_revision.set(inner.presence_counter.get());
    }

    pub fn presence_cursor_rect(&self, client_id: u64) -> Option<Rect> {
        let cursor = self
            .inner()
            .remote_cursors
            .borrow()
            .iter()
            .find(|(id, _)| *id == client_id)
            .map(|(_, cursor)| *cursor)?;
        let focus = self
            .text
            .with_core(|core| core.position_index(cursor.focus))?;
        self.text.layout().get_untracked().caret_rect(focus)
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let inner = self.inner();
        let ranges = inner.document.reference_ranges(old);
        self.text.execute(EditorCommand::ReplaceRanges {
            ranges: &ranges,
            replacement: new.to_string().as_bytes(),
        });
        self.pump();
        true
    }
}
