use beui::NodeId;
use beui::reactive::*;
use terminal_view::{Emulator, Fit, TerminalScreen, TerminalView};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;
const MAX_SCROLLBACK: usize = 10_000;

pub(crate) struct Session {
    emulator: Emulator,
}

#[derive(Clone, Copy)]
pub(crate) struct Pane {
    screen: TerminalScreen,
}

impl Session {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            emulator: Emulator::new(DEFAULT_COLS, DEFAULT_ROWS, MAX_SCROLLBACK)?,
        })
    }
}

impl Pane {
    pub(crate) fn new(session: Session) -> Self {
        Self {
            screen: TerminalScreen::new(session.emulator),
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
            self.refresh();
        }
    }
}

#[component]
pub(crate) fn TerminalPane(pane: Pane) -> NodeId {
    view! {
        <TerminalView
            screen={pane.screen}
            on_input={|_: Vec<u8>| {}}
            on_fit={move |fit: Fit| pane.fit(fit)}
        />
    }
}
