use std::cell::RefCell;
use std::rc::Rc;

use beui::reactive::{
    ClickCatcher, ForEach, Frame, List, Memo, ReadSignal, Text, WriteSignal, clone, component,
    component_size, create_effect, create_memo, create_signal, layout_text, untrack, view,
};
use beui::{Color32, Direction, FontId, NodeId, ScrollGesture, TextLayout};
use ghostty_vt::{Renderer, Rgb, Terminal};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;
const MAX_SCROLLBACK: usize = 10_000;
const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 6.0;
const LINE_SPACING: f32 = 1.2;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Span {
    text: String,
    color: Color32,
    background: Option<Color32>,
    bold: bool,
    italic: bool,
    underline: bool,
}

impl Span {
    fn same_style(&self, other: &Self) -> bool {
        self.color == other.color
            && self.background == other.background
            && self.bold == other.bold
            && self.italic == other.italic
            && self.underline == other.underline
    }
}

pub(crate) type Row = Vec<Span>;

pub(crate) struct Session {
    terminal: Terminal,
    renderer: Renderer,
    cols: u16,
    rows: u16,
}

#[derive(Clone)]
pub(crate) struct Pane {
    session: Rc<RefCell<Session>>,
    rows: ReadSignal<Vec<Row>>,
    set_rows: WriteSignal<Vec<Row>>,
    background: ReadSignal<Color32>,
    set_background: WriteSignal<Color32>,
}

impl Session {
    pub(crate) fn new() -> Result<Self, String> {
        let terminal = Terminal::new(DEFAULT_COLS, DEFAULT_ROWS, MAX_SCROLLBACK)
            .map_err(|error| format!("could not create the terminal emulator: {error}"))?;
        let renderer = Renderer::new()
            .map_err(|error| format!("could not create the terminal renderer: {error}"))?;
        Ok(Self {
            terminal,
            renderer,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        })
    }
}

impl Pane {
    pub(crate) fn new(session: Session) -> Self {
        let (rows, set_rows) = create_signal(Vec::new());
        let (background, set_background) = create_signal(Color32::BLACK);
        Self {
            session: Rc::new(RefCell::new(session)),
            rows,
            set_rows,
            background,
            set_background,
        }
    }

    pub(crate) fn write(&self, bytes: &[u8]) {
        self.session
            .borrow_mut()
            .terminal
            .write(&with_carriage_returns(bytes));
    }

    pub(crate) fn write_line(&self, text: &str) {
        self.write(format!("{text}\n").as_bytes());
    }

    pub(crate) fn refresh(&self) {
        let mut session = self.session.borrow_mut();
        let Session {
            terminal, renderer, ..
        } = &mut *session;
        let Ok(screen) = renderer.update(terminal) else {
            return;
        };
        let rows = screen
            .rows
            .iter()
            .map(|row| {
                let mut spans: Vec<Span> = Vec::new();
                for cell in &row.cells {
                    let span = Span {
                        text: if cell.text.is_empty() {
                            " ".to_owned()
                        } else {
                            cell.text.clone()
                        },
                        color: color(cell.foreground),
                        background: cell.background.map(color),
                        bold: cell.bold,
                        italic: cell.italic,
                        underline: cell.underline || cell.strikethrough,
                    };
                    match spans.last_mut() {
                        Some(last) if last.same_style(&span) => last.text.push_str(&span.text),
                        _ => spans.push(span),
                    }
                }
                spans
            })
            .collect();
        let background = color(screen.background);
        drop(session);
        self.set_rows.set(rows);
        self.set_background.set(background);
    }

    fn resize(&self, cols: u16, rows: u16, cell_width: u32, cell_height: u32) {
        {
            let mut session = self.session.borrow_mut();
            if session.cols == cols && session.rows == rows {
                return;
            }
            session.cols = cols;
            session.rows = rows;
            let _ = session.terminal.resize(cols, rows, cell_width, cell_height);
        }
        self.refresh();
    }

    fn scroll(&self, rows: isize) {
        self.session.borrow_mut().terminal.scroll_by(rows);
        self.refresh();
    }
}

pub(crate) fn with_carriage_returns(bytes: &[u8]) -> Vec<u8> {
    let mut converted = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        if byte == b'\n' {
            converted.push(b'\r');
        }
        converted.push(byte);
    }
    converted
}

fn color(color: Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

#[component]
pub(crate) fn TerminalPane(pane: Pane) -> NodeId {
    let size = component_size();
    create_effect(clone!(pane -> move || {
        let size = size.get();
        let Some(cell) = layout_text("M", FontId::monospace(FONT_SIZE), TextLayout::DEFAULT)
            .map(|galley| galley.size())
        else {
            return;
        };
        if cell.x <= 0.0 || cell.y <= 0.0 {
            return;
        }
        let cols = ((size.x - 2.0 * PADDING) / cell.x).floor().max(1.0) as u16;
        let rows = ((size.y - 2.0 * PADDING) / cell.y).floor().max(1.0) as u16;
        untrack(|| pane.resize(cols, rows, cell.x as u32, cell.y as u32));
    }));
    let rows = pane.rows.clone();
    let keys = create_memo(clone!(rows -> move || (0..rows.with(Vec::len)).collect::<Vec<_>>()));
    let background = pane.background.clone();
    view! {
        <ClickCatcher
            on_scroll={move |gesture: ScrollGesture| {
                let rows = (gesture.delta.y / (FONT_SIZE * LINE_SPACING)).round() as isize;
                if rows != 0 {
                    pane.scroll(-rows);
                }
            }}
        >
            <Frame color={background} padding_horizontal=PADDING padding_vertical=PADDING>
                <List spacing=0.0>
                    <ForEach keys>
                        {move |index: usize| {
                            let rows = rows.clone();
                            let row = create_memo(move || rows.with(|rows| rows.get(index).cloned()).unwrap_or_default());
                            view! {
                                <TerminalLine row />
                            }
                        }}
                    </ForEach>
                </List>
            </Frame>
        </ClickCatcher>
    }
}

#[component]
fn TerminalLine(row: Memo<Row>) -> NodeId {
    let keys = create_memo(clone!(row -> move || (0..row.with(Vec::len)).collect::<Vec<_>>()));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys>
                {move |index: usize| {
                    let row = row.clone();
                    let span = create_memo(move || row.with(|row| row.get(index).cloned()));
                    view! {
                        <TerminalSpan span />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TerminalSpan(span: Memo<Option<Span>>) -> NodeId {
    let field = |read: fn(&Span) -> bool| {
        let span = span.clone();
        create_memo(move || span.with(|span| span.as_ref().is_some_and(read)))
    };
    let bold = field(|span| span.bold);
    let italic = field(|span| span.italic);
    let underline = field(|span| span.underline);
    let text = create_memo(clone!(span -> move || {
        span.with(|span| span.as_ref().map(|span| span.text.clone()).unwrap_or_default())
    }));
    let color = create_memo(clone!(span -> move || {
        span.with(|span| span.as_ref().map_or(Color32::WHITE, |span| span.color))
    }));
    let background = create_memo(move || {
        span.with(|span| span.as_ref().and_then(|span| span.background))
            .unwrap_or(Color32::TRANSPARENT)
    });
    view! {
        <Frame color={background}>
            <Text string={text} font_size=FONT_SIZE color monospace=true bold italic underline />
        </Frame>
    }
}
