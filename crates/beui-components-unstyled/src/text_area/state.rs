use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::fast::FixedState;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use text_editor_core::{
    CollapsibleSection, CopyMode, Core, CursorPosition, EditorCommand, FindDirection, FindStatus,
    Position, SyntaxHighlight, TextChange, TextIndentation, TextLanguage, TextRun,
    markdown_checkbox_marker,
};

use beui_core::font::FontId;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_view::reactive::*;

use super::colors::TextAreaColors;
use super::rows::{
    DOCUMENT_PADDING, Prose, RowInputs, RowOptions, TextWidget, build_row, line_range, line_starts,
    rich_layout,
};

#[derive(Clone, Copy)]
pub enum Grab {
    Selection(Position),
    Caret,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownCheckbox {
    pub line_start: usize,
    pub marker: Range<usize>,
    pub checked: bool,
}

#[derive(Default)]
pub struct Snapshot {
    pub loaded: bool,
    pub revision: u64,
    pub bytes: Arc<[u8]>,
    pub starts: Rc<Vec<usize>>,
    pub language: TextLanguage,
    pub sections: Vec<CollapsibleSection>,
    pub hidden: Vec<Range<usize>>,
    pub checkboxes: Vec<MarkdownCheckbox>,
    pub highlight: Option<SyntaxHighlight>,
    pub runs: Option<Rc<Vec<TextRun>>>,
}

const MEASURED_BYTES: usize = 64 * 1024;

impl Snapshot {
    pub fn runs_in(&self, start: usize, end: usize) -> &[TextRun] {
        let Some(runs) = &self.runs else {
            return &[];
        };
        let first = runs.partition_point(|run| run.at < start);
        let last = runs.partition_point(|run| run.at <= end);
        &runs[first..last.max(first)]
    }

    pub fn highlight(&self) -> &SyntaxHighlight {
        self.highlight
            .as_ref()
            .expect("a snapshot is only read once it has been filled in")
    }
}

pub trait AreaGeometry {
    fn caret_rect(&self, byte: usize) -> Option<Rect>;
    fn index_at(&self, pos: Pos2) -> Option<usize>;
}

#[derive(Clone, Default)]
pub struct TextAreaLayout {
    geometry: Option<Rc<dyn AreaGeometry>>,
}

impl TextAreaLayout {
    pub fn new(geometry: Rc<dyn AreaGeometry>) -> Self {
        Self {
            geometry: Some(geometry),
        }
    }

    pub fn caret_rect(&self, byte: usize) -> Option<Rect> {
        self.geometry.as_ref()?.caret_rect(byte)
    }

