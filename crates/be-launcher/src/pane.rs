use beui::NodeId;
use beui::reactive::*;
use terminal_view::{Emulator, Fit, TerminalScreen, TerminalView};

use crate::tasks::Tasks;

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;
const MAX_SCROLLBACK: usize = 10_000;

pub(crate) struct Session {
    emulator: Emulator,
}

#[derive(Clone, Copy)]
pub(crate) struct Pane {
    screen: TerminalScreen,
    tasks: Stored<Tasks>,
}

impl Session {
    pub(crate) fn new(tasks: Tasks) -> Result<Self, String> {
        let mut emulator = Emulator::new(DEFAULT_COLS, DEFAULT_ROWS, MAX_SCROLLBACK)?;
        emulator.on_reply(move |reply| tasks.input(reply))?;
        Ok(Self { emulator })
    }
}

impl Pane {
    pub(crate) fn new(session: Session, tasks: Tasks) -> Self {
        Self {
            screen: TerminalScreen::new(session.emulator),
            tasks: store(tasks),
        }
    }

    pub(crate) fn write(&self, bytes: &[u8]) {
        self.screen.write(bytes);
    }

    pub(crate) fn write_line(&self, text: &str) {
        self.write(format!("{text}\r\n").as_bytes());
    }

    pub(crate) fn refresh(&self) {
        self.screen.refresh();
    }

    fn fit(&self, fit: Fit) {
        if self
            .screen
            .resize(fit.cols, fit.rows, fit.cell_width, fit.cell_height)
        {
            self.tasks.value().resize(fit.cols, fit.rows);
            self.refresh();
        }
    }
}

#[component]
pub(crate) fn TerminalPane(pane: Pane) -> NodeId {
    view! {
        <TerminalView
            screen={pane.screen}
            on_input={move |bytes: Vec<u8>| pane.tasks.value().input(&bytes)}
            on_fit={move |fit: Fit| pane.fit(fit)}
        />
    }
}
