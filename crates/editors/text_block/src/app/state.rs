use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::{Rect, Vec2};
use block::{BlockParent, BlockReferenceList, ClientId};
use block_client::{
    BlockClient, BlockHandle, ReferenceList, block_ref::BlockRef, blocks::image::Image,
    blocks::text::TextDocument, presence::PresenceColor, presence::UserActive,
};
use block_editor_plugin::{ChildState, Editor, EditorHost, ImagePaster, Task};
use text_editor_core::{
    CollapsibleSection, CopyMode, Core, CursorPosition, EditorCommand, SyntaxHighlight,
    TextIndentation, TextLanguage, markdown_checkbox_marker,
};
use uuid::Uuid;

use crate::document::{BlockDocument, inside_block_url};
use crate::layout::{DocumentLayout, ResolvedEmbed};
use crate::presence::TextCursor;

use super::embeds::{EmbedReferenceCache, PendingEmbed, resolve_embeds};
use super::find::{Find, close_find};

pub(crate) const DIRECT_EDITOR_WIDTH: f32 = 600.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkdownCheckbox {
    pub line_start: usize,
    pub marker: Range<usize>,
    pub checked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FocusedEmbed {
    pub id: Uuid,
    pub source_start: usize,
}

#[derive(Default)]
pub(crate) struct Snapshot {
    pub loaded: bool,
    pub revision: u64,
    pub bytes: Vec<u8>,
    pub language: TextLanguage,
    pub sections: Vec<CollapsibleSection>,
    pub hidden: Vec<Range<usize>>,
    pub checkboxes: Vec<MarkdownCheckbox>,
    pub checkbox_markers: Vec<Range<usize>>,
    pub highlight: Option<SyntaxHighlight>,
}

impl Snapshot {
    pub fn highlight(&self) -> &SyntaxHighlight {
        self.highlight
            .as_ref()
            .expect("a snapshot is only read once it has been filled in")
    }
}

#[derive(Clone)]
pub(crate) struct Layout(pub Rc<DocumentLayout>);

impl PartialEq for Layout {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self(Rc::new(DocumentLayout::default()))
    }
}

pub(crate) struct State {
    pub editor: Editor,
    pub client: Arc<BlockClient>,
    pub block: BlockHandle<TextDocument>,
    pub document: Arc<BlockDocument>,
    pub workspace_id: Uuid,
    pub core: RefCell<Core>,
    pub dependencies: ReferenceList,
    pub paster: RefCell<ImagePaster>,
    pub references: RefCell<EmbedReferenceCache>,
    pub pending_embeds: RefCell<Vec<PendingEmbed>>,
    pub embed_sizes: RefCell<HashMap<Uuid, Vec2>>,
    pub snapshot: RefCell<Snapshot>,
    pub find: Find,
    pub embed_children: RefCell<HashMap<FocusedEmbed, ChildState>>,
    pub cursor_cache: RefCell<Vec<CursorPosition>>,
    pub published_cursor: Cell<Option<TextCursor>>,
    pub focused_embed: ReadSignal<Option<FocusedEmbed>>,
    pub set_focused_embed: WriteSignal<Option<FocusedEmbed>>,
    pub focus_confirmed: Cell<bool>,
    pub selecting: Cell<bool>,
    pub touch_mode: Cell<bool>,
    pub dragging_handle: Cell<Option<text_editor_core::Position>>,
    pub dragging_handle_offset: Cell<Vec2>,
    pub reveal_cursor: Cell<bool>,
    pub paste_requested: Cell<bool>,

    pub content: ReadSignal<u64>,
    set_content: WriteSignal<u64>,
    content_counter: Cell<u64>,
    pub cursors: ReadSignal<u64>,
    set_cursors: WriteSignal<u64>,
    cursor_counter: Cell<u64>,
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
    remote_cursors: RefCell<Vec<(ClientId, TextCursor)>>,
}

pub(crate) type Shared = Rc<State>;