    pub fn index_at(&self, pos: Pos2) -> Option<usize> {
        self.geometry.as_ref()?.index_at(pos)
    }
}

impl PartialEq for TextAreaLayout {
    fn eq(&self, other: &Self) -> bool {
        match (&self.geometry, &other.geometry) {
            (Some(left), Some(right)) => Rc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Find {
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

type CursorCache = (
    Vec<CursorPosition>,
    Vec<(Option<usize>, Option<usize>)>,
    Option<std::ops::Range<usize>>,
);

struct Inner {
    core: RefCell<Core>,
    snapshot: RefCell<Snapshot>,
    heights: RefCell<HashMap<u64, (f32, f32)>>,
    find: Find,
    canvas: NodeRef,
    cursor_cache: RefCell<CursorCache>,
    selecting: Cell<bool>,
    show_runs: Cell<bool>,
    touch_mode: ReadSignal<bool>,
    set_touch_mode: WriteSignal<bool>,
    caret_handle: ReadSignal<bool>,
    set_caret_handle: WriteSignal<bool>,
    grab: Cell<Option<Grab>>,
    grab_offset: Cell<Vec2>,
    reveal_cursor: Cell<bool>,
    tab_released: Cell<bool>,
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
    rows_built: Cell<u64>,
    layout: ReadSignal<TextAreaLayout>,
    set_layout: WriteSignal<TextAreaLayout>,
}

#[derive(Clone, Copy)]
pub struct TextAreaState(Stored<Inner>);

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
        let state = Self(store(Inner {
            core: RefCell::new(core),
            snapshot: RefCell::new(Snapshot::default()),
            heights: RefCell::default(),
            find: Find::new(),
            canvas: NodeRef::new(),
            cursor_cache: RefCell::new((Vec::new(), Vec::new(), None)),
            selecting: Cell::new(false),
            show_runs: Cell::new(false),
            touch_mode,
            set_touch_mode,
            caret_handle,
            set_caret_handle,
            grab: Cell::new(None),
            grab_offset: Cell::new(Vec2::ZERO),
            reveal_cursor: Cell::new(false),
            tab_released: Cell::new(false),
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
            rows_built: Cell::new(0),
            layout,
            set_layout,
        }));
        state.sync();
        state
    }

    pub fn with_core<R>(&self, read: impl FnOnce(&Core) -> R) -> R {
        self.0.with(|inner| read(&inner.core.borrow()))
    }

    pub fn with_core_mut<R>(&self, change: impl FnOnce(&mut Core) -> R) -> R {
        self.0.with(|inner| change(&mut inner.core.borrow_mut()))
    }

    pub fn execute(&self, command: EditorCommand<'_>) {
        let inner = self.0.get();
        inner.core.borrow_mut().execute_command(command);
        self.sync();
    }

    pub fn copy(&self, mode: CopyMode) -> String {
        let inner = self.0.get();
        let text = inner.core.borrow_mut().copy_utf8(mode);
        self.sync();
        text
    }

    pub fn external_edit(&self) {
        let inner = self.0.get();
        inner.core.borrow_mut().external_edit();
        self.sync();
    }

    pub fn sync(&self) {
        self.sync_document();
        self.sync_cursors();
    }

    pub fn set_show_runs(&self, shown: bool) {
        let inner = self.0.get();
        inner.show_runs.set(shown);
        self.sync_document();
    }

    pub fn content(&self) -> ReadSignal<u64> {
        let inner = self.0.get();
        inner.content
    }

    pub fn rows_built(&self) -> u64 {
        self.0.with(|inner| inner.rows_built.get())
    }

    pub(super) fn count_row_built(&self) {
        self.0
            .with(|inner| inner.rows_built.set(inner.rows_built.get() + 1));
    }

    pub fn cursors(&self) -> ReadSignal<u64> {
        let inner = self.0.get();
        inner.cursors
    }

    pub fn layout(&self) -> ReadSignal<TextAreaLayout> {
        let inner = self.0.get();
        inner.layout
    }

    pub fn loaded(&self) -> bool {
        let inner = self.0.get();
        inner.snapshot.borrow().loaded
    }

    pub fn bytes(&self) -> Arc<[u8]> {
        self.0.with(|inner| inner.snapshot.borrow().bytes.clone())
    }

    pub fn language(&self) -> TextLanguage {
        let inner = self.0.get();
        inner.core.borrow().language()
    }

    pub fn indentation(&self) -> TextIndentation {
        let inner = self.0.get();
        inner.core.borrow().indentation()
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        let inner = self.0.get();
        let core = inner.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.selection_range(cursor))
            .collect()
    }

    pub fn caret_indices(&self) -> Vec<usize> {
        let inner = self.0.get();
        let core = inner.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.position_index(cursor.pos.focus))
            .collect()
    }

    pub fn select(&self, range: Range<usize>) {
        let (anchor, focus) =
            self.with_core(|core| (core.position(range.start), core.position(range.end)));
        self.execute(EditorCommand::SetSelection { anchor, focus });
    }

    pub fn selection_contains(&self, byte: usize) -> bool {
        self.selection_ranges()
            .iter()
            .any(|range| range.start < range.end && range.contains(&byte))
    }

