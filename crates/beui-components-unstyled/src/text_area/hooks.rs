use beui_macros::{component, view};

use beui_view::components::text::Span;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Align;
use beui_view::types::Color;
use beui_view::types::Corners;
use beui_view::types::{SpanBackground, SpanStyle};
use text_editor_core::schema::basic;
use text_editor_core::{LineLook, ListMarker, MarkSet, SynHlStyle, SyntaxHighlight, TableRow};

use super::rows::RowTable;
use super::state::TextAreaState;
use beui_view::types::{Align2, Extent, Sides};

pub const MARKER_COLUMN_EM: f32 = 1.15;
pub const MARKER_GAP_EM: f32 = 0.55;
pub const LIST_INDENT_EM: f32 = MARKER_COLUMN_EM + MARKER_GAP_EM;
pub const QUOTE_INDENT_EM: f32 = 1.4;
pub const MARGIN_EM: f32 = 3.1;
pub const MARGIN_MIN_WIDTH_EM: f32 = 40.0;
pub const COLUMN_MAX_EM: f32 = 48.0;
pub const CODE_PADDING_EM: f32 = 1.0;
pub const BULLET_EM: f32 = 0.375;

#[derive(Clone)]
pub struct TextLeaf {
    pub text: String,
    pub style: SpanStyle,
    pub syntax: SynHlStyle,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MarkerKind {
    Bullet,
    Number(usize),
    Check(bool),
    Void(MarkSet),
}

impl From<ListMarker> for MarkerKind {
    fn from(list: ListMarker) -> Self {
        match list {
            ListMarker::Bullet => MarkerKind::Bullet,
            ListMarker::Number(number) => MarkerKind::Number(number),
            ListMarker::Check(checked) => MarkerKind::Check(checked),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextMarker {
    pub state: TextAreaState,
    pub line: usize,
    pub kind: Memo<MarkerKind>,
    pub label: Memo<String>,
    pub depth: Memo<usize>,
    pub style: Memo<SpanStyle>,
    pub checked: Binding<bool>,
    pub disabled: Memo<bool>,
}

impl TextMarker {
    pub fn number(&self) -> Memo<String> {
        let (kind, label) = (self.kind, self.label);
        create_memo(move || match kind.get() {
            MarkerKind::Number(_) if !label.get().is_empty() => label.get(),
            MarkerKind::Number(number) => format!("{number}."),
            _ => String::new(),
        })
    }

    pub fn hollow(&self) -> Memo<bool> {
        let depth = self.depth;
        create_memo(move || depth.get() % 2 == 1)
    }

    pub fn written(&self) -> Memo<bool> {
        let label = self.label;
        create_memo(move || !label.get().is_empty())
    }

    pub fn gap(&self) -> Memo<Sides> {
        let style = self.style;
        create_memo(move || Sides::new(0.0, 0.0, style.get().font.size * MARKER_GAP_EM, 0.0))
    }

    pub fn bullet(&self) -> Memo<f32> {
        let style = self.style;
        create_memo(move || style.get().font.size * BULLET_EM)
    }
}

#[component]
pub fn MarkerColumn(marker: TextMarker, content: NodeId) -> NodeId {
    let style = marker.style;
    let width = create_memo(move || {
        Extent::at_least(style.get().font.size * (MARKER_COLUMN_EM + MARKER_GAP_EM))
    });
    let gap = marker.gap();
    view! {
        <Frame width padding=gap align={Align2::new(Align::End, Align::Start)}>{content}</Frame>
    }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Margin {
    pub left: f32,
    pub column: f32,
    pub right: f32,
    pub hangs: bool,
}

impl Margin {
    pub fn centered(width: f32, size: f32, hangs: bool) -> Self {
        let lead = match hangs {
            true => size * MARGIN_EM,
            false => 0.0,
        };
        let column = (width - lead).min(size * COLUMN_MAX_EM).max(0.0);
        let left = ((width - column) / 2.0).max(lead);
        Self {
            left,
            column,
            right: (width - column - left).max(0.0),
            hangs,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextLine {
    pub state: TextAreaState,
    pub line: Memo<usize>,
    pub start: Memo<usize>,
    pub look: Memo<LineLook>,
    pub padding: Memo<Sides>,
    pub size: Memo<f32>,
    pub margin: Memo<Margin>,
    pub outdent: Memo<f32>,
    pub space: Memo<(f32, f32)>,
    pub table: Memo<Option<RowTable>>,
    pub text: NodeId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct QuoteBar {
    pub level: usize,
    pub opens: bool,
    pub closes: bool,
}

impl TextLine {
    pub fn quotes(&self) -> Memo<Vec<QuoteBar>> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            (0..look.quote)
                .map(|level| QuoteBar {
                    level,
                    opens: level >= look.quote_above,
                    closes: level >= look.quote_below,
                })
                .collect()
        })
    }

    pub fn quote_indent(&self) -> Memo<f32> {
        let size = self.size;
        create_memo(move || size.get() * QUOTE_INDENT_EM)
    }

    pub fn column(&self) -> Memo<f32> {
        let (padding, margin) = (self.padding, self.margin);
        create_memo(move || padding.get().left + margin.get().left)
    }

    pub fn right(&self) -> Memo<f32> {
        let (padding, margin, table, size) = (self.padding, self.margin, self.table, self.size);
        let left = self.left();
        create_memo(move || {
            let margin = margin.get();
            let padding = padding.get();
            let end = padding.left + margin.left + margin.column;
            let wanted = table
                .get()
                .and_then(|table| table.edges.last().map(|edge| edge.0))
                .map_or(0.0, |last| left.get() + last + size.get() - end);
            padding.right + (margin.right - wanted.max(0.0)).max(0.0)
        })
    }

    pub fn indent(&self) -> Memo<f32> {
        let (look, size, margin, outdent) = (self.look, self.size, self.margin, self.outdent);
        create_memo(move || {
            let look = look.get();
            let listed = look.list.is_some() || look.continues;
            let levels = match listed {
                true => look.depth as f32 + f32::from(u8::from(look.continues)),
                false => 0.0,
            };
            let margin = margin.get();
            let hung = listed && look.quote == 0 && margin.hangs;
            let nested = (levels - f32::from(u8::from(hung))) * LIST_INDENT_EM;
            let indent = (look.quote as f32 * QUOTE_INDENT_EM + nested) * size.get();
            margin.left + indent - outdent.get()
        })
    }

    pub fn left(&self) -> Memo<f32> {
        let (padding, indent) = (self.padding, self.indent());
        create_memo(move || (padding.get().left + indent.get()).max(0.0))
    }

    pub fn code_padding(&self) -> Memo<f32> {
        let (look, size, margin) = (self.look, self.size, self.margin);
        create_memo(move || match look.get().code_block && margin.get().hangs {
            true => size.get() * CODE_PADDING_EM,
            false => 0.0,
        })
    }

    pub fn opens_code(&self) -> Memo<bool> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            look.code_block && !look.code_above
        })
    }

    pub fn code_language(&self) -> Memo<String> {
        let (state, start) = (self.state, self.start);
        let content = state.content();
        create_memo(move || {
            content.get();
            let start = start.get();
            state.with_snapshot(|snapshot| {
                snapshot
                    .formats
                    .as_deref()
                    .and_then(|formats| formats.block_at(start))
                    .and_then(|block| block.props.get(basic::CODE_BLOCK).cloned())
                    .and_then(|language| language.as_text().map(str::to_owned))
                    .unwrap_or_default()
            })
        })
    }

    pub fn code_text(&self) -> String {
        let start = self.start.get_untracked();
        self.state
            .with_snapshot(|snapshot| code_block_text(&snapshot.bytes, snapshot.highlight(), start))
    }

    pub fn code_corners(&self, radius: f32) -> Memo<Corners> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            Corners::new(
                if look.code_above { 0.0 } else { radius },
                if look.code_above { 0.0 } else { radius },
                if look.code_below { 0.0 } else { radius },
                if look.code_below { 0.0 } else { radius },
            )
        })
    }
}

