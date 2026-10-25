use std::cell::RefCell;
use std::rc::Rc;

use be_block::ObjectId;
use beui::NodeId;
use beui::reactive::*;
use terminal_emulator::Emulator;
use terminal_types::{HISTORY_LINES, TerminalColors, TerminalLine, TerminalSize};
use terminal_view::{Fit, Lines, TerminalInput, TerminalView};

const FIRST_SIZE: TerminalSize = TerminalSize {
    cols: 100,
    rows: 30,
};

pub(crate) struct Session {
    emulator: Emulator,
}

struct Scrollback {
    emulator: Emulator,
    history: Vec<(ObjectId, Rc<TerminalLine>)>,
    rows: Vec<(ObjectId, Rc<TerminalLine>)>,
}

#[derive(Clone, Copy)]
pub(crate) struct Pane {
    state: Stored<RefCell<Scrollback>>,
    history: Signal<Lines>,
    rows: Signal<Lines>,
    colors: Signal<TerminalColors>,
}

impl Session {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            emulator: Emulator::new(FIRST_SIZE)?,
        })
    }
}

impl Scrollback {
    fn retract(&mut self, returned: &[TerminalLine]) {
        for row in returned.iter().rev() {
            let Some((_, tail)) = self.history.last_mut() else {
                return;
            };
            let keep = tail.text.len().saturating_sub(row.text.len());
            if keep == 0 {
                self.history.pop();
                continue;
            }
            let tail = Rc::make_mut(tail);
            tail.truncate(keep);
            tail.wrapped = true;
        }
    }

    fn append(&mut self, scrolled: Vec<TerminalLine>) {
        for line in scrolled {
            match self.history.last_mut() {
                Some((_, tail)) if tail.wrapped => Rc::make_mut(tail).join(&line),
                _ => self.history.push((ObjectId::new(), Rc::new(line))),
            }
        }
        let excess = self.history.len().saturating_sub(HISTORY_LINES);
        self.history.drain(..excess);
    }

    fn show(&mut self, shown: Vec<TerminalLine>) {
        self.rows.resize_with(shown.len(), || {
            (ObjectId::new(), Rc::new(TerminalLine::default()))
        });
        for ((_, held), line) in self.rows.iter_mut().zip(shown) {
            if **held != line {
                *held = Rc::new(line);
            }
        }
    }
}

impl Pane {
    pub(crate) fn new(session: Session) -> Self {
        Self {
            state: store(RefCell::new(Scrollback {
                emulator: session.emulator,
                history: Vec::new(),
                rows: Vec::new(),
            })),
            history: create_signal(Lines::default()),
            rows: create_signal(Lines::default()),
            colors: create_signal(TerminalColors::default()),
        }
    }

    pub(crate) fn write(&self, bytes: &[u8]) {
        self.state.value().borrow_mut().emulator.feed(bytes);
    }

    pub(crate) fn write_line(&self, text: &str) {
        self.write(format!("{text}\r\n").as_bytes());
    }

    pub(crate) fn refresh(&self) {
        let state = self.state.value();
        let mut state = state.borrow_mut();
        let (returned, scrolled) = state.emulator.take_scrolled();
        let scrolls = !returned.is_empty() || !scrolled.is_empty();
        state.retract(&returned);
        state.append(scrolled);
        let shown = state.emulator.shown();
        state.show(shown.rows);
        let history = scrolls.then(|| Lines::new(state.history.clone()));
        let rows = Lines::new(state.rows.clone());
        drop(state);
        if let Some(history) = history {
            self.history.set(history);
        }
        self.rows.set(rows);
        self.colors.set(shown.colors);
    }

    fn fit(&self, fit: Fit) {
        let size = TerminalSize {
            cols: fit.cols,
            rows: fit.rows,
        };
        self.state.value().borrow_mut().emulator.resize(size);
        self.refresh();
    }
}

#[component]
pub(crate) fn TerminalPane(pane: Pane) -> NodeId {
    view! {
        <TerminalView
            history={pane.history}
            rows={pane.rows}
            colors={pane.colors}
            input=false
            on_input={|_: TerminalInput| {}}
            on_fit={move |fit: Fit| pane.fit(fit)}
        />
    }
}