    pub fn cursor_line_collapsed(&self) -> bool {
        let inner = self.0.get();
        let core = inner.core.borrow();
        let Some(cursor) = core.cursor_positions().first() else {
            return false;
        };
        let line_start = core.get_line_start(cursor.pos.focus);
        let Some(line_start) = core.position_index(line_start) else {
            return false;
        };
        inner
            .snapshot
            .borrow()
            .sections
            .iter()
            .any(|section| section.line_start == line_start && section.collapsed)
    }

    pub fn reveal_cursor(&self) {
        let inner = self.0.get();
        inner.reveal_cursor.set(true);
        self.request_reveal();
    }

    pub fn reveal(&self, rect: Rect) {
        let inner = self.0.get();
        inner.reveal.set(Some(rect));
        self.request_reveal();
    }

    fn request_reveal(&self) {
        let inner = self.0.get();
        inner.set_reveals.update(|reveals| *reveals += 1);
    }

    pub fn focus(&self) {
        let inner = self.0.get();
        inner.set_focus_requests.update(|count| *count += 1);
    }

    pub fn focus_requests(&self) -> ReadSignal<u64> {
        let inner = self.0.get();
        inner.focus_requests
    }

    pub fn reveals(&self) -> ReadSignal<u64> {
        let inner = self.0.get();
        inner.reveals
    }

    pub fn canvas(&self) -> NodeRef {
        let inner = self.0.get();
        inner.canvas
    }

    pub fn take_reveal_cursor(&self) -> bool {
        let inner = self.0.get();
        inner.reveal_cursor.replace(false)
    }

    pub fn take_reveal(&self) -> Option<Rect> {
        let inner = self.0.get();
        inner.reveal.take()
    }

    pub fn with_snapshot<R>(&self, read: impl FnOnce(&Snapshot) -> R) -> R {
        let inner = self.0.get();
        read(&inner.snapshot.borrow())
    }

    pub fn publish_layout(&self, layout: TextAreaLayout) {
        let inner = self.0.get();
        inner.set_layout.set(layout);
    }

    pub fn sections(&self) -> Vec<CollapsibleSection> {
        let inner = self.0.get();
        inner.snapshot.borrow().sections.clone()
    }

    pub fn checkboxes(&self) -> Vec<MarkdownCheckbox> {
        let inner = self.0.get();
        inner.snapshot.borrow().checkboxes.clone()
    }

    pub fn selecting(&self) -> bool {
        let inner = self.0.get();
        inner.selecting.get()
    }

    pub fn set_selecting(&self, selecting: bool) {
        let inner = self.0.get();
        inner.selecting.set(selecting);
    }

    pub fn touch_mode(&self) -> ReadSignal<bool> {
        let inner = self.0.get();
        inner.touch_mode
    }

    pub fn set_touch_mode(&self, touch: bool) {
        let inner = self.0.get();
        inner.set_touch_mode.set(touch);
    }

    pub fn caret_handle(&self) -> ReadSignal<bool> {
        let inner = self.0.get();
        inner.caret_handle
    }

    pub fn release_tab(&self, released: bool) -> bool {
        let inner = self.0.get();
        inner.tab_released.replace(released)
    }

    pub fn set_caret_handle(&self, shown: bool) {
        let inner = self.0.get();
        inner.set_caret_handle.set(shown);
    }

    pub fn grab(&self) -> Option<Grab> {
        let inner = self.0.get();
        inner.grab.get()
    }

    pub fn begin_grab(&self, grab: Grab, offset: Vec2) {
        let inner = self.0.get();
        inner.grab.set(Some(grab));
        inner.grab_offset.set(offset);
        inner.selecting.set(false);
    }

    pub fn end_grab(&self) -> Option<Grab> {
        let inner = self.0.get();
        inner.grab_offset.set(Vec2::ZERO);
        inner.grab.take()
    }

    pub fn grab_offset(&self) -> Vec2 {
        let inner = self.0.get();
        inner.grab_offset.get()
    }

