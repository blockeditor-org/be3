mod detail;
mod github;
mod markdown;
mod model;
mod pane;
mod runner;
mod tasks;
mod time;
mod view;

#[cfg(test)]
mod tests;

use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use beui::reactive::{Frame, build, view, with_reactive_scope};
use beui::styled::use_theme;
use beui::{App, Color32, Context, Document, Rect, Setup};

use model::Model;
use pane::{Pane, Session};
use tasks::{Event, Tasks};
use view::Launcher;

pub fn run() -> Result<(), Box<dyn Error>> {
    let root = tasks::repository_root()?;
    beui::run("be3 launcher", LauncherApp::new(root)?)
}

struct LauncherApp {
    document: Document,
    model: Model,
    tasks: Tasks,
    events: Receiver<Event>,
}

impl LauncherApp {
    fn new(root: PathBuf) -> Result<Self, String> {
        let (sender, events) = channel();
        let tasks = Tasks::new(root, sender);
        let session = Session::new()?;
        let mut exported = None;
        let document = build(|| {
            let theme = use_theme();
            let model = Model::new(tasks.clone(), Pane::new(session));
            exported = Some(model.clone());
            view! {
                <Frame color={theme.background.clone()} radius=0>
                    <Launcher model />
                </Frame>
            }
        });
        let model = exported.expect("build ran the view");
        model.start();
        Ok(Self {
            document,
            model,
            tasks,
            events,
        })
    }

    fn receive(&mut self) {
        let events: Vec<Event> = self.events.try_iter().collect();
        if events.is_empty() {
            return;
        }
        let model = self.model.clone();
        with_reactive_scope(&mut self.document, || {
            for event in events {
                model.receive(event);
            }
            model.pane.refresh();
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
