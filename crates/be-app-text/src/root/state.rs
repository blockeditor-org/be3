use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use be_sdk_beui::BlockQuery;
use be_sdk_beui::be_block_content::ViewStateKind;
use be_sdk_beui::be_block_content::presence::pick_free_color;
use be_sdk_beui::{BlockList, Blocks};
use be_sdk_beui::{ChildState, ContentProjection, Editor, EditorHost, LinkBaseWatch};
use be_sdk_text::SharedCaret;
use be_sdk_text::live_text::Synced;
use beui::reactive::*;
use beui::styled::TextAreaState;
use beui::{Rect, Vec2};
use beui_text_editor_core::{EditorCommand, TextLanguage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::document::{BlockDocument, TextBody, inside_block_url};
use crate::presence::TextCursor;

use super::embeds::{ResolvedEmbed, image_embed_directive, resolve_embeds};
use be_type_image::ImageContent;
use be_type_text::TextContent;

/// Whether the gutter of line numbers is shown, kept in the view's state.
#[derive(Deserialize, PartialEq, Serialize)]
pub(crate) struct LineNumbers(pub bool);

impl ViewStateKind for LineNumbers {
    const ID: Uuid = Uuid::from_u128(0x7465_7874_2d6c_696e_652d_6e75_6d00_0001);
}

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
    /// Whether the gutter of line numbers is shown: a choice of this view, kept in its state
    /// (`LineNumbers`), not in the text. Off until it is chosen.
    pub line_numbers: Memo<bool>,
    pub hex_insert_mode: Signal<bool>,
    /// Set once the text has first been read from its block, with the caret at its start.
    pub loaded: Signal<bool>,
    /// Set once the position saved for the text was put back (or there was none), after which a
    /// place it was opened at may take the caret (`select_where_opened`).
    pub position_restored: Signal<bool>,
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
        let text =
            TextAreaState::new(Arc::clone(&document) as Arc<dyn beui_text_editor_core::Document>);
        text.with_core_mut(|core| core.config.inside_atomic_unit = inside_block_url);
        let dependencies = client.watch(BlockQuery::References(block_id));
        let embeds = create_signal(Vec::new());
        let hex_view = create_signal(false);
        let kept_line_numbers = editor.view_state::<LineNumbers>();
        let line_numbers = create_memo(move || {
            kept_line_numbers
                .get()
                .ok()
                .flatten()
                .is_some_and(|kept| kept.0)
        });
        let hex_insert_mode = create_signal(false);
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
            line_numbers,
            hex_insert_mode,
            loaded: create_signal(false),
            position_restored: create_signal(false),
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
            edited.track();
            let Some(inner) = state.inner.try_value() else {
                return;
            };
            inner.content.track();
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
            .sync(&inner.content.live(), &self.host(), self.block_id, first);
        match synced {
            Synced::Waiting => return,
            Synced::External | Synced::Unchanged => self.text.sync(),
        }
        inner.adopted.set(true);
        if first {
            let start = self.text.with_core(|core| core.position(0));
            self.text.execute(EditorCommand::SetSelection {
                anchor: start,
                focus: start,
            });
            self.loaded.set(true);
        }
    }

    pub fn refresh_embeds(&self) {
        let resolved = resolve_embeds(self);
        self.embeds.set(resolved);
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

    /// Shows the gutter of line numbers if it is hidden, else hides it, for this view.
    pub fn toggle_line_numbers(&self) {
        let shown = !self.line_numbers.get_untracked();
        self.editor.view_state().set(Some(LineNumbers(shown)));
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
}