    pub fn byte_at(&self, pos: Pos2) -> Option<usize> {
        let inner = self.0.get();
        inner.layout.get_untracked().index_at(pos)
    }

    pub fn measure(&self, widgets: &[TextWidget], width: f32) -> Option<Vec2> {
        let inner = self.0.get();
        let snapshot = inner.snapshot.borrow();
        if !snapshot.loaded {
            return None;
        }
        let options = RowOptions {
            body_size: super::rows::BODY_SIZE,
            mask: false,
            single_line: false,
            prose: None,
        };
        let colors = TextAreaColors::DEFAULT;
        let starts = &snapshot.starts;
        let wrap = (width - super::PADDING.x * 2.0).max(1.0);
        let mut height = DOCUMENT_PADDING.y;
        let (mut shaped_bytes, mut shaped_height, mut one_line) = (0usize, 0.0f32, f32::MAX);
        let mut held = inner.heights.borrow_mut();
        let mut heights = HashMap::with_capacity(held.len());
        for line in 0..starts.len() {
            let (start, end, newline) = line_range(&snapshot.bytes, starts, line)?;
            if snapshot.hidden.iter().any(|range| range.contains(&start)) {
                continue;
            }
            if start >= MEASURED_BYTES && shaped_bytes > 0 {
                let per_byte = shaped_height / shaped_bytes as f32;
                height += ((end - start + 1) as f32 * per_byte).max(one_line);
                continue;
            }
            let inputs = RowInputs {
                snapshot: &snapshot,
                widgets,
                client_colors: &[],
                composition: None,
                colors: &colors,
                options,
                spacers: &[],
                placeholder: None,
            };
            let key = line_key(&snapshot, widgets, start, end, wrap);
            let (row_height, block) = match held.get(&key) {
                Some(measured) => *measured,
                None => {
                    let row = build_row(&inputs, line, start, end, newline);
                    let font = FontId::proportional(options.body_size);
                    let row_height = rich_layout(&row, font, options.padding(), wrap)?.size.y;
                    (row_height, row.block.map_or(0.0, |(_, size)| size.y))
                }
            };
            heights.insert(key, (row_height, block));
            height += row_height + block;
            shaped_bytes += end - start + 1;
            shaped_height += row_height;
            one_line = one_line.min(row_height);
        }
        *held = heights;
        Some(Vec2::new(width, height))
    }

    pub fn measure_prose(
        &self,
        font_size: f32,
        prose: Prose,
        placeholder: &str,
        wrap_width: f32,
    ) -> Option<Vec2> {
        let inner = self.0.get();
        let snapshot = inner.snapshot.borrow();
        if !snapshot.loaded {
            return None;
        }
        let options = RowOptions {
            body_size: font_size,
            mask: false,
            single_line: false,
            prose: Some(prose),
        };
        let colors = TextAreaColors::DEFAULT;
        let font = FontId::proportional(font_size).bold(prose.bold);
        let wrap = match prose.wrap {
            true => wrap_width.max(1.0),
            false => f32::INFINITY,
        };
        let starts = match snapshot.starts.is_empty() {
            true => vec![0],
            false => snapshot.starts.to_vec(),
        };
        let mut size = Vec2::ZERO;
        for line in 0..starts.len() {
            let (start, end, newline) = line_range(&snapshot.bytes, &starts, line)?;
            let shown = (snapshot.bytes.is_empty() && !placeholder.is_empty() && line == 0)
                .then_some(placeholder);
            let inputs = RowInputs {
                snapshot: &snapshot,
                widgets: &[],
                client_colors: &[],
                composition: None,
                colors: &colors,
                options,
                spacers: &[],
                placeholder: shown,
            };
            let row = build_row(&inputs, line, start, end, newline);
            let laid = rich_layout(&row, font, options.padding(), wrap)?.size;
            size = Vec2::new(size.x.max(laid.x), size.y + laid.y);
        }
        Some(size)
    }