fn code_block_text(bytes: &[u8], highlight: &SyntaxHighlight, start: usize) -> String {
    let mut lines = Vec::new();
    let mut at = start.min(bytes.len());
    loop {
        let end = memchr::memchr(b'\n', &bytes[at..]).map_or(bytes.len(), |newline| at + newline);
        let look = highlight.line_look(at);
        lines.push(&bytes[look.marker.end.clamp(at, end)..end]);
        if !look.code_below || end >= bytes.len() {
            break;
        }
        at = end + 1;
    }
    let fence = |line: &[u8]| {
        let trimmed = line.trim_ascii_start();
        trimmed.starts_with(b"```") || trimmed.starts_with(b"~~~")
    };
    let indent = match lines.first() {
        Some(first) if fence(first) => {
            let indent = first.len() - first.trim_ascii_start().len();
            lines.remove(0);
            if lines.last().is_some_and(|last| fence(last)) {
                lines.pop();
            }
            indent
        }
        _ => lines
            .iter()
            .filter(|line| !line.trim_ascii().is_empty())
            .map(|line| line.len() - line.trim_ascii_start().len())
            .min()
            .unwrap_or(0),
    };
    let lines: Vec<String> = lines
        .iter()
        .map(|line| {
            let strip = line.len() - line.trim_ascii_start().len();
            String::from_utf8_lossy(&line[strip.min(indent)..]).into_owned()
        })
        .collect();
    lines.join("\n")
}

