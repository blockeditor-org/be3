use std::time::Instant;

use be_drive::Driven;
use beui_core::app::automation::Reply;
use paint_snapshot::{Painting, Snapshot};

const SIZE: &str = "1100x720";
const ACCEPT: &str = "./scripts/verify --plugin-tests, or ./scripts/buck test //crates/app-tests:test -- --env UPDATE_SNAPSHOTS=1";

pub struct AppTest {
    app: Option<Driven>,
    steps: Vec<String>,
    recorded: Option<Snapshot>,
    crop: Option<String>,
    started: Instant,
}

impl AppTest {
    pub fn start() -> Self {
        Self::start_with(&[], "Pane \"Files\"")
    }

    pub fn start_desktop() -> Self {
        Self::start_with(&["--desktop"], "Pane \"Linux desktop\"")
    }

    pub fn start_with(arguments: &[&str], ready: &str) -> Self {
        let started = Instant::now();
        let program = std::env::var("BLOCK_APP")
            .expect("BLOCK_APP names the app to test; run these through ./scripts/buck test //crates/app-tests:test");
        let mut all = vec!["--dev-workspace"];
        all.extend_from_slice(arguments);
        let app = Driven::start(&program, &all, SIZE, std::env::var("APP_LIBRARIES").ok())
            .unwrap_or_else(|error| panic!("block-app did not start: {error}"));
        let mut test = Self {
            app: Some(app),
            steps: Vec::new(),
            recorded: None,
            crop: None,
            started,
        };
        test.wait(ready);
        test.run(&["clock", "stop"]);
        test
    }

    pub fn run(&mut self, words: &[&str]) -> String {
        self.steps.push(line(words));
        match self.driven().request(words) {
            Ok(Reply::Text(text)) => text,
            Ok(_) => self.fail(&format!(
                "{} answered with something other than text",
                words[0]
            )),
            Err(error) => self.fail(&error),
        }
    }

    pub fn wait(&mut self, target: &str) {
        self.run(&["wait", target]);
    }

    pub fn gone(&mut self, target: &str) {
        self.run(&["gone", target]);
    }

    pub fn click(&mut self, target: &str) -> String {
        self.wait(target);
        self.run(&["click", target])
    }

    pub fn double_click(&mut self, target: &str) -> String {
        self.wait(target);
        self.run(&["click", target, "double"])
    }

    pub fn type_text(&mut self, text: &str) -> String {
        self.run(&["type", text])
    }

    pub fn key(&mut self, chord: &str) -> String {
        self.run(&["key", chord])
    }

    pub fn pause(&mut self, milliseconds: u32) {
        self.run(&["pause", &milliseconds.to_string()]);
    }

    pub fn tree(&mut self) -> String {
        self.run(&["tree"])
    }

    pub fn place(&mut self, target: &str) -> (f32, f32, f32, f32) {
        let found = self.run(&["find", target]);
        let numbers: Vec<f32> = found
            .split(|character: char| !character.is_ascii_digit() && character != '-' && character != '.')
            .filter_map(|number| number.parse().ok())
            .collect();
        match numbers[..] {
            [x, y, width, height, ..] => (x, y, width, height),
            _ => self.fail(&format!("find answered {found:?}, not a place")),
        }
    }

    pub fn count(&mut self, text: &str) -> usize {
        self.tree().lines().filter(|line| line.contains(text)).count()
    }

    pub fn shown(&mut self, text: &str) -> bool {
        self.tree().contains(text)
    }

    pub fn crop(&mut self, target: &str) {
        self.crop = Some(target.to_owned());
    }

    pub fn record(&mut self) {
        let painted = self.paint();
        match &mut self.recorded {
            Some(recorded) => recorded.append(painted),
            None => self.recorded = Some(painted),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        self.record();
        let recorded = self.recorded.take().expect("a painting was just recorded");
        paint_snapshot::assert_accepted(name, &recorded, ACCEPT);
    }

    pub fn close(mut self) {
        self.steps.push("close".to_owned());
        let app = self.app.take().expect("the app is still open");
        if let Err(error) = app.close() {
            self.fail(&format!("the app did not exit cleanly: {error}"));
        }
        eprintln!(
            "the app ran for {:.1}s, {} steps",
            self.started.elapsed().as_secs_f32(),
            self.steps.len()
        );
    }

    fn paint(&mut self) -> Snapshot {
        self.gone("#editor.loading");
        let mut words = vec!["paint"];
        words.extend(self.crop.as_deref());
        self.steps
            .push(line(&words).replacen("paint", "paint FILE", 1));
        let painted = match self.driven().request(&words) {
            Ok(Reply::Painting(bytes)) => Painting::decode(&bytes).and_then(Snapshot::of_painting),
            Ok(_) => Err("paint answered with something other than a painting".to_owned()),
            Err(error) => Err(error),
        };
        painted.unwrap_or_else(|error| self.fail(&error))
    }

    fn driven(&self) -> &Driven {
        self.app.as_ref().expect("the app is still open")
    }

    fn fail(&self, error: &str) -> ! {
        let log = self
            .app
            .as_ref()
            .map(|app| {
                let tree = match app.request(&["tree"]) {
                    Ok(Reply::Text(tree)) => tree,
                    _ => "(the app did not answer)".to_owned(),
                };
                format!(
                    "\n\nWhat the app shows:\n{tree}\n\nThe end of the app's log:\n{}",
                    app.log_tail()
                )
            })
            .unwrap_or_default();
        panic!(
            "{error}\n\nThe steps so far, which `drive -` replays against ./scripts/buck run //crates/block-app:dev -- --fresh:\n{}{log}",
            self.steps.join("\n")
        );
    }
}

fn line(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| {
            let plain = !word.is_empty()
                && word.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "#._-,/=".contains(character)
                });
            match plain {
                true => (*word).to_owned(),
                false => format!("'{}'", word.replace('\'', "'\\''")),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
