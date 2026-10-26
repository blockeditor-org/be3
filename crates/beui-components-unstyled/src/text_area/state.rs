use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use text_editor_core::{
    CollapsibleSection, CopyMode, Core, CursorPosition, EditorCommand, FindDirection, FindStatus,
    Formats, MarkdownCommand, Position, Schema, Selected, SyntaxHighlight, TextChange,
    TextIndentation, TextLanguage, TextRun,
};

use beui_core::rich::{Piece, RichLayout, RichOptions};
use beui_view::reactive::*;
use beui_view::types::FontId;
use beui_view::types::SpanStyle;
use beui_view::types::{Pos2, Rect, Vec2};

use super::colors::TextAreaColors;
use super::look::TextAreaGeometry;
use super::rows::{Prose, RowInputs, RowOptions, build_row, line_range, line_starts};

pub struct ProseLine {
    pub layout: RichLayout,
    pub quote: usize,
}

#[derive(Clone, Copy)]
pub enum Grab {
    Selection(Position),
    Caret,
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
    pub highlight: Option<SyntaxHighlight>,
    pub runs: Option<Rc<Vec<TextRun>>>,
    pub formats: Option<Arc<Formats>>,
}

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
    pub open: Signal<bool>,
    pub query: Signal<String>,
    pub replacement: Signal<String>,
    pub case_sensitive: Signal<bool>,
    pub show_replace: Signal<bool>,
    pub focus_query: Signal<bool>,
}

