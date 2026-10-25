use std::cell::RefCell;
use std::rc::Rc;

use ghostty_vt::{
    ActiveScreen, Anchor, AnchorPlace, Cell, Colors, CursorStyle, KeyEncoder, Mode, Mods,
    MouseEncoder, Renderer, Rgb, Row, Terminal,
};
use terminal_types::{
    HISTORY_LINES, TerminalColor, TerminalColors, TerminalCursor, TerminalCursorStyle, TerminalKey,
    TerminalLine, TerminalSize, TerminalStyle, TerminalWheel,
};

use super::keys::key_input;

const SCROLLBACK: usize = 4_000;
const FEED_CHUNK: usize = 256;

pub struct Shown {
    pub rows: Vec<TerminalLine>,
    pub alternate: bool,
    pub cursor: TerminalCursor,
    pub size: TerminalSize,
    pub colors: TerminalColors,
    pub title: String,
}

pub struct Emulator {
    terminal: Terminal,
    renderer: Renderer,
    anchor: Anchor,
    keys: KeyEncoder,
    mouse: MouseEncoder,
    replies: Rc<RefCell<Vec<u8>>>,
    scrolled: Vec<TerminalLine>,
    returned: Vec<TerminalLine>,
}

impl Emulator {
    pub fn new(size: TerminalSize) -> Result<Self, String> {
        let failed =
            |error: ghostty_vt::Error| format!("could not make a terminal emulator: {error}");
        let mut terminal = Terminal::new(size.cols, size.rows, SCROLLBACK).map_err(failed)?;
        let replies = Rc::new(RefCell::new(Vec::new()));
        let held = Rc::clone(&replies);
        terminal
            .on_reply(move |reply| held.borrow_mut().extend_from_slice(reply))
            .map_err(failed)?;
        Ok(Self {
            anchor: Anchor::new(&terminal).map_err(failed)?,
            renderer: Renderer::new().map_err(failed)?,
            keys: KeyEncoder::new().map_err(failed)?,
            mouse: MouseEncoder::new().map_err(failed)?,
            terminal,
            replies,
            scrolled: Vec::new(),
            returned: Vec::new(),
        })
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(FEED_CHUNK) {
            self.terminal.write(chunk);
            self.collect();
        }
    }

    pub fn resize(&mut self, size: TerminalSize) {
        let _ = self.terminal.resize(size.cols, size.rows, 1, 1);
        self.collect();
    }

