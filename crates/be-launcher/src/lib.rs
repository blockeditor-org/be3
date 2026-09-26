#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "android", test))]
mod builds;
mod detail;
mod github;
#[cfg(not(target_os = "android"))]
mod keys;
mod markdown;
mod model;
#[cfg(not(target_os = "android"))]
mod pane;
#[cfg(target_os = "android")]
mod phone;
#[cfg(not(target_os = "android"))]
mod targets;
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
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use beui::reactive::{Frame, build, view, with_reactive_scope};
use beui::styled::use_theme;
use beui::{App, Color32, Context, Document, Rect, Setup};

use model::Model;
#[cfg(not(target_os = "android"))]
use pane::{Pane, Session};
#[cfg(target_os = "android")]
use phone::Phone;
use tasks::{Event, Tasks};
use view::Launcher;

#[cfg(not(target_os = "android"))]
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
    #[cfg(not(target_os = "android"))]
    fn new(root: PathBuf) -> Result<Self, String> {
        let (sender, events) = channel();
        let tasks = Tasks::new(root, sender);
        let session = Session::new(tasks.clone())?;
        Ok(Self::with(tasks, events, move |tasks| {
            Model::new(tasks.clone(), Pane::new(session, tasks))
        }))
    }

    #[cfg(target_os = "android")]
    fn new(files: PathBuf, shell: String) -> Self {
        let (sender, events) = channel();
        let tasks = Tasks::new(files, sender);
        Self::with(tasks, events, move |tasks| {
            Model::new(tasks.clone(), Phone::new(tasks, shell))
        })
    }

    fn with(tasks: Tasks, events: Receiver<Event>, model: impl FnOnce(Tasks) -> Model) -> Self {
        let mut exported = None;
        let document = build(|| {
            let theme = use_theme();
            let model = model(tasks.clone());
            exported = Some(model.clone());
            view! {
                <Frame color={theme.background.clone()} radius=0>
                    <Launcher model />
                </Frame>
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
        let model = self.model.clone();
        with_reactive_scope(&mut self.document, || {
            for event in events {
                model.receive(event);
            }
            #[cfg(not(target_os = "android"))]
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
        #[cfg(not(target_os = "android"))]
        self.tasks.stop();
    }
}
