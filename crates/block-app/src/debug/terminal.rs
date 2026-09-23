use std::cell::RefCell;

use beui::{Color32, Key, Modifiers};
use ghostty_vt::{Renderer, Rgb, Terminal};

use crate::ui::{TerminalInput, TerminalRow, TerminalSpan, TerminalView};

const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;
const MAX_SCROLLBACK: usize = 10_000;
const PROMPT: &str = "\x1b[32mblock\x1b[0m:\x1b[34m~\x1b[0m$ ";

#[derive(Default)]
struct TerminalDebugWindow {
    session: Option<Session>,
    error: Option<String>,
}

thread_local! {
    static STATE: RefCell<TerminalDebugWindow> = RefCell::new(TerminalDebugWindow::default());
}

pub(super) fn poll() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.session.is_none() && state.error.is_none() {
            match Session::new() {
                Ok(session) => state.session = Some(session),
                Err(err) => state.error = Some(err),
            }
        }
    });
}

pub(super) fn close() {
    STATE.with(|state| *state.borrow_mut() = TerminalDebugWindow::default());
}

pub(super) fn input(input: TerminalInput) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if let TerminalInput::Retry = input {
            state.error = None;
            return;
        }
        let Some(session) = &mut state.session else {
            return;
        };
        match input {
            TerminalInput::Text(text) => session.insert(&text.replace(['\r', '\n'], " ")),
            TerminalInput::Key(key, modifiers) => session.key(key, modifiers),
            TerminalInput::Scroll(rows) => session.terminal.scroll_by(rows),
            TerminalInput::Resize {
                cols,
                rows,
                cell_width,
                cell_height,
            } => session.resize(cols, rows, cell_width, cell_height),
            TerminalInput::Retry => {}
        }
    });
}

pub(super) fn view() -> TerminalView {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let error = state.error.clone();
        let Some(session) = &mut state.session else {
            return TerminalView {
                rows: Vec::new(),
                background: Color32::BLACK,
                error,
            };
        };
        session.view()
    })
}

struct Session {
    terminal: Terminal,
    renderer: Renderer,
    line: String,
    history: Vec<String>,
    recalled: Option<usize>,
    cols: u16,
    rows: u16,
}

impl Session {
    fn new() -> Result<Self, String> {
        let terminal = Terminal::new(DEFAULT_COLS, DEFAULT_ROWS, MAX_SCROLLBACK)
            .map_err(|err| format!("Failed to create the terminal emulator: {err}"))?;
        let renderer =
            Renderer::new().map_err(|err| format!("Failed to create the renderer: {err}"))?;
        let mut session = Self {
            terminal,
            renderer,
            line: String::new(),
            history: Vec::new(),
            recalled: None,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        };
        session.banner();
        Ok(session)
    }

