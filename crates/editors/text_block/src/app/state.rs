use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{ReadSignal, WriteSignal, create_signal};
use beui::unstyled::TextAreaState;
use beui::{Rect, Vec2};
use block::{BlockParent, BlockReferenceList, ClientId};
use block_client::{
    BlockClient, BlockHandle, ReferenceList, block_ref::BlockRef, blocks::image::Image,
    blocks::text::TextDocument, presence::PresenceColor, presence::UserActive,
};
use block_editor_plugin::{ChildState, Editor, EditorHost, ImagePaster, Task};
use text_editor_core::{EditorCommand, TextLanguage};
use uuid::Uuid;

use crate::document::{BlockDocument, inside_block_url};
use crate::presence::TextCursor;

use super::embeds::{EmbedReferenceCache, PendingEmbed, ResolvedEmbed, resolve_embeds};

pub(crate) const DIRECT_EDITOR_WIDTH: f32 = 600.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FocusedEmbed {
    pub id: Uuid,
    pub source_start: usize,
}

pub(crate) struct State {
    pub editor: Editor,
    pub client: Arc<BlockClient>,
    pub block: BlockHandle<TextDocument>,
    pub document: Arc<BlockDocument>,
    pub workspace_id: Uuid,
    pub text: TextAreaState,
    pub dependencies: ReferenceList,
    pub paster: RefCell<ImagePaster>,
    pub references: RefCell<EmbedReferenceCache>,
    pub pending_embeds: RefCell<Vec<PendingEmbed>>,
    pub embed_sizes: RefCell<HashMap<Uuid, Vec2>>,
    pub embed_children: RefCell<HashMap<FocusedEmbed, ChildState>>,
    pub published_cursor: Cell<Option<TextCursor>>,
    pub focused_embed: ReadSignal<Option<FocusedEmbed>>,
    pub set_focused_embed: WriteSignal<Option<FocusedEmbed>>,
    pub focus_confirmed: Cell<bool>,
    pub paste_requested: Cell<bool>,

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
        let text = TextAreaState::new(Arc::clone(&document) as Arc<dyn text_editor_core::Document>);
        text.core_mut().config.inside_atomic_unit = inside_block_url;
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        let (embeds, set_embeds) = create_signal(Vec::new());
        let (hex_view, set_hex_view) = create_signal(false);
        let (hex_insert_mode, set_hex_insert_mode) = create_signal(false);
        let (import_error, set_import_error) = create_signal(None);
        let (presence_revision, set_presence_revision) = create_signal(0);
        let (focused_embed, set_focused_embed) = create_signal(None);
        Rc::new(Self {
            workspace_id: client.workspace_id(),
            editor,
            client,
            block,
            document,
            text,
            dependencies,
            paster: RefCell::new(ImagePaster::default()),
            references: RefCell::new(EmbedReferenceCache::default()),
            pending_embeds: RefCell::new(Vec::new()),
            embed_sizes: RefCell::new(HashMap::new()),
            embed_children: RefCell::new(HashMap::new()),
            published_cursor: Cell::new(None),
            focused_embed,
            set_focused_embed,
            focus_confirmed: Cell::new(false),
            paste_requested: Cell::new(false),
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
        })
    }

    pub fn host(&self) -> &EditorHost {
        self.editor.host()
    }

    pub fn poll_external_edit(&self) {
        if self.document.take_external_edit() {
            self.text.external_edit();
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
            markdown: self.text.language() == TextLanguage::Markdown,
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
        if let Some(cursor) = self.text.core().cursor_positions().first() {
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

    pub fn presence_cursor_rect(&self, client_id: ClientId) -> Option<Rect> {
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
        true
    }
}