#[component]
pub fn TableFill(line: TextLine, fill: Memo<Color>) -> NodeId {
    let left = line.left();
    let table = line.table;
    let span = create_memo(move || {
        let table = table.get()?;
        let first = table.edges.first()?.0;
        let last = table.edges.last()?.0;
        Some((
            left.get() + first,
            last - first,
            table.row == TableRow::Header,
        ))
    });
    let padding =
        create_memo(move || Sides::new(span.get().map_or(0.0, |span| span.0), 0.0, 0.0, 0.0));
    let width = create_memo(move || Extent::px(span.get().map_or(0.0, |span| span.1)));
    let color = create_memo(move || match span.get().is_some_and(|span| span.2) {
        true => fill.get(),
        false => Color::TRANSPARENT,
    });
    view! {
        <Frame padding align={Align2::new(Align::Start, Align::Stretch)}>
            <Frame width fill=color />
        </Frame>
    }
}

#[component]
pub fn TableLines(line: TextLine, color: Memo<Color>, rule: f32) -> NodeId {
    let left = line.left();
    let table = line.table;
    let edges = create_memo(move || {
        let left = left.get();
        table.get().map_or_else(Vec::new, |table| {
            table
                .edges
                .iter()
                .filter(|(_, drawn)| *drawn)
                .map(|(edge, _)| (left + edge).to_bits())
                .collect::<Vec<u32>>()
        })
    });
    let span = create_memo(move || {
        let table = table.get()?;
        let first = table.edges.first()?.0;
        let last = table.edges.last()?.0;
        Some((
            left.get() + first,
            last - first + rule,
            table.row == TableRow::Header,
        ))
    });
    let padding =
        create_memo(move || Sides::new(span.get().map_or(0.0, |span| span.0), 0.0, 0.0, 0.0));
    let width = create_memo(move || Extent::px(span.get().map_or(0.0, |span| span.1)));
    let top = create_memo(move || match span.get().is_some_and(|span| span.2) {
        true => color.get(),
        false => Color::TRANSPARENT,
    });
    let bottom = create_memo(move || match span.get() {
        Some(_) => color.get(),
        None => Color::TRANSPARENT,
    });
    view! {
        <Layers>
            <Layers>
                <ForEach keys={edges}>|edge: u32|
                    <Frame
                        padding={Sides::new(f32::from_bits(edge), 0.0, 0.0, 0.0)}
                        align={Align2::new(Align::Start, Align::Stretch)}
                    >
                        <Frame width={Extent::px(rule)} fill=color />
                    </Frame>
                </ForEach>
            </Layers>
            <Frame padding align={Align2::new(Align::Start, Align::Start)}>
                <Frame width height={Extent::px(rule)} fill=top />
            </Frame>
            <Frame padding align={Align2::new(Align::Start, Align::End)}>
                <Frame width height={Extent::px(rule)} fill=bottom />
            </Frame>
        </Layers>
    }
}

impl QuoteBar {
    pub fn corners(self, radius: f32) -> Corners {
        let top = if self.opens { radius } else { 0.0 };
        let bottom = if self.closes { radius } else { 0.0 };
        Corners::new(top, top, bottom, bottom)
    }
}

#[component]
pub fn LeafSpan(
    leaf: TextLeaf,
    #[prop(default = None)] background: Prop<Option<SpanBackground>>,
) -> Span {
    let TextLeaf { text, style, .. } = leaf;
    view! {
        <Span
            font={Some(style.font)}
            color={Some(style.color)}
            underline={Some(style.underline)}
            strikethrough={Some(style.strikethrough)}
            background
        >
            {text}
        </Span>
    }
}

#[component]
pub fn PlainLine(line: TextLine) -> NodeId {
    let (left, right, code) = (line.left(), line.right(), line.code_padding());
    let padding = create_memo(move || {
        let (above, below) = line.space.get();
        Sides::new(
            left.get() + code.get(),
            above,
            right.get() + code.get(),
            below,
        )
    });
    let text = line.text;
    view! {
        <Frame padding>{text}</Frame>
    }
}