impl State {
    pub fn new(editor: Editor) -> Shared {
        let client = Arc::clone(editor.client());
        let block = client.get_block::<TextDocument>(editor.block_id());
        let document = Arc::new(BlockDocument::new(block.clone()));
        let mut core = Core::new(Arc::clone(&document) as Arc<dyn text_editor_core::Document>);
        core.config.inside_atomic_unit = inside_block_url;
        let start = core.position(0);
        core.execute_command(EditorCommand::SetSelection {
            anchor: start,
            focus: start,
        });
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        let (content, set_content) = create_signal(0);
        let (cursors, set_cursors) = create_signal(0);
        let (embeds, set_embeds) = create_signal(Vec::new());
        let (hex_view, set_hex_view) = create_signal(false);
        let (hex_insert_mode, set_hex_insert_mode) = create_signal(false);
        let (import_error, set_import_error) = create_signal(None);
        let (presence_revision, set_presence_revision) = create_signal(0);
        let (focused_embed, set_focused_embed) = create_signal(None);
        let state = Rc::new(Self {
            workspace_id: client.workspace_id(),
            editor,
            client,
            block,
            document,
            core: RefCell::new(core),
            dependencies,
            paster: RefCell::new(ImagePaster::default()),
            references: RefCell::new(EmbedReferenceCache::default()),
            pending_embeds: RefCell::new(Vec::new()),
            embed_sizes: RefCell::new(HashMap::new()),
            snapshot: RefCell::new(Snapshot::default()),
            find: Find::new(),
            embed_children: RefCell::new(HashMap::new()),
            cursor_cache: RefCell::new(Vec::new()),
            published_cursor: Cell::new(None),
            focused_embed,
            set_focused_embed,
            focus_confirmed: Cell::new(false),
            selecting: Cell::new(false),
            touch_mode: Cell::new(false),
            dragging_handle: Cell::new(None),
            dragging_handle_offset: Cell::new(Vec2::ZERO),
            reveal_cursor: Cell::new(false),
            paste_requested: Cell::new(false),
            content,
            set_content,
            content_counter: Cell::new(0),
            cursors,
            set_cursors,
            cursor_counter: Cell::new(0),
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
        state.sync();
        state
    }

    pub fn host(&self) -> &EditorHost {
        self.editor.host()
    }

    pub fn execute(&self, command: EditorCommand<'_>) {
        self.core.borrow_mut().execute_command(command);
        self.sync();
    }

    pub fn copy(&self, mode: CopyMode) -> String {
        let text = self.core.borrow_mut().copy_utf8(mode);
        self.sync();
        text
    }

    pub fn sync(&self) {
        self.sync_document();
        self.sync_cursors();
    }

    fn sync_document(&self) {
        let revision = self.block.revision();
        let (sections, language) = {
            let core = self.core.borrow();
            (core.collapsible_sections(), core.language())
        };
        let loaded = self.block.read().is_some();
        {
            let snapshot = self.snapshot.borrow();
            if snapshot.highlight.is_some()
                && snapshot.loaded == loaded
                && snapshot.revision == revision
                && snapshot.language == language
                && snapshot.sections == sections
            {
                return;
            }
        }
        let bytes = self
            .block
            .read()
            .map(|document| document.bytes().to_vec())
            .unwrap_or_default();
        let highlight = self.core.borrow_mut().highlight();
        let checkboxes = match language {
            TextLanguage::Markdown => parse_markdown_checkboxes(&bytes),
            _ => Vec::new(),
        };
        let checkbox_markers = checkboxes
            .iter()
            .map(|checkbox| checkbox.marker.clone())
            .collect();
        let hidden = hidden_ranges(&sections);
        *self.snapshot.borrow_mut() = Snapshot {
            loaded,
            revision,
            bytes,
            language,
            sections,
            hidden,
            checkboxes,
            checkbox_markers,
            highlight: Some(highlight),
        };
        self.content_counter.set(self.content_counter.get() + 1);
        self.set_content.set(self.content_counter.get());
    }

    fn sync_cursors(&self) {
        let positions = self.core.borrow().cursor_positions().to_vec();
        if *self.cursor_cache.borrow() == positions {
            return;
        }
        *self.cursor_cache.borrow_mut() = positions;
        self.cursor_counter.set(self.cursor_counter.get() + 1);
        self.set_cursors.set(self.cursor_counter.get());
    }

    pub fn poll_external_edit(&self) {
        if self.document.take_external_edit() {
            self.core.borrow_mut().external_edit();
        }
        self.sync();
    }

    pub fn refresh_embeds(&self) {
        let resolved = resolve_embeds(self);
        if self.embeds.get_untracked() != resolved {
            self.set_embeds.set(resolved);
        }
    }

    pub fn language(&self) -> TextLanguage {
        self.core.borrow().language()
    }

    pub fn indentation(&self) -> TextIndentation {
        self.core.borrow().indentation()
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        let core = self.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.selection_range(cursor))
            .collect()
    }

    pub fn caret_indices(&self) -> Vec<usize> {
        let core = self.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.position_index(cursor.pos.focus))
            .collect()
    }