    pub fn reset(&mut self, size: TerminalSize) {
        self.terminal.reset();
        let _ = self.terminal.resize(size.cols, size.rows, 1, 1);
        self.scrolled.clear();
        self.returned.clear();
        let _ = self.anchor.reset(&self.terminal);
    }

    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.replies.borrow_mut())
    }

    pub fn take_scrolled(&mut self) -> (Vec<TerminalLine>, Vec<TerminalLine>) {
        (
            std::mem::take(&mut self.returned),
            std::mem::take(&mut self.scrolled),
        )
    }

    fn collect(&mut self) {
        if self.terminal.active_screen() == ActiveScreen::Alternate {
            return;
        }
        let colors = self.terminal.colors();
        let scrollback = self.terminal.scrollback_rows();
        match self.anchor.place() {
            AnchorPlace::Active(0) => return,
            AnchorPlace::Active(back) => {
                let back = back as usize;
                let unwritten = back.min(self.scrolled.len());
                self.scrolled.truncate(self.scrolled.len() - unwritten);
                self.returned.extend((0..back - unwritten).filter_map(|y| {
                    let row = self.terminal.active_row(y as u16, &colors)?;
                    Some(line(&row, &colors))
                }));
            }
            AnchorPlace::History(first) => self.read_history(first..scrollback, &colors),
            AnchorPlace::Lost => {
                eprintln!(
                    "be: the terminal scrolled faster than it could be read; some lines are missing"
                );
                self.read_history(0..scrollback, &colors);
            }
        }
        let _ = self.anchor.reset(&self.terminal);
    }

    fn read_history(&mut self, rows: std::ops::Range<usize>, colors: &Colors) {
        for y in rows {
            if let Some(row) = self.terminal.history_row(y, colors) {
                self.scrolled.push(line(&row, colors));
            }
        }
        let excess = self.scrolled.len().saturating_sub(HISTORY_LINES);
        self.scrolled.drain(..excess);
    }

    pub fn shown(&mut self) -> Shown {
        let colors = self.terminal.colors();
        let (cols, rows) = self.terminal.size();
        let lines: Vec<(Row, TerminalLine)> = (0..rows)
            .filter_map(|y| {
                let row = self.terminal.active_row(y, &colors)?;
                let line = line(&row, &colors);
                Some((row, line))
            })
            .collect();
        let (column, row) = self.terminal.cursor_position();
        let rendered = self.renderer.cursor(&self.terminal).ok().flatten();
        let byte = lines
            .get(usize::from(row))
            .map_or(usize::from(column), |(cells, _)| {
                cells
                    .cells
                    .iter()
                    .take(usize::from(column))
                    .filter(|cell| !cell.spacer)
                    .map(|cell| cell.text.len().max(1))
                    .sum()
            });
        Shown {
            alternate: self.terminal.active_screen() == ActiveScreen::Alternate,
            cursor: TerminalCursor {
                row,
                column,
                byte: byte as u32,
                visible: self.terminal.cursor_visible(),
                style: match rendered.map(|cursor| cursor.style) {
                    Some(CursorStyle::Bar) => TerminalCursorStyle::Bar,
                    Some(CursorStyle::Underline) => TerminalCursorStyle::Underline,
                    Some(CursorStyle::BlockHollow) => TerminalCursorStyle::BlockHollow,
                    Some(CursorStyle::Block) | None => TerminalCursorStyle::Block,
                },
                blink: rendered.is_some_and(|cursor| cursor.blinking),
            },
            rows: lines.into_iter().map(|(_, line)| line).collect(),
            size: TerminalSize { cols, rows },
            colors: TerminalColors {
                foreground: color(colors.foreground),
                background: color(colors.background),
            },
            title: self.terminal.title(),
        }
    }

    pub fn key(&mut self, key: &TerminalKey) -> Vec<u8> {
        let input = key_input(key);
        self.keys.encode(&self.terminal, &input)
    }

    pub fn paste(&mut self, text: &str) -> Vec<u8> {
        let _ = self.terminal.paste(text);
        self.take_replies()
    }

    pub fn wheel(&mut self, wheel: &TerminalWheel) -> Vec<u8> {
        let up = wheel.lines < 0;
        let mut bytes = Vec::new();
        for _ in 0..wheel.lines.unsigned_abs() {
            if self.terminal.mouse_tracking() {
                bytes.extend(self.mouse.wheel(
                    &self.terminal,
                    up,
                    wheel.col,
                    wheel.row,
                    Mods::default(),
                ));
            } else if self.terminal.active_screen() == ActiveScreen::Alternate
                && self.terminal.mode(Mode::ALTERNATE_SCROLL)
            {
                let key = TerminalKey {
                    key: match up {
                        true => terminal_types::Key::ArrowUp,
                        false => terminal_types::Key::ArrowDown,
                    },
                    modifiers: terminal_types::Modifiers::default(),
                    text: String::new(),
                    pressed: true,
                };
                bytes.extend(self.key(&key));
            }
        }
        bytes
    }
}

fn color(rgb: Rgb) -> TerminalColor {
    TerminalColor {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}

fn style(cell: &Cell, colors: &Colors) -> TerminalStyle {
    TerminalStyle {
        foreground: (cell.foreground != colors.foreground).then(|| color(cell.foreground)),
        background: cell.background.map(color),
        bold: cell.bold,
        italic: cell.italic,
        faint: cell.faint,
        underline: cell.underline,
        strikethrough: cell.strikethrough,
    }
}

fn line(row: &Row, colors: &Colors) -> TerminalLine {
    let cells: Vec<&Cell> = row.cells.iter().filter(|cell| !cell.spacer).collect();
    let kept = match row.wrapped {
        true => cells.len(),
        false => cells
            .iter()
            .rposition(|cell| {
                (!cell.text.is_empty() && cell.text != " ") || cell.background.is_some()
            })
            .map_or(0, |last| last + 1),
    };
    let mut line = TerminalLine::default();
    for cell in &cells[..kept] {
        let text = match cell.text.is_empty() {
            true => " ",
            false => cell.text.as_str(),
        };
        line.push(text, style(cell, colors));
    }
    line.wrapped = row.wrapped;
    line
}
