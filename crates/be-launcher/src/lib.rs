mod pane;
mod tasks;
mod view;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::sync::mpsc::{Receiver, channel};

use beui::reactive::{Frame, build, view, with_reactive_scope};
use beui::styled::use_theme;
use beui::{App, Color32, Context, Document, Rect, Setup};

use pane::{Pane, Session};
use tasks::{Event, Tasks};
use view::{Launcher, State};

const PADDING: f32 = 16.0;
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

pub fn run() -> Result<(), Box<dyn Error>> {
    let root = tasks::repository_root()?;
    beui::run("be3 launcher", LauncherApp::new(root)?)
}

struct LauncherApp {
    document: Document,
    state: State,
    tasks: Tasks,
    events: Receiver<Event>,
}

impl LauncherApp {
    fn new(root: std::path::PathBuf) -> Result<Self, String> {
        let (sender, events) = channel();
        let tasks = Tasks::new(root, sender);
        let session = Session::new()?;
        let mut exported = None;
        let document = build(|| {
            let theme = use_theme();
            let state = State::new(Pane::new(session));
            exported = Some(state.clone());
            let tasks = tasks.clone();
            view! {
                <Frame
                    color={theme.background.clone()}
                    radius=0
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <Launcher state tasks />
                </Frame>
            }
        });
        let state = exported.expect("build ran the view");
        tasks.list_pull_requests();
        tasks.read_head();
        Ok(Self {
            document,
            state,
            tasks,
            events,
        })
    }

    fn receive(&mut self) {
        let events: Vec<Event> = self.events.try_iter().collect();
        if events.is_empty() {
            return;
        }
        let state = self.state.clone();
        with_reactive_scope(&mut self.document, || {
            for event in events {
                match event {
                    Event::Started(description) => {
                        state.set_running.set(true);
                        state.set_status.set(format!("Running {description}"));
                        state
                            .pane
                            .write_line(&format!("{BOLD}$ {description}{RESET}"));
                    }
                    Event::Output(bytes) => state.pane.write(&bytes),
                    Event::Finished { summary, success } => {
                        let color = if success { GREEN } else { RED };
                        state.pane.write_line(&format!("{color}{summary}{RESET}\n"));
                        state.set_running.set(false);
                        state.set_status.set(summary);
                    }
                    Event::Head(head) => state.set_head.set(head),
                    Event::PullRequests(Ok(listing)) => {
                        state.set_status.set(format!(
                            "{} {}",
                            listing.pull_requests.len(),
                            listing.source
                        ));
                        state.pull_requests.reconcile_owned(
                            listing
                                .pull_requests
                                .into_iter()
                                .map(|pull_request| (pull_request.branch.clone(), pull_request)),
                        );
                    }
                    Event::PullRequests(Err(error)) => state.set_status.set(error),
                }
            }
            state.pane.refresh();
        });
    }
}

impl App for LauncherApp {
    fn setup(&mut self, setup: &Setup) {
        self.tasks.set_waker(setup.waker.clone());
    }

    fn update(&mut self, context: &Context, rect: Rect) {
        self.receive();
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }

    fn exiting(&mut self) {
        self.tasks.stop();
    }
}