impl Find {
    fn new() -> Self {
        let open = create_signal(false);
        let query = create_signal(String::new());
        let replacement = create_signal(String::new());
        let case_sensitive = create_signal(false);
        let show_replace = create_signal(false);
        let focus_query = create_signal(false);
        Self {
            open,
            query,
            replacement,
            case_sensitive,
            show_replace,
            focus_query,
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
    find: Find,
    canvas: NodeRef,
    cursor_cache: RefCell<CursorCache>,
    selecting: Cell<bool>,
    show_runs: Cell<bool>,
    touch_mode: Signal<bool>,
    caret_handle: Signal<bool>,
    grab: Cell<Option<Grab>>,
    grab_offset: Cell<Vec2>,
    reveal_cursor: Cell<bool>,
    tab_released: Cell<bool>,
    reveal: Cell<Option<Rect>>,
    reveals: Signal<u64>,
    focus_requests: Signal<u64>,
    content: Signal<u64>,
    content_counter: Cell<u64>,
    cursors: Signal<u64>,
    cursor_counter: Cell<u64>,
    selected: Signal<Option<Selected>>,
    rows_built: Cell<u64>,
    layout: Signal<TextAreaLayout>,
}

#[derive(Clone, Copy)]
pub struct TextAreaState(Stored<Inner>);

impl TextAreaState {
    pub fn new(document: Arc<dyn text_editor_core::Document>) -> Self {
        Self::from_core(Core::new(document))
    }

    pub fn with_schema(
        document: Arc<dyn text_editor_core::Document>,
        schema: Arc<dyn Schema>,
    ) -> Self {
        let mut core = Core::new(document);
        core.set_schema(schema);
        Self::from_core(core)
    }

    fn from_core(mut core: Core) -> Self {
        let start = core.position(0);
        core.execute_command(EditorCommand::SetSelection {
            anchor: start,
            focus: start,
        });
        let content = create_signal(0);
        let cursors = create_signal(0);
        let layout = create_signal(TextAreaLayout::default());
        let reveals = create_signal(0);
        let focus_requests = create_signal(0);
        let touch_mode = create_signal(false);
        let caret_handle = create_signal(false);
        let selected = create_signal(None);
        let state = Self(store(Inner {
            core: RefCell::new(core),
            snapshot: RefCell::new(Snapshot::default()),
            find: Find::new(),
            canvas: NodeRef::new(),
            cursor_cache: RefCell::new((Vec::new(), Vec::new(), None)),
            selecting: Cell::new(false),
            show_runs: Cell::new(false),
            touch_mode,
            caret_handle,
            grab: Cell::new(None),
            grab_offset: Cell::new(Vec2::ZERO),
            reveal_cursor: Cell::new(false),
            tab_released: Cell::new(false),
            reveal: Cell::new(None),
            reveals,
            focus_requests,
            content,
            content_counter: Cell::new(0),
            cursors,
            cursor_counter: Cell::new(0),
            selected,
            rows_built: Cell::new(0),
            layout,
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
        let inner = self.0.value();
        inner.core.borrow_mut().execute_command(command);
        self.sync();
    }

    pub fn copy(&self, mode: CopyMode) -> String {
        let inner = self.0.value();
        let text = inner.core.borrow_mut().copy_utf8(mode);
        self.sync();
        text
    }

    pub fn external_edit(&self) {
        let inner = self.0.value();
        inner.core.borrow_mut().external_edit();
        self.sync();
    }

    pub fn sync(&self) {
        self.sync_document();
        self.sync_cursors();
        self.sync_selected();
    }

    pub fn selected(&self) -> ReadSignal<Option<Selected>> {
        let inner = self.0.value();
        inner.selected.read_only()
    }

    pub fn schema(&self) -> Arc<dyn Schema> {
        self.with_core(Core::schema)
    }

    pub fn set_checked(&self, line_start: usize, checked: bool) {
        let rich = self.with_snapshot(|snapshot| snapshot.language == TextLanguage::RichText);
        let at = self.with_core(|core| core.position(line_start));
        let command = match rich {
            true => {
                let Some((key, value)) = self.schema().check(checked) else {
                    return;
                };
                EditorCommand::SetBlock {
                    at: Some(at),
                    key,
                    value,
                }
            }
            false => EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(at)),
        };
        self.execute(command);
    }

    fn sync_selected(&self) {
        let inner = self.0.value();
        let selected = inner.core.borrow().selected();
        if inner.selected.get_untracked() != selected {
            inner.selected.set(selected);
        }
    }

    pub fn set_show_runs(&self, shown: bool) {
        let inner = self.0.value();
        inner.show_runs.set(shown);
        self.sync_document();
    }

    pub fn content(&self) -> ReadSignal<u64> {
        let inner = self.0.value();
        inner.content.read_only()
    }

    pub fn rows_built(&self) -> u64 {
        self.0.with(|inner| inner.rows_built.get())
    }

    pub(super) fn count_row_built(&self) {
        self.0
            .with(|inner| inner.rows_built.set(inner.rows_built.get() + 1));
    }

    pub fn cursors(&self) -> ReadSignal<u64> {
        let inner = self.0.value();
        inner.cursors.read_only()
    }

    pub fn layout(&self) -> ReadSignal<TextAreaLayout> {
        let inner = self.0.value();
        inner.layout.read_only()
    }

    pub fn loaded(&self) -> bool {
        let inner = self.0.value();
        inner.snapshot.borrow().loaded
    }

    pub fn bytes(&self) -> Arc<[u8]> {
        self.0.with(|inner| inner.snapshot.borrow().bytes.clone())
    }

    pub fn language(&self) -> TextLanguage {
        let inner = self.0.value();
        inner.core.borrow().language()
    }

    pub fn indentation(&self) -> TextIndentation {
        let inner = self.0.value();
        inner.core.borrow().indentation()
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        let inner = self.0.value();
        let core = inner.core.borrow();
        core.cursor_positions()
            .iter()
            .filter_map(|cursor| core.selection_range(cursor))
            .collect()
    }

    pub fn caret_indices(&self) -> Vec<usize> {
        let inner = self.0.value();
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
        let inner = self.0.value();
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
        let inner = self.0.value();
        inner.reveal_cursor.set(true);
        self.request_reveal();
    }

    pub fn reveal(&self, rect: Rect) {
        let inner = self.0.value();
        inner.reveal.set(Some(rect));
        self.request_reveal();
    }

    fn request_reveal(&self) {
        let inner = self.0.value();
        inner.reveals.update(|reveals| *reveals += 1);
    }

    pub fn focus(&self) {
        let inner = self.0.value();
        inner.focus_requests.update(|count| *count += 1);
    }

    pub fn focus_requests(&self) -> ReadSignal<u64> {
        let inner = self.0.value();
        inner.focus_requests.read_only()
    }

    pub fn reveals(&self) -> ReadSignal<u64> {
        let inner = self.0.value();
        inner.reveals.read_only()
    }

    pub fn canvas(&self) -> NodeRef {
        let inner = self.0.value();
        inner.canvas
    }

    pub fn take_reveal_cursor(&self) -> bool {
        let inner = self.0.value();
        inner.reveal_cursor.replace(false)
    }

    pub fn take_reveal(&self) -> Option<Rect> {
        let inner = self.0.value();
        inner.reveal.take()
    }

    pub fn with_snapshot<R>(&self, read: impl FnOnce(&Snapshot) -> R) -> R {
        let inner = self.0.value();
        read(&inner.snapshot.borrow())
    }

    pub fn publish_layout(&self, layout: TextAreaLayout) {
        let inner = self.0.value();
        inner.layout.set(layout);
    }

    pub fn sections(&self) -> Vec<CollapsibleSection> {
        let inner = self.0.value();
        inner.snapshot.borrow().sections.clone()
    }

    pub fn selecting(&self) -> bool {
        let inner = self.0.value();
        inner.selecting.get()
    }

    pub fn set_selecting(&self, selecting: bool) {
        let inner = self.0.value();
        inner.selecting.set(selecting);
    }

    pub fn touch_mode(&self) -> ReadSignal<bool> {
        let inner = self.0.value();
        inner.touch_mode.read_only()
    }

    pub fn set_touch_mode(&self, touch: bool) {
        let inner = self.0.value();
        inner.touch_mode.set(touch);
    }

    pub fn caret_handle(&self) -> ReadSignal<bool> {
        let inner = self.0.value();
        inner.caret_handle.read_only()
    }

    pub fn release_tab(&self, released: bool) -> bool {
        let inner = self.0.value();
        inner.tab_released.replace(released)
    }

    pub fn set_caret_handle(&self, shown: bool) {
        let inner = self.0.value();
        inner.caret_handle.set(shown);
    }

    pub fn grab(&self) -> Option<Grab> {
        let inner = self.0.value();
        inner.grab.get()
    }

    pub fn begin_grab(&self, grab: Grab, offset: Vec2) {
        let inner = self.0.value();
        inner.grab.set(Some(grab));
        inner.grab_offset.set(offset);
        inner.selecting.set(false);
    }

    pub fn end_grab(&self) -> Option<Grab> {
        let inner = self.0.value();
        inner.grab_offset.set(Vec2::ZERO);
        inner.grab.take()
    }

    pub fn grab_offset(&self) -> Vec2 {
        let inner = self.0.value();
        inner.grab_offset.get()
    }

    pub fn byte_at(&self, pos: Pos2) -> Option<usize> {
        let inner = self.0.value();
        inner.layout.get_untracked().index_at(pos)
    }

    pub fn lay_out_prose(
        &self,
        font_size: f32,
        prose: Prose,
        placeholder: &str,
        wrap_width: f32,
        colors: &TextAreaColors,
        geometry: TextAreaGeometry,
        square: f32,
        mut lay_out: impl FnMut(&[Piece], RichOptions) -> Option<RichLayout>,
    ) -> Option<Vec<ProseLine>> {
        let inner = self.0.value();
        let snapshot = inner.snapshot.borrow();
        if !snapshot.loaded {
            return None;
        }
        let options = RowOptions {
            body_size: font_size,
            mask: false,
            single_line: false,
            prose: Some(prose),
            geometry,
        };
        let font = FontId::proportional(font_size).bold(prose.bold);
        let wrap = match prose.wrap {
            true => wrap_width.max(1.0),
            false => f32::INFINITY,
        };
        let starts = match snapshot.starts.is_empty() {
            true => vec![0],
            false => snapshot.starts.to_vec(),
        };
        let mut lines = Vec::with_capacity(starts.len());
        for line in 0..starts.len() {
            let (start, end, newline) = line_range(&snapshot.bytes, &starts, line)?;
            let shown = (snapshot.bytes.is_empty() && !placeholder.is_empty() && line == 0)
                .then_some(placeholder);
            let inputs = RowInputs {
                snapshot: &snapshot,
                widgets: &[],
                client_colors: &[],
                composition: None,
                colors,
                options,
                placeholder: shown,
                columns: None,
            };
            let row = build_row(&inputs, line, start, end, newline);
            let quote = row.look.quote;
            let options = RichOptions {
                wrap_width: (wrap - quote as f32 * square).max(1.0),
                padding: options.padding(),
                style: SpanStyle::new(font, colors.syntax.markdown_plain_text),
                hang: row.hang,
                anywhere: false,
                least: false,
            };
            lines.push(ProseLine {
                layout: lay_out(&row.pieces(Vec2::splat(square)), options)?,
                quote,
            });
        }
        Some(lines)
    }

    pub fn find_open(&self) -> ReadSignal<bool> {
        let inner = self.0.value();
        inner.find.open.read_only()
    }

    pub fn find_status(&self) -> FindStatus {
        let inner = self.0.value();
        inner.content.get();
        inner.cursors.get();
        let query = inner.find.query.get();
        let case_sensitive = inner.find.case_sensitive.get();
        inner.core.borrow().find_status(&query, case_sensitive)
    }

    pub fn find_summary(&self) -> String {
        let inner = self.0.value();
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
        let inner = self.0.value();
        inner.find.query.set(query);
        self.sync_find();
    }

    pub fn set_find_case_sensitive(&self, case_sensitive: bool) {
        let inner = self.0.value();
        inner.find.case_sensitive.set(case_sensitive);
        self.sync_find();
    }

    pub fn set_find_replacement(&self, replacement: String) {
        let inner = self.0.value();
        inner.find.replacement.set(replacement);
    }

    pub fn open_find(&self, show_replace: bool) {
        let inner = self.0.value();
        if !inner.find.open.get_untracked() {
            let selected = self.copy(CopyMode::Copy);
            let query = match !selected.is_empty() && !selected.contains('\n') {
                true => selected,
                false => String::new(),
            };
            inner.find.query.set(query);
            inner.find.open.set(true);
        }
        inner.find.show_replace.set(show_replace);
        inner.find.focus_query.set(true);
        self.sync_find();
    }

    pub fn close_find(&self) {
        let inner = self.0.value();
        inner.find.open.set(false);
        inner.find.focus_query.set(false);
    }

    fn sync_find(&self) {
        let inner = self.0.value();
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
        let inner = self.0.value();
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
        let inner = self.0.value();
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
        let inner = self.0.value();
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
        let inner = self.0.value();
        (
            inner.find.query.get_untracked(),
            inner.find.case_sensitive.get_untracked(),
        )
    }

    fn sync_document(&self) {
        let inner = self.0.value();
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
            .bytes()
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
        let starts = {
            let snapshot = inner.snapshot.borrow();
            match change {
                Some(change) => Rc::new(shifted_line_starts(&snapshot.starts, &bytes, change)),
                None => Rc::new(line_starts(&bytes)),
            }
        };
        let hidden = hidden_ranges(&sections);
        let formats = (language == TextLanguage::RichText)
            .then(|| {
                let core = inner.core.borrow();
                core.document().read().and_then(|read| read.formats())
            })
            .flatten();
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
            highlight: Some(highlight),
            runs,
            formats,
        };
        inner.content_counter.set(inner.content_counter.get() + 1);
        inner.content.set(inner.content_counter.get());
    }

    fn sync_cursors(&self) {
        let inner = self.0.value();
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
        inner.cursors.set(inner.cursor_counter.get());
    }
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