    fn resize(&mut self, cols: u16, rows: u16, cell_width: u32, cell_height: u32) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        let _ = self.terminal.resize(cols, rows, cell_width, cell_height);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        match key {
            Key::Enter => self.submit(),
            Key::Backspace => self.backspace(),
            Key::ArrowUp => self.recall(-1),
            Key::ArrowDown => self.recall(1),
            Key::C if modifiers.ctrl => {
                self.write_line("^C");
                self.line.clear();
                self.recalled = None;
                self.prompt();
            }
            Key::U if modifiers.ctrl => {
                self.line.clear();
                self.redraw_line();
            }
            Key::L if modifiers.ctrl => {
                self.write("\x1b[2J\x1b[H");
                self.redraw_line();
            }
            _ => {}
        }
    }

    fn banner(&mut self) {
        self.write("\x1b[1mlibghostty-vt demo\x1b[0m\r\n");
        self.write(
            "This window is a terminal emulator with a toy command line attached to it.\r\n\
             Nothing here runs a program: the commands below are all there is.\r\n\
             Type \x1b[1mhelp\x1b[0m for the list.\r\n\r\n",
        );
        self.prompt();
    }

    fn write(&mut self, text: &str) {
        self.terminal.write(text.as_bytes());
    }

    fn write_line(&mut self, text: &str) {
        self.write(text);
        self.write("\r\n");
    }

    fn prompt(&mut self) {
        self.write(PROMPT);
    }

    fn insert(&mut self, text: &str) {
        let text = text
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>();
        if text.is_empty() {
            return;
        }
        self.line.push_str(&text);
        self.write(&text);
    }

    fn backspace(&mut self) {
        if self.line.pop().is_some() {
            self.write("\x08 \x08");
        }
    }

    fn redraw_line(&mut self) {
        let line = std::mem::take(&mut self.line);
        self.write("\r\x1b[K");
        self.prompt();
        self.write(&line);
        self.line = line;
    }

    fn recall(&mut self, direction: isize) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        self.recalled = match (self.recalled, direction) {
            (None, -1) => Some(last),
            (Some(index), -1) => Some(index.saturating_sub(1)),
            (Some(index), _) if index < last => Some(index + 1),
            (Some(_), _) => None,
            (None, _) => None,
        };
        self.line = self
            .recalled
            .map(|index| self.history[index].clone())
            .unwrap_or_default();
        self.redraw_line();
    }

    fn submit(&mut self) {
        self.write("\r\n");
        let line = std::mem::take(&mut self.line);
        self.recalled = None;
        let command = line.trim().to_owned();
        if !command.is_empty() && self.history.last() != Some(&command) {
            self.history.push(command.clone());
        }
        self.run(&command);
        self.prompt();
    }

    fn run(&mut self, line: &str) {
        let (command, argument) = match line.split_once(char::is_whitespace) {
            Some((command, argument)) => (command, argument.trim()),
            None => (line, ""),
        };
        match command {
            "" => {}
            "help" => self.help(),
            "echo" => self.write_line(argument),
            "clear" => self.write("\x1b[2J\x1b[H"),
            "colors" => self.colors(),
            "style" => self.styles(),
            "history" => self.list_history(),
            "about" => self.about(),
            other => self.write_line(&format!(
                "\x1b[31m{other}: not one of the commands this demo knows\x1b[0m"
            )),
        }
    }

    fn help(&mut self) {
        for (command, description) in [
            ("help", "show this list"),
            ("echo TEXT", "write TEXT back out"),
            ("colors", "print the palette the emulator resolves"),
            ("style", "print bold, italic, underlined and inverted text"),
            ("history", "list the commands entered so far"),
            ("clear", "clear the screen"),
            ("about", "what this window is"),
        ] {
            self.write_line(&format!("  \x1b[1m{command:<10}\x1b[0m  {description}"));
        }
        self.write_line("");
        self.write_line("Ctrl+C abandons a line, Ctrl+U clears it, Ctrl+L clears the screen.");
        self.write_line("The arrow keys walk back through the lines already entered.");
    }

    fn colors(&mut self) {
        self.write_line("The sixteen named colors, as foreground and as background:");
        let mut line = String::from("  ");
        for index in 0..16 {
            line.push_str(&format!("\x1b[38;5;{index}m{index:>3}\x1b[0m "));
        }
        self.write_line(&line);
        let mut line = String::from("  ");
        for index in 0..16 {
            line.push_str(&format!("\x1b[48;5;{index}m{index:>3}\x1b[0m "));
        }
        self.write_line(&line);
        self.write_line("");
        self.write_line("A slice of the 256-color cube:");
        for row in 0..6 {
            let mut line = String::from("  ");
            for column in 0..36 {
                let index = 16 + row * 36 + column;
                line.push_str(&format!("\x1b[48;5;{index}m \x1b[0m"));
            }
            self.write_line(&line);
        }
    }

    fn styles(&mut self) {
        self.write_line("  \x1b[1mbold\x1b[0m");
        self.write_line("  \x1b[3mitalic\x1b[0m");
        self.write_line("  \x1b[4munderlined\x1b[0m");
        self.write_line("  \x1b[9mstruck through\x1b[0m");
        self.write_line("  \x1b[7minverted\x1b[0m");
        self.write_line("  \x1b[38;2;255;128;0mtwenty-four bit color\x1b[0m");
    }

    fn list_history(&mut self) {
        if self.history.is_empty() {
            self.write_line("  nothing yet");
            return;
        }
        for (index, entry) in self.history.clone().iter().enumerate() {
            self.write_line(&format!("  {:>3}  {entry}", index + 1));
        }
    }

    fn about(&mut self) {
        self.write_line(
            "The grid above is Ghostty's terminal emulator, libghostty-vt, linked into the app \
             as a static archive.",
        );
        self.write_line(
            "It parses the bytes this command line writes and answers with the cells to draw, \
             which is all a terminal emulator does.",
        );
        self.write_line(
            "Running programs needs a pseudo-terminal, which the browser and Android do not \
             have, so this window has a toy command line in place of a shell.",
        );
    }

    fn view(&mut self) -> TerminalView {
        let Ok(screen) = self.renderer.update(&mut self.terminal) else {
            return TerminalView {
                rows: Vec::new(),
                background: Color32::BLACK,
                error: None,
            };
        };
        let cursor = screen.cursor;
        let rows = screen
            .rows
            .iter()
            .enumerate()
            .map(|(y, row)| {
                let mut spans: Vec<TerminalSpan> = Vec::new();
                for (x, cell) in row.cells.iter().enumerate() {
                    let at_cursor = cursor
                        .is_some_and(|cursor| usize::from(cursor.x) == x && usize::from(cursor.y) == y);
                    let background = match at_cursor {
                        true => cursor.map(|cursor| half(color(cursor.color))),
                        false => cell.background.map(color),
                    };
                    let text = match cell.text.is_empty() {
                        true => " ".to_owned(),
                        false => cell.text.clone(),
                    };
                    let span = TerminalSpan {
                        text,
                        color: color(cell.foreground),
                        background,
                        bold: cell.bold,
                        italic: cell.italic,
                        underline: cell.underline || cell.strikethrough,
                    };
                    match spans.last_mut() {
                        Some(last) if last.same_style(&span) => last.text.push_str(&span.text),
                        _ => spans.push(span),
                    }
                }
                TerminalRow { spans }
            })
            .collect();
        TerminalView {
            rows,
            background: color(screen.background),
            error: None,
        }
    }
}

fn color(color: Rgb) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn half(color: Color32) -> Color32 {
    let [red, green, blue, _] = color.to_array();
    Color32::from_rgba_unmultiplied(red, green, blue, 128)
}
