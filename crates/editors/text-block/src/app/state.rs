use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::*;
use beui::styled::TextAreaState;
use beui::{Rect, Vec2};
use block_editor_beui::BlockQuery;
use block_editor_beui::SharedCaret;
use block_editor_beui::be_block::presence::pick_free_color;
use block_editor_beui::live_text::Synced;
use block_editor_beui::{BlockList, Blocks};
use block_editor_beui::{ChildState, ContentProjection, Editor, EditorHost, LinkBaseWatch};
use text_editor_core::{EditorCommand, TextLanguage};
use uuid::Uuid;

use crate::document::{BlockDocument, TextBody, inside_block_url, reference_ranges};
use crate::presence::TextCursor;

use super::embeds::{ResolvedEmbed, image_embed_directive, resolve_embeds};
use image_block_types::ImageContent;
use text_block_types::TextContent;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FocusedEmbed {
    pub id: Uuid,
    pub source_start: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct State {
    pub editor: Editor,
    pub block_id: Uuid,
    pub organization_id: Uuid,
    link_base: Memo<String>,
    pub text: TextAreaState,
    pub embed_sizes: Signal<HashMap<Uuid, Vec2>>,
    pub focused_embed: Signal<Option<FocusedEmbed>>,

    pub embeds: Signal<Vec<ResolvedEmbed>>,
    pub hex_view: Signal<bool>,
    pub typing: Signal<bool>,
    pub hex_insert_mode: Signal<bool>,
    pub import_error: Signal<Option<String>>,
    pub peers: ReadSignal<Vec<(u64, TextCursor)>>,
    pub remote_cursors: Memo<Vec<(u64, TextCursor)>>,
    inner: Stored<Inner>,
}

pub(crate) struct Inner {
    pub client: Blocks,
    content: ContentProjection<TextContent>,
    adopted: Cell<bool>,
    pub document: Arc<BlockDocument>,
    pub dependencies: BlockList,
    pub embed_children: RefCell<HashMap<FocusedEmbed, ChildState>>,
    published_cursor: Cell<Option<TextCursor>>,
    pub focus_confirmed: Cell<bool>,
    pub hex_pending_nibble: Cell<Option<u8>>,
    pub hex_selection_anchor: Cell<Option<usize>>,
}

impl State {
    pub fn new(editor: Editor) -> Self {
        let client = editor.blocks();
        let block_id = editor.block_id();
        let content = editor.block_content::<TextContent>();
        content.record();
        let document = Arc::new(BlockDocument::new(TextBody, editor.host().waker()));
        let text = TextAreaState::new(Arc::clone(&document) as Arc<dyn text_editor_core::Document>);
        text.with_core_mut(|core| core.config.inside_atomic_unit = inside_block_url);
        let dependencies = client.watch(BlockQuery::References(block_id));
        let embeds = create_signal(Vec::new());
        let hex_view = create_signal(false);
        let typing = create_signal(false);
        let hex_insert_mode = create_signal(false);
        let import_error = create_signal(None);
        let peers = editor.peers::<TextCursor>();
        let visible = editor.presence_visible();
        let remote_cursors = create_memo(move || {
            if !visible.get() {
                return Vec::new();
            }
            let mut remote = peers.get();
            remote.sort_by_key(|(client_id, _)| *client_id);
            remote
        });
        let focused_embed = create_signal(None);
        let embed_sizes = create_signal(HashMap::new());
        let state = Self {
            organization_id: editor.host().organization_id(),
            link_base: editor.watch::<LinkBaseWatch>(),
            peers,
            remote_cursors,
            editor,
            block_id,
            text,
            embed_sizes,
            focused_embed,
            embeds,
            hex_view,
            typing,
            hex_insert_mode,
            import_error,
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
            }),
        };
        let edited = state.text.content();
        create_effect(move || {
            edited.get();
            let Some(inner) = state.inner.try_value() else {
                return;
            };
            let _ = inner.content.revision();
            untrack(|| state.pump());
        });
        state
    }

    pub fn inner(&self) -> Rc<Inner> {
        self.inner.value()
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    pub fn host(&self) -> EditorHost {
        self.editor.host()
    }

    pub fn pump(&self) {
        let inner = self.inner();
        let first = !inner.adopted.get();
        let synced = inner
            .document
            .sync(&inner.content, &self.host(), self.block_id, first);
        match synced {
            Synced::Waiting => return,
            Synced::External => self.text.external_edit(),
            Synced::Unchanged => self.text.sync(),
        }
        inner.adopted.set(true);
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
            self.embeds.set(resolved);
        }
    }

    pub fn toggle_hex_view(&self) {
        let inner = self.inner();
        let next = !self.hex_view.get_untracked();
        self.hex_view.set(next);
        inner.hex_pending_nibble.set(None);
        inner.hex_selection_anchor.set(None);
        self.focused_embed.set(None);
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
            image_embed_directive(&link_base, self.organization_id, &id, source_name, markdown);
        self.text
            .execute(EditorCommand::InsertText(directive.as_bytes()));
        self.text.reveal_cursor();
    }

    pub fn publish_cursor(&self, visible: bool) {
        let inner = self.inner();
        if !visible {
            if inner.published_cursor.take().is_some() {
                self.editor.show::<TextCursor>(None);
            }
            return;
        }
        let Some(caret) = SharedCaret::of(self.text) else {
            return;
        };
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
        let published = TextCursor { caret, color };
        if inner.published_cursor.get() != Some(published) {
            inner.published_cursor.set(Some(published));
            self.editor.show(Some(&published));
        }
    }

    pub fn presence_cursor_rect(&self, client_id: u64) -> Option<Rect> {
        let cursor = self.remote_cursors.with_untracked(|remote| {
            remote
                .iter()
                .find(|(id, _)| *id == client_id)
                .map(|(_, cursor)| *cursor)
        })?;
        let focus = self
            .text
            .with_core(|core| core.position_index(cursor.caret.focus))?;
        self.text.layout().get_untracked().caret_rect(focus)
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let inner = self.inner();
        let ranges = inner
            .document
            .with_bytes(|bytes| reference_ranges(bytes, old));
        self.text.execute(EditorCommand::ReplaceRanges {
            ranges: &ranges,
            replacement: new.to_string().as_bytes(),
        });
        self.pump();
        true
    }
}