    pub fn selection_contains(&self, byte: usize) -> bool {
        self.selection_ranges()
            .iter()
            .any(|range| range.start < range.end && range.contains(&byte))
    }

    pub fn cursor_line_collapsed(&self) -> bool {
        let core = self.core.borrow();
        let Some(cursor) = core.cursor_positions().first() else {
            return false;
        };
        let line_start = core.get_line_start(cursor.pos.focus);
        let Some(line_start) = core.position_index(line_start) else {
            return false;
        };
        self.snapshot
            .borrow()
            .sections
            .iter()
            .any(|section| section.line_start == line_start && section.collapsed)
    }

    pub fn toggle_hex_view(&self) {
        let next = !self.hex_view.get_untracked();
        self.set_hex_view.set(next);
        self.hex_pending_nibble.set(None);
        self.hex_selection_anchor.set(None);
        self.set_focused_embed.set(None);
        self.focus_confirmed.set(false);
        if next {
            close_find(self);
        }
    }

    pub fn create_image_block(&self, image: Image) -> Uuid {
        let block = self.client.create_block(image);
        block.set_parent(BlockParent::Uuid(self.block.id()));
        block.id()
    }

    pub fn insert_image_embed(&self, id: Uuid, source_name: &str) {
        let client = Arc::clone(&self.client);
        let referencing_id = self.block.id();
        let task: Task<BlockRef> = self.host().spawn(async move {
            client
                .classify_reference(
                    referencing_id,
                    id,
                    &block_client::blocks::version_control_worktree::VersionControlWorktreeMembership,
                )
                .await
        });
        self.pending_embeds.borrow_mut().push(PendingEmbed {
            task,
            source_name: source_name.to_owned(),
            markdown: self.language() == TextLanguage::Markdown,
        });
    }

    pub fn presence_colors(&self) -> HashMap<ClientId, PresenceColor> {
        self.client
            .presence::<UserActive>(self.block.id())
            .into_iter()
            .map(|(client_id, user)| (client_id, user.color))
            .collect()
    }

    pub fn remote_cursors(&self) -> Vec<(ClientId, TextCursor)> {
        self.remote_cursors.borrow().clone()
    }

    pub fn poll_presence(&self, visible: bool) {
        if !visible {
            if self.published_cursor.take().is_some() {
                self.client
                    .set_presence::<TextCursor>(self.block.id(), None);
            }
            if !self.remote_cursors.borrow().is_empty() {
                self.remote_cursors.borrow_mut().clear();
                self.bump_presence();
            }
            return;
        }
        if let Some(cursor) = self.core.borrow().cursor_positions().first() {
            let published = TextCursor {
                anchor: cursor.pos.anchor,
                focus: cursor.pos.focus,
            };
            if self.published_cursor.get() != Some(published) {
                self.published_cursor.set(Some(published));
                self.client.set_presence(self.block.id(), Some(&published));
            }
        }
        let mut remote = self.client.presence::<TextCursor>(self.block.id());
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

    pub fn presence_cursor_rect(
        &self,
        client_id: ClientId,
        layout: &DocumentLayout,
    ) -> Option<Rect> {
        let cursor = self
            .remote_cursors
            .borrow()
            .iter()
            .find(|(id, _)| *id == client_id)
            .map(|(_, cursor)| *cursor)?;
        let focus = self.core.borrow().position_index(cursor.focus)?;
        let position = layout.positions.get(focus).copied().flatten()?;
        let line = layout.lines.get(position.line)?;
        Some(Rect::from_min_size(
            beui::Pos2::new(position.x, line.y),
            Vec2::new(2.0, line.height),
        ))
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let ranges = self.document.reference_ranges(old);
        self.core
            .borrow_mut()
            .execute_command(EditorCommand::ReplaceRanges {
                ranges: &ranges,
                replacement: new.to_string().as_bytes(),
            });
        true
    }
}

pub(crate) fn hidden_ranges(sections: &[CollapsibleSection]) -> Vec<Range<usize>> {
    sections
        .iter()
        .filter(|section| section.collapsed)
        .map(|section| section.line_end + 1..section.content_end + 1)
        .collect()
}

pub(crate) fn parse_markdown_checkboxes(bytes: &[u8]) -> Vec<MarkdownCheckbox> {
    let mut result = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| line_start + offset);
        if let Some(marker) = markdown_checkbox_marker(bytes, line_start) {
            result.push(MarkdownCheckbox {
                line_start,
                marker: marker.marker,
                checked: marker.checked,
            });
        }
        if line_end == bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }
    result
}
