use std::cell::{Cell, Ref, RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use text_editor_core::{
    CollapsibleSection, CopyMode, Core, CursorPosition, EditorCommand, FindDirection, FindStatus,
    Position, SyntaxHighlight, TextIndentation, TextLanguage, markdown_checkbox_marker,
};

use crate::geometry::{Pos2, Rect, Vec2};
use crate::reactive::{NodeRef, ReadSignal, WriteSignal, create_signal, with_document};

use super::layout::{DocumentLayout, LayoutOptions, TextWidget, hit_test, layout_document};

#[derive(Clone, Copy)]
pub(crate) enum Grab {
    Selection(Position),
    Caret,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MarkdownCheckbox {
    pub line_start: usize,
    pub marker: Range<usize>,
    pub checked: bool,
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

#[derive(Clone, Default)]
pub struct TextAreaLayout {
    document: Rc<DocumentLayout>,
    origin: Vec2,
}

impl TextAreaLayout {
    pub(crate) fn new(document: Rc<DocumentLayout>, origin: Vec2) -> Self {
        Self { document, origin }
    }

    pub(crate) fn with_origin(&self, origin: Vec2) -> Self {
        Self {
            document: Rc::clone(&self.document),
            origin,
        }
    }

    pub(crate) fn document(&self) -> &DocumentLayout {
        &self.document
    }

    pub(crate) fn origin(&self) -> Vec2 {
        self.origin
    }

    pub fn size(&self) -> Vec2 {
        self.document.size
    }

    pub fn caret_rect(&self, byte: usize) -> Option<Rect> {
        super::shapes::caret_rect(&self.document, byte, self.origin)
    }

    pub fn widget_rect(&self, widget: usize) -> Option<Rect> {
        self.document
            .widgets
            .iter()
            .find(|laid_out| laid_out.index == widget)
            .map(|laid_out| laid_out.rect.translate(self.origin))
    }

    pub fn block_widgets(&self) -> Vec<usize> {
        self.document
            .widgets
            .iter()
            .filter(|laid_out| laid_out.block)
            .map(|laid_out| laid_out.index)
            .collect()
    }
}

impl PartialEq for TextAreaLayout {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.document, &other.document) && self.origin == other.origin
    }
}

pub(crate) struct Find {
    pub open: ReadSignal<bool>,
    pub set_open: WriteSignal<bool>,
    pub query: ReadSignal<String>,
    pub set_query: WriteSignal<String>,
    pub replacement: ReadSignal<String>,
    pub set_replacement: WriteSignal<String>,
    pub case_sensitive: ReadSignal<bool>,
    pub set_case_sensitive: WriteSignal<bool>,
    pub show_replace: ReadSignal<bool>,
    pub set_show_replace: WriteSignal<bool>,
    pub focus_query: ReadSignal<bool>,
    pub set_focus_query: WriteSignal<bool>,
}

impl Find {
    fn new() -> Self {
        let (open, set_open) = create_signal(false);
        let (query, set_query) = create_signal(String::new());
        let (replacement, set_replacement) = create_signal(String::new());
        let (case_sensitive, set_case_sensitive) = create_signal(false);
        let (show_replace, set_show_replace) = create_signal(false);
        let (focus_query, set_focus_query) = create_signal(false);
        Self {
            open,
            set_open,
            query,
            set_query,
            replacement,
            set_replacement,
            case_sensitive,
            set_case_sensitive,
            show_replace,
            set_show_replace,
            focus_query,
            set_focus_query,
        }
    }
}

struct Inner {
    core: RefCell<Core>,
    snapshot: RefCell<Snapshot>,
    find: Find,
    canvas: NodeRef,
    cursor_cache: RefCell<Vec<CursorPosition>>,
    selecting: Cell<bool>,
    touch_mode: ReadSignal<bool>,
    set_touch_mode: WriteSignal<bool>,
    caret_handle: ReadSignal<bool>,
    set_caret_handle: WriteSignal<bool>,
    grab: Cell<Option<Grab>>,
    grab_offset: Cell<Vec2>,
    reveal_cursor: Cell<bool>,
    reveal: Cell<Option<Rect>>,
    reveals: ReadSignal<u64>,
    set_reveals: WriteSignal<u64>,
    focus_requests: ReadSignal<u64>,
    set_focus_requests: WriteSignal<u64>,
    content: ReadSignal<u64>,
    set_content: WriteSignal<u64>,
    content_counter: Cell<u64>,
    cursors: ReadSignal<u64>,
    set_cursors: WriteSignal<u64>,
    cursor_counter: Cell<u64>,
    layout: ReadSignal<TextAreaLayout>,
    set_layout: WriteSignal<TextAreaLayout>,
}

#[derive(Clone)]
pub struct TextAreaState(Rc<Inner>);

impl TextAreaState {
    pub fn new(document: Arc<dyn text_editor_core::Document>) -> Self {
        let mut core = Core::new(document);
        let start = core.position(0);
        core.execute_command(EditorCommand::SetSelection {
            anchor: start,
            focus: start,
        });
        let (content, set_content) = create_signal(0);
        let (cursors, set_cursors) = create_signal(0);
        let (layout, set_layout) = create_signal(TextAreaLayout::default());
        let (reveals, set_reveals) = create_signal(0);
        let (focus_requests, set_focus_requests) = create_signal(0);
        let (touch_mode, set_touch_mode) = create_signal(false);
        let (caret_handle, set_caret_handle) = create_signal(false);
        let state = Self(Rc::new(Inner {
            core: RefCell::new(core),
            snapshot: RefCell::new(Snapshot::default()),
            find: Find::new(),
            canvas: NodeRef::new(),
            cursor_cache: RefCell::new(Vec::new()),
            selecting: Cell::new(false),
            touch_mode,
            set_touch_mode,
            caret_handle,
            set_caret_handle,
            grab: Cell::new(None),
            grab_offset: Cell::new(Vec2::ZERO),
            reveal_cursor: Cell::new(false),
            reveal: Cell::new(None),
            reveals,
            set_reveals,
            focus_requests,
            set_focus_requests,
            content,
            set_content,
            content_counter: Cell::new(0),
            cursors,
            set_cursors,
            cursor_counter: Cell::new(0),
            layout,
            set_layout,
        }));
        state.sync();
        state
    }

    pub fn core(&self) -> Ref<'_, Core> {
        self.0.core.borrow()
    }

    pub fn core_mut(&self) -> RefMut<'_, Core> {
        self.0.core.borrow_mut()
    }

    pub fn execute(&self, command: EditorCommand<'_>) {
        self.0.core.borrow_mut().execute_command(command);
        self.sync();
    }

    pub fn copy(&self, mode: CopyMode) -> String {
        let text = self.0.core.borrow_mut().copy_utf8(mode);
        self.sync();
        text
    }

    pub fn external_edit(&self) {
        self.0.core.borrow_mut().external_edit();
        self.sync();
    }

    pub fn sync(&self) {
        self.sync_document();
        self.sync_cursors();
    }

    pub fn content(&self) -> ReadSignal<u64> {
        self.0.content.clone()
    }

    pub fn cursors(&self) -> ReadSignal<u64> {
        self.0.cursors.clone()
    }

    pub fn layout(&self) -> ReadSignal<TextAreaLayout> {
        self.0.layout.clone()
    }

    pub fn loaded(&self) -> bool {
        self.0.snapshot.borrow().loaded
    }

    pub fn bytes(&self) -> Ref<'_, [u8]> {
        Ref::map(self.0.snapshot.borrow(), |snapshot| {
            snapshot.bytes.as_slice()
        })
    }

    pub fn language(&self) -> TextLanguage {
        self.0.core.borrow().language()
    }

    pub fn indentation(&self) -> TextIndentation {
        self.0.core.borrow().indentation()
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        let core = self.0.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.selection_range(cursor))
            .collect()
    }

    pub fn caret_indices(&self) -> Vec<usize> {
        let core = self.0.core.borrow();
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
        let core = self.0.core.borrow();
        let Some(cursor) = core.cursor_positions().first() else {
            return false;
        };
        let line_start = core.get_line_start(cursor.pos.focus);
        let Some(line_start) = core.position_index(line_start) else {
            return false;
        };
        self.0
            .snapshot
            .borrow()
            .sections
            .iter()
            .any(|section| section.line_start == line_start && section.collapsed)
    }

    pub fn reveal_cursor(&self) {
        self.0.reveal_cursor.set(true);
        self.request_reveal();
    }

    pub fn reveal(&self, rect: Rect) {
        self.0.reveal.set(Some(rect));
        self.request_reveal();
    }

    fn request_reveal(&self) {
        self.0.set_reveals.update(|count| *count += 1);
    }

    pub fn focus(&self) {
        self.0.set_focus_requests.update(|count| *count += 1);
    }

    pub(crate) fn focus_requests(&self) -> ReadSignal<u64> {
        self.0.focus_requests.clone()
    }

    pub(crate) fn reveals(&self) -> ReadSignal<u64> {
        self.0.reveals.clone()
    }

    pub(crate) fn canvas(&self) -> NodeRef {
        self.0.canvas.clone()
    }

    pub(crate) fn take_reveal_cursor(&self) -> bool {
        self.0.reveal_cursor.replace(false)
    }

    pub(crate) fn take_reveal(&self) -> Option<Rect> {
        self.0.reveal.take()
    }

    pub(crate) fn with_snapshot<R>(&self, read: impl FnOnce(&Snapshot) -> R) -> R {
        read(&self.0.snapshot.borrow())
    }

    pub(crate) fn publish_layout(&self, layout: TextAreaLayout) {
        self.0.set_layout.set(layout);
    }

    pub(crate) fn sections(&self) -> Vec<CollapsibleSection> {
        self.0.snapshot.borrow().sections.clone()
    }

    pub(crate) fn checkboxes(&self) -> Vec<MarkdownCheckbox> {
        self.0.snapshot.borrow().checkboxes.clone()
    }

    pub(crate) fn selecting(&self) -> bool {
        self.0.selecting.get()
    }

    pub(crate) fn set_selecting(&self, selecting: bool) {
        self.0.selecting.set(selecting);
    }

    pub(crate) fn touch_mode(&self) -> ReadSignal<bool> {
        self.0.touch_mode.clone()
    }

    pub(crate) fn set_touch_mode(&self, touch: bool) {
        self.0.set_touch_mode.set(touch);
    }

    pub(crate) fn caret_handle(&self) -> ReadSignal<bool> {
        self.0.caret_handle.clone()
    }

    pub(crate) fn set_caret_handle(&self, shown: bool) {
        self.0.set_caret_handle.set(shown);
    }

    pub(crate) fn grab(&self) -> Option<Grab> {
        self.0.grab.get()
    }

    pub(crate) fn begin_grab(&self, grab: Grab, offset: Vec2) {
        self.0.grab.set(Some(grab));
        self.0.grab_offset.set(offset);
        self.0.selecting.set(false);
    }

    pub(crate) fn end_grab(&self) -> Option<Grab> {
        self.0.grab_offset.set(Vec2::ZERO);
        self.0.grab.take()
    }

    pub(crate) fn grab_offset(&self) -> Vec2 {
        self.0.grab_offset.get()
    }

    pub fn byte_at(&self, pos: Pos2) -> Option<usize> {
        let local = self.local(pos)?;
        let layout = self.0.layout.get_untracked();
        Some(hit_test(layout.document(), Vec2::new(local.x, local.y)))
    }

    pub fn measure(&self, widgets: &[TextWidget], width: f32) -> Option<Vec2> {
        let snapshot = self.0.snapshot.borrow();
        if !snapshot.loaded {
            return None;
        }
        let document = layout_document(
            &snapshot.bytes,
            snapshot.highlight(),
            widgets,
            &snapshot.checkbox_markers,
            &snapshot.hidden,
            &LayoutOptions::wrapped((width - super::shapes::PADDING.x * 2.0).max(1.0)),
        )?;
        Some(Vec2::new(width, document.size.y))
    }

    pub fn find_open(&self) -> ReadSignal<bool> {
        self.0.find.open.clone()
    }

    pub fn find_status(&self) -> FindStatus {
        let (query, case_sensitive) = self.query();
        self.0.core.borrow().find_status(&query, case_sensitive)
    }

    pub fn open_find(&self, show_replace: bool) {
        if !self.0.find.open.get_untracked() {
            let selected = self.copy(CopyMode::Copy);
            let query = match !selected.is_empty() && !selected.contains('\n') {
                true => selected,
                false => String::new(),
            };
            self.0.find.set_query.set(query);
            self.0.find.set_open.set(true);
        }
        self.0.find.set_show_replace.set(show_replace);
        self.0.find.set_focus_query.set(true);
        if self.sync_find() {
            self.reveal_cursor();
        }
    }

    pub fn close_find(&self) {
        self.0.find.set_open.set(false);
        self.0.find.set_focus_query.set(false);
    }

    pub fn sync_find(&self) -> bool {
        if !self.0.find.open.get_untracked() {
            return false;
        }
        let (query, case_sensitive) = self.query();
        let status = self.0.core.borrow().find_status(&query, case_sensitive);
        if status.current.is_none() && status.total > 0 {
            self.execute(EditorCommand::Find {
                text: &query,
                case_sensitive,
                direction: FindDirection::Next,
            });
        }
        status.total > 0
    }

    pub fn find_step(&self, direction: FindDirection) -> bool {
        if !self.0.find.open.get_untracked() {
            return false;
        }
        let (query, case_sensitive) = self.query();
        self.execute(EditorCommand::Find {
            text: &query,
            case_sensitive,
            direction,
        });
        self.0
            .core
            .borrow()
            .find_status(&query, case_sensitive)
            .total
            > 0
    }

    pub fn replace_match(&self) {
        let (query, case_sensitive) = self.query();
        let replacement = self.0.find.replacement.get_untracked();
        self.execute(EditorCommand::ReplaceMatch {
            text: &query,
            case_sensitive,
            replacement: replacement.as_bytes(),
        });
    }

    pub fn replace_all_matches(&self) {
        let (query, case_sensitive) = self.query();
        let replacement = self.0.find.replacement.get_untracked();
        self.execute(EditorCommand::ReplaceAllMatches {
            text: &query,
            case_sensitive,
            replacement: replacement.as_bytes(),
        });
    }

    pub(crate) fn find(&self) -> &Find {
        &self.0.find
    }

    pub(crate) fn local(&self, pos: Pos2) -> Option<Pos2> {
        let node = self.0.canvas.try_get()?;
        let rect = with_document(|document| document.node_rect(node))?;
        let origin = self.0.layout.get_untracked().origin();
        Some(Pos2::new(
            pos.x - rect.min.x - origin.x,
            pos.y - rect.min.y - origin.y,
        ))
    }

    pub(crate) fn gutter_local(&self, pos: Pos2) -> Option<Pos2> {
        let node = self.0.canvas.try_get()?;
        let rect = with_document(|document| document.node_rect(node))?;
        Some(Pos2::new(pos.x - rect.min.x, pos.y - rect.min.y))
    }

    fn query(&self) -> (String, bool) {
        (
            self.0.find.query.get_untracked(),
            self.0.find.case_sensitive.get_untracked(),
        )
    }

    fn sync_document(&self) {
        let revision = self.0.core.borrow().document().revision();
        let (sections, language) = {
            let core = self.0.core.borrow();
            (core.collapsible_sections(), core.language())
        };
        let loaded = self.0.core.borrow().document().read().is_some();
        {
            let snapshot = self.0.snapshot.borrow();
            if snapshot.highlight.is_some()
                && snapshot.loaded == loaded
                && snapshot.revision == revision
                && snapshot.language == language
                && snapshot.sections == sections
            {
                return;
            }
        }
        let bytes = read_bytes(&self.0.core.borrow());
        let highlight = self.0.core.borrow_mut().highlight();
        let checkboxes = match language {
            TextLanguage::Markdown => parse_markdown_checkboxes(&bytes),
            _ => Vec::new(),
        };
        let checkbox_markers = checkboxes
            .iter()
            .map(|checkbox| checkbox.marker.clone())
            .collect();
        let hidden = hidden_ranges(&sections);
        *self.0.snapshot.borrow_mut() = Snapshot {
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
        self.0.content_counter.set(self.0.content_counter.get() + 1);
        self.0.set_content.set(self.0.content_counter.get());
    }

    fn sync_cursors(&self) {
        let positions = self.0.core.borrow().cursor_positions().to_vec();
        if *self.0.cursor_cache.borrow() == positions {
            return;
        }
        *self.0.cursor_cache.borrow_mut() = positions;
        self.0.cursor_counter.set(self.0.cursor_counter.get() + 1);
        self.0.set_cursors.set(self.0.cursor_counter.get());
    }
}

fn read_bytes(core: &Core) -> Vec<u8> {
    let Some(read) = core.document().read() else {
        return Vec::new();
    };
    read.slice(0..read.len()).into_owned()
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