    pub fn find_open(&self) -> ReadSignal<bool> {
        let inner = self.0.get();
        inner.find.open
    }

    pub fn find_status(&self) -> FindStatus {
        let inner = self.0.get();
        inner.content.get();
        inner.cursors.get();
        let query = inner.find.query.get();
        let case_sensitive = inner.find.case_sensitive.get();
        inner.core.borrow().find_status(&query, case_sensitive)
    }

    pub fn find_summary(&self) -> String {
        let inner = self.0.get();
        let query_empty = inner.find.query.with(String::is_empty);
        match (query_empty, self.find_status()) {
            (true, FindStatus { total: 0, .. }) => String::new(),
            (false, FindStatus { total: 0, .. }) => "No results".to_owned(),
            (
                _,
                FindStatus {
                    total,
                    current: Some(current),
                },
            ) => format!("{}/{total}", current + 1),
            (
                _,
                FindStatus {
                    total,
                    current: None,
                },
            ) => format!("0/{total}"),
        }
    }

    pub fn find_has_matches(&self) -> bool {
        self.find_status().total > 0
    }

    pub fn find_has_current(&self) -> bool {
        self.find_status().current.is_some()
    }

    pub fn set_find_query(&self, query: String) {
        let inner = self.0.get();
        inner.find.set_query.set(query);
        self.sync_find();
    }

    pub fn set_find_case_sensitive(&self, case_sensitive: bool) {
        let inner = self.0.get();
        inner.find.set_case_sensitive.set(case_sensitive);
        self.sync_find();
    }

    pub fn set_find_replacement(&self, replacement: String) {
        let inner = self.0.get();
        inner.find.set_replacement.set(replacement);
    }

    pub fn open_find(&self, show_replace: bool) {
        let inner = self.0.get();
        if !inner.find.open.get_untracked() {
            let selected = self.copy(CopyMode::Copy);
            let query = match !selected.is_empty() && !selected.contains('\n') {
                true => selected,
                false => String::new(),
            };
            inner.find.set_query.set(query);
            inner.find.set_open.set(true);
        }
        inner.find.set_show_replace.set(show_replace);
        inner.find.set_focus_query.set(true);
        self.sync_find();
    }

    pub fn close_find(&self) {
        let inner = self.0.get();
        inner.find.set_open.set(false);
        inner.find.set_focus_query.set(false);
    }

    fn sync_find(&self) {
        let inner = self.0.get();
        if !inner.find.open.get_untracked() {
            return;
        }
        let (query, case_sensitive) = self.query();
        let status = inner.core.borrow().find_status(&query, case_sensitive);
        if status.current.is_none() && status.total > 0 {
            self.execute(EditorCommand::Find {
                text: &query,
                case_sensitive,
                direction: FindDirection::Next,
            });
        }
        if status.total > 0 {
            self.reveal_cursor();
        }
    }

    pub fn find_step(&self, direction: FindDirection) {
        let inner = self.0.get();
        if !inner.find.open.get_untracked() {
            return;
        }
        let (query, case_sensitive) = self.query();
        self.execute(EditorCommand::Find {
            text: &query,
            case_sensitive,
            direction,
        });
        if inner
            .core
            .borrow()
            .find_status(&query, case_sensitive)
            .total
            > 0
        {
            self.reveal_cursor();
        }
    }

    pub fn replace_match(&self) {
        let inner = self.0.get();
        let (query, case_sensitive) = self.query();
        let replacement = inner.find.replacement.get_untracked();
        self.execute(EditorCommand::ReplaceMatch {
            text: &query,
            case_sensitive,
            replacement: replacement.as_bytes(),
        });
        self.reveal_cursor();
    }

    pub fn replace_all_matches(&self) {
        let inner = self.0.get();
        let (query, case_sensitive) = self.query();
        let replacement = inner.find.replacement.get_untracked();
        self.execute(EditorCommand::ReplaceAllMatches {
            text: &query,
            case_sensitive,
            replacement: replacement.as_bytes(),
        });
    }

