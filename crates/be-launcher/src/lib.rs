mod actions;
#[cfg(target_os = "android")]
mod android;
mod builds;
mod detail;
mod github;
mod helper;
mod markdown;
mod model;
mod net;
mod pane;
mod runs;
mod tasks;
mod time;
mod view;
mod viewer;
#[cfg(not(target_os = "android"))]
mod workspace;

#[cfg(test)]
mod tests;

#[cfg(not(target_os = "android"))]
use std::error::Error;

use beui::styled::*;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use beui::native::Document;
use beui::reactive::*;
use beui::{App, Context, Rect, Setup};

use model::Model;
use pane::{Pane, Session};
use runs::Runs;
use tasks::{Event, Tasks};
use view::Launcher;

#[cfg(not(target_os = "android"))]
pub fn run() -> Result<(), Box<dyn Error>> {
    beui::run(LauncherApp::new(runs::cache_directory())?)
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
        Ok(Self::with(tasks, events, move |tasks| {
            let runs = Runs::new(tasks.clone(), Pane::new(session));
            Model::new(tasks, runs)
        }))
    }

    fn with(tasks: Tasks, events: Receiver<Event>, model: impl FnOnce(Tasks) -> Model) -> Self {
        let mut exported = None;
        let document = build(|| {
            let model = model(tasks.clone());
            exported = Some(model);
            view! {
                <Root title="be3 launcher">
                    <Launcher model />
                </Root>
            }
        });
        let model = exported.expect("build ran the view");
        model.start();
        Self {
            document,
            model,
            tasks,
            events,
        }
    }

    fn receive(&mut self) {
        let events: Vec<Event> = self.events.try_iter().collect();
        if events.is_empty() {
            return;
        }
        let model = self.model;
        for event in events {
            model.receive(event);
        }
        model.runs.pane.refresh();
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
}
