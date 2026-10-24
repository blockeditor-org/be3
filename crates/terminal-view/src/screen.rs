use std::cell::RefCell;

use beui::Color32;
use beui::reactive::*;
use ghostty_vt::{Renderer, Rgb, Terminal};

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: Color32,
    pub background: Option<Color32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
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

pub type Row = Vec<Span>;

pub struct Emulator {
    terminal: Terminal,
    renderer: Renderer,
    cols: u16,
    rows: u16,
    cursor: bool,
}

impl Emulator {
    pub fn new(cols: u16, rows: u16, scrollback: usize) -> Result<Self, String> {
        let terminal = Terminal::new(cols, rows, scrollback)
            .map_err(|error| format!("could not create the terminal emulator: {error}"))?;
        let renderer = Renderer::new()
            .map_err(|error| format!("could not create the terminal renderer: {error}"))?;
        Ok(Self {
            terminal,
            renderer,
            cols,
            rows,
            cursor: false,
        })
    }

    pub fn with_cursor(mut self) -> Self {
        self.cursor = true;
        self
    }

    pub fn on_reply(&mut self, reply: impl FnMut(&[u8]) + 'static) -> Result<(), String> {
        self.terminal
            .on_reply(reply)
            .map_err(|error| format!("could not answer the terminal's queries: {error}"))
    }
}

#[derive(Clone, Copy)]
pub struct TerminalScreen {
    state: Stored<RefCell<Emulator>>,
    rows: Signal<Vec<Row>>,
    background: Signal<Color32>,
}

impl TerminalScreen {
    pub fn new(emulator: Emulator) -> Self {
        Self {
            state: store(RefCell::new(emulator)),
            rows: create_signal(Vec::new()),
            background: create_signal(Color32::BLACK),
        }
    }

    pub fn rows(&self) -> ReadSignal<Vec<Row>> {
        self.rows.read_only()
    }

    pub fn background(&self) -> ReadSignal<Color32> {
        self.background.read_only()
    }

    pub fn size(&self) -> (u16, u16) {
        let state = self.state.value();
        let state = state.borrow();
        (state.cols, state.rows)
    }

    pub fn write(&self, bytes: &[u8]) {
        self.state.value().borrow_mut().terminal.write(bytes);
    }

    pub fn reset(&self) {
        self.state.value().borrow_mut().terminal.reset();
    }

    pub fn resize(&self, cols: u16, rows: u16, cell_width: u32, cell_height: u32) -> bool {
        let state = self.state.value();
        let mut state = state.borrow_mut();
        if state.cols == cols && state.rows == rows {
            return false;
        }
        state.cols = cols;
        state.rows = rows;
        let _ = state.terminal.resize(cols, rows, cell_width, cell_height);
        true
    }

    pub fn scroll(&self, rows: isize) {
        self.state.value().borrow_mut().terminal.scroll_by(rows);
        self.refresh();
    }

    pub fn scroll_to_bottom(&self) {
        self.state.value().borrow_mut().terminal.scroll_to_bottom();
    }

    pub fn text(&self) -> String {
        let state = self.state.value();
        let mut state = state.borrow_mut();
        let Emulator {
            terminal, renderer, ..
        } = &mut *state;
        renderer
            .update(terminal)
            .map(|screen| screen.text())
            .unwrap_or_default()
    }

    pub fn show(&self, rows: Vec<Row>, background: Color32) {
        self.rows.set(rows);
        self.background.set(background);
    }

    pub fn refresh(&self) {
        let state = self.state.value();
        let mut state = state.borrow_mut();
        let Emulator {
            terminal,
            renderer,
            cursor: draws_cursor,
            ..
        } = &mut *state;
        let Ok(screen) = renderer.update(terminal) else {
            return;
        };
        let cursor = screen.cursor.filter(|_| *draws_cursor);
        let rows = screen
            .rows
            .iter()
            .enumerate()
            .map(|(y, row)| {
                let mut spans: Vec<Span> = Vec::new();
                for (x, cell) in row.cells.iter().enumerate() {
                    let mut span = Span {
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
                    if let Some(cursor) = cursor
                        && usize::from(cursor.x) == x
                        && usize::from(cursor.y) == y
                    {
                        let under = span.background.unwrap_or(color(screen.background));
                        span.background = Some(color(cursor.color));
                        span.color = under;
                    }
                    match spans.last_mut() {
                        Some(last) if last.same_style(&span) => last.text.push_str(&span.text),
                        _ => spans.push(span),
                    }
                }
                spans
            })
            .collect();
        let background = color(screen.background);
        drop(state);
        self.rows.set(rows);
        self.background.set(background);
    }
}

fn color(color: Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}