    pub fn find(&self) -> Find {
        self.0.with(|inner| inner.find)
    }

    fn query(&self) -> (String, bool) {
        let inner = self.0.get();
        (
            inner.find.query.get_untracked(),
            inner.find.case_sensitive.get_untracked(),
        )
    }

    fn sync_document(&self) {
        let inner = self.0.get();
        let revision = inner.core.borrow().document().revision();
        let (sections, language) = {
            let core = inner.core.borrow();
            (core.collapsible_sections(), core.language())
        };
        let loaded = inner.core.borrow().document().read().is_some();
        {
            let snapshot = inner.snapshot.borrow();
            if snapshot.highlight.is_some()
                && snapshot.loaded == loaded
                && snapshot.revision == revision
                && snapshot.language == language
                && snapshot.sections == sections
                && snapshot.runs.is_some() == inner.show_runs.get()
            {
                return;
            }
        }
        let highlight = inner.core.borrow_mut().highlight();
        let bytes = highlight
            .markdown_bytes()
            .unwrap_or_else(|| read_bytes(&inner.core.borrow()).into());
        let change = {
            let snapshot = inner.snapshot.borrow();
            (snapshot.highlight.is_some() && snapshot.loaded && snapshot.language == language)
                .then(|| {
                    inner
                        .core
                        .borrow()
                        .document()
                        .changes_since(snapshot.revision)
                })
                .flatten()
        };
        let (starts, checkboxes) = {
            let snapshot = inner.snapshot.borrow();
            match change {
                Some(change) => (
                    Rc::new(shifted_line_starts(&snapshot.starts, &bytes, change)),
                    shifted_checkboxes(&snapshot.checkboxes, &bytes, change),
                ),
                None => (
                    Rc::new(line_starts(&bytes)),
                    parse_markdown_checkboxes(&bytes),
                ),
            }
        };
        let checkboxes = match language {
            TextLanguage::Markdown => checkboxes,
            _ => Vec::new(),
        };
        let hidden = hidden_ranges(&sections);
        let runs = inner.show_runs.get().then(|| {
            let core = inner.core.borrow();
            Rc::new(
                core.document()
                    .read()
                    .map(|read| read.runs())
                    .unwrap_or_default(),
            )
        });
        *inner.snapshot.borrow_mut() = Snapshot {
            loaded,
            revision,
            starts,
            bytes,
            language,
            sections,
            hidden,
            checkboxes,
            highlight: Some(highlight),
            runs,
        };
        inner.content_counter.set(inner.content_counter.get() + 1);
        inner.set_content.set(inner.content_counter.get());
    }

    fn sync_cursors(&self) {
        let inner = self.0.get();
        let positions = {
            let core = inner.core.borrow();
            let cursors = core.cursor_positions();
            let indices = cursors
                .iter()
                .map(|cursor| {
                    (
                        core.position_index(cursor.pos.anchor),
                        core.position_index(cursor.pos.focus),
                    )
                })
                .collect();
            (cursors.to_vec(), indices, core.composition())
        };
        if *inner.cursor_cache.borrow() == positions {
            return;
        }
        *inner.cursor_cache.borrow_mut() = positions;
        inner.cursor_counter.set(inner.cursor_counter.get() + 1);
        inner.set_cursors.set(inner.cursor_counter.get());
    }
}

fn line_key(
    snapshot: &Snapshot,
    widgets: &[TextWidget],
    start: usize,
    end: usize,
    wrap: f32,
) -> u64 {
    let mut hasher = FixedState::with_seed(0).build_hasher();
    wrap.to_bits().hash(&mut hasher);
    let last = (end + 1).min(snapshot.bytes.len());
    snapshot.bytes[start..last].hash(&mut hasher);
    let mut runs = snapshot.highlight().styles_in(start..last).into_iter();
    if let Some(mut style) = runs.next() {
        let mut length = 1usize;
        for next in runs {
            if next == style {
                length += 1;
                continue;
            }
            (style, length).hash(&mut hasher);
            (style, length) = (next, 1);
        }
        (style, length).hash(&mut hasher);
    }
    for widget in widgets
        .iter()
        .filter(|widget| widget.range.start <= end && widget.range.end >= start)
    {
        (
            widget.range.start.wrapping_sub(start),
            widget.range.end.wrapping_sub(start),
        )
            .hash(&mut hasher);
        (&widget.label, widget.icon, widget.italic).hash(&mut hasher);
        widget
            .block_size
            .map(|size| (size.x.to_bits(), size.y.to_bits()))
            .hash(&mut hasher);
    }
    snapshot
        .sections
        .iter()
        .any(|section| section.collapsed && section.line_start == start)
        .hash(&mut hasher);
    for run in snapshot.runs_in(start, end) {
        (run.at - start, run.pos, run.len, run.visible, &run.deleted).hash(&mut hasher);
    }
    hasher.finish()
}

fn shifted_line_starts(starts: &[usize], bytes: &[u8], change: TextChange) -> Vec<usize> {
    if starts.is_empty() {
        return line_starts(bytes);
    }
    let kept = starts.partition_point(|start| *start <= change.start);
    let after = starts.partition_point(|start| *start <= change.old_end);
    let mut shifted = Vec::with_capacity(starts.len());
    shifted.extend_from_slice(&starts[..kept]);
    shifted.extend(
        memchr::memchr_iter(b'\n', &bytes[change.start..change.new_end])
            .map(|at| change.start + at + 1),
    );
    shifted.extend(
        starts[after..]
            .iter()
            .map(|start| start - change.old_end + change.new_end),
    );
    shifted
}

fn shifted_checkboxes(
    checkboxes: &[MarkdownCheckbox],
    bytes: &[u8],
    change: TextChange,
) -> Vec<MarkdownCheckbox> {
    let first = memchr::memrchr(b'\n', &bytes[..change.start]).map_or(0, |newline| newline + 1);
    let end = memchr::memchr(b'\n', &bytes[change.new_end..])
        .map_or(bytes.len(), |newline| change.new_end + newline);
    let old_end = end - change.new_end + change.old_end;
    let shift = |at: usize| at - change.old_end + change.new_end;
    let mut shifted: Vec<MarkdownCheckbox> = checkboxes
        .iter()
        .take_while(|checkbox| checkbox.line_start < first)
        .cloned()
        .collect();
    shifted.extend(
        parse_markdown_checkboxes(&bytes[first..end])
            .into_iter()
            .map(|checkbox| MarkdownCheckbox {
                line_start: checkbox.line_start + first,
                marker: checkbox.marker.start + first..checkbox.marker.end + first,
                checked: checkbox.checked,
            }),
    );
    shifted.extend(
        checkboxes
            .iter()
            .filter(|checkbox| checkbox.line_start > old_end)
            .map(|checkbox| MarkdownCheckbox {
                line_start: shift(checkbox.line_start),
                marker: shift(checkbox.marker.start)..shift(checkbox.marker.end),
                checked: checkbox.checked,
            }),
    );
    shifted
}

fn read_bytes(core: &Core) -> Vec<u8> {
    let Some(read) = core.document().read() else {
        return Vec::new();
    };
    read.slice(0..read.len()).into_owned()
}

pub fn hidden_ranges(sections: &[CollapsibleSection]) -> Vec<Range<usize>> {
    sections
        .iter()
        .filter(|section| section.collapsed)
        .map(|section| section.line_end + 1..section.content_end + 1)
        .collect()
}

pub fn parse_markdown_checkboxes(bytes: &[u8]) -> Vec<MarkdownCheckbox> {
    let mut result = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = memchr::memchr(b'\n', &bytes[line_start..])
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
