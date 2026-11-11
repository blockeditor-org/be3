use std::cell::Cell;
use std::time::{Duration, Instant};

use be_tool_drive::Driven;
use beui_layer_drive::Reply;
use beui_paint_snapshot::{Painting, Snapshot};

const SIZE: &str = "1100x720";
const MISSING_EDITOR: [&str; 2] = ["Unsupported block type", "No block types match."];
const SLOW_WAIT: Duration = Duration::from_secs(5);

/// be-host started headless with the dev workspace open, driven as `drive` drives it, for a test
/// of what happens between editors (guides/testing_a_gui.md, 6).
///
/// The clock is stopped as the host starts, so time passes only with [`HostTest::pause`], which
/// costs only the frames it runs; a closing menu or sheet stays until a pause as long as its
/// animation. The wall clock is the headless platform's fixed moment plus the frame clock.
/// A test that does not end with [`HostTest::close`] fails.
pub struct HostTest {
    host: Option<Driven>,
    steps: Vec<String>,
    recorded: Option<Snapshot>,
    overflows: Vec<String>,
    crop: Option<String>,
    started: Instant,
    reported: Cell<bool>,
    space: bool,
    editors: Option<String>,
}

impl HostTest {
    pub fn start() -> Self {
        Self::start_with(&[], "Pane \"Files\"")
    }

    pub fn start_desktop() -> Self {
        Self::start_with(&["--desktop"], "Pane \"Desktop\"")
    }

    /// Starts the host in the VR simulator (`BEUI_LAYERS=space`): the painting is the 3D view,
    /// targets and points are the main panel's. It draws through lavapipe, since the 3D view is
    /// a texture.
    pub fn start_in_space() -> Self {
        Self::launch(&[], "Pane \"Files\"", true, true)
    }

    /// Starts the host with a GPU renderer (lavapipe), which block previews need since they
    /// are textures. Every other start runs with `BEUI_RENDERER=none`, and apps then draw
    /// nothing on the GPU either.
    pub fn start_drawing_previews() -> Self {
        Self::launch(&[], "Pane \"Files\"", false, true)
    }

    pub fn start_with(arguments: &[&str], ready: &str) -> Self {
        Self::launch(arguments, ready, false, false)
    }

    fn launch(arguments: &[&str], ready: &str, space: bool, render: bool) -> Self {
        let started = Instant::now();
        let program = std::env::var("BE_HOST")
            .expect("BE_HOST names the host to test; run these through ./scripts/buck test //crates/be-host-tests:test");
        let mut all = vec!["--dev-workspace"];
        all.extend_from_slice(arguments);
        let host = Driven::start(
            &program,
            &all,
            SIZE,
            std::env::var("HOST_LIBRARIES").ok(),
            space,
            render,
        )
        .unwrap_or_else(|error| panic!("be-host did not start: {error}"));
        let mut test = Self {
            host: Some(host),
            steps: Vec::new(),
            recorded: None,
            overflows: Vec::new(),
            crop: None,
            started,
            reported: Cell::new(false),
            space,
            editors: std::env::var("HOST_TEST_APPS").ok(),
        };
        test.wait(ready);
        test.run(&["clock", "stop"]);
        test
    }

    pub fn run(&mut self, words: &[&str]) -> String {
        let answered = self.driven().request(words);
        self.answered(words, answered)
    }

    fn answered(&mut self, words: &[&str], answered: Result<Reply, String>) -> String {
        self.steps.push(line(words));
        match answered {
            Ok(Reply::Text(text)) => text,
            Ok(_) => self.fail(&format!(
                "{} answered with something other than text",
                words[0]
            )),
            Err(error) => self.fail(&error),
        }
    }

    pub fn wait(&mut self, target: &str) {
        if self.within(SLOW_WAIT, &["wait", target]).is_ok() {
            self.steps.push(line(&["wait", target]));
            return;
        }
        self.check_editors();
        self.run(&["wait", target]);
    }

    fn within(&mut self, timeout: Duration, words: &[&str]) -> Result<Reply, String> {
        let host = self.host.as_mut().expect("the host is still open");
        host.set_timeout(timeout);
        let reply = host.request(words);
        host.set_timeout(be_tool_drive::TIMEOUT);
        reply
    }

    fn check_editors(&mut self) {
        let tree = self.tree();
        let Some(shown) = MISSING_EDITOR.iter().find(|text| tree.contains(*text)) else {
            return;
        };
        let value = |marker: &str, prefix: &str, end: char| {
            tree.lines()
                .skip_while(|line| !line.contains(marker))
                .find_map(|line| line.split_once(prefix))
                .map(|(_, rest)| rest.split(end).next().unwrap_or_default().to_owned())
        };
        let needle = match *shown == MISSING_EDITOR[0] {
            true => value("Type: ", "Type: ", '"'),
            false => value("#picker.search", "value=\"", '"')
                .map(|search| format!("\"display_name\": \"{search}\"")),
        };
        let editors = match &self.editors {
            Some(editors) => format!(
                "its host stages only the apps {editors}: add {} to this test's apps in crates/be-host-tests/BUCK",
                needle
                    .as_deref()
                    .map_or_else(Vec::new, editors_naming)
                    .join(" or ")
            ),
            None => "its host stages every editor, and none edits this block's type".to_owned(),
        };
        self.fail(&format!(
            "the host shows {shown:?}: this test opens or creates a block of a type no staged editor edits; {editors}"
        ));
    }

    pub fn gone(&mut self, target: &str) {
        self.run(&["gone", target]);
    }

    /// Clicks at once when the target is on screen, and otherwise waits for it first. A gesture
    /// that needs a point takes it from [`HostTest::place`] and sends it with [`HostTest::run`].
    pub fn click(&mut self, target: &str) -> String {
        self.when_shown(target, &["click", target])
    }

    pub fn double_click(&mut self, target: &str) -> String {
        self.when_shown(target, &["click", target, "double"])
    }

    fn when_shown(&mut self, target: &str, words: &[&str]) -> String {
        match self.driven().request(words) {
            Err(error) if missing(&error) => {
                self.wait(target);
                self.run(words)
            }
            answered => self.answered(words, answered),
        }
    }

    pub fn type_text(&mut self, text: &str) -> String {
        self.run(&["type", text])
    }

    pub fn key(&mut self, chord: &str) -> String {
        self.run(&["key", chord])
    }

    /// Moves the stopped clock on, a frame at a time. A long pause runs many frames, so it may take
    /// as long as it names on top of the usual timeout.
    pub fn pause(&mut self, milliseconds: u32) {
        let words = ["pause", &milliseconds.to_string()];
        let timeout = be_tool_drive::TIMEOUT + Duration::from_millis(milliseconds.into());
        let answered = self.within(timeout, &words);
        self.answered(&words, answered);
    }

    pub fn tree(&mut self) -> String {
        self.run(&["tree"])
    }

    pub fn place(&mut self, target: &str) -> (f32, f32, f32, f32) {
        let found = self.run(&["find", target]);
        let numbers: Vec<f32> = found
            .split(|character: char| {
                !character.is_ascii_digit() && character != '-' && character != '.'
            })
            .filter_map(|number| number.parse().ok())
            .collect();
        match numbers[..] {
            [x, y, width, height, ..] => (x, y, width, height),
            _ => self.fail(&format!("find answered {found:?}, not a place")),
        }
    }

    pub fn count(&mut self, text: &str) -> usize {
        self.tree()
            .lines()
            .filter(|line| line.contains(text))
            .count()
    }

    pub fn shown(&mut self, text: &str) -> bool {
        self.tree().contains(text)
    }

    /// Paints only that target's area from here on. Block ids are random, so a painting that
    /// shows one, or the file tree's order by id, differs every run: crop to `#workspace.block`.
    pub fn crop(&mut self, target: &str) {
        self.crop = Some(target.to_owned());
    }

    /// Keeps the frame for the next [`HostTest::snapshot`], once every editor's loading state
    /// (`#editor.loading`) has gone. It is `drive paint`: each app's painting where the host
    /// composites it, and each preview replaced by the preview's own painting. A preview waits
    /// out its cooldown before it paints a second change, so pause for `PREVIEW_COOLDOWN` after
    /// changing a block whose preview the test looks at.
    ///
    /// The frame is painted twice, with a settle between, and the two must match: a painting
    /// that changes between two frames of a program with nothing left to do shows something
    /// that was still arriving (a hover, an animation's last step, an app's late frame), and
    /// would be a different painting on a slower machine. The test waits for what it needs, as
    /// guides/testing_a_gui.md, 6, says.
    pub fn record(&mut self) {
        let painted = self.paint();
        self.run(&["settle"]);
        let again = self.paint();
        if painted.body().ok() != again.body().ok() {
            self.fail(
                "the painting is not settled: it changed between two frames, so something was still arriving. Wait for it (`wait`, `gone`) before the snapshot, or park the pointer (`leave`) if it is a hover",
            );
        }
        let found = self.run(&["overflows"]);
        self.overflows.extend(found.lines().map(str::to_owned));
        match &mut self.recorded {
            Some(recorded) => recorded.append(painted),
            None => self.recorded = Some(painted),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        self.record();
        let recorded = self.recorded.take().expect("a painting was just recorded");
        beui_paint_snapshot::assert_accepted(name, &recorded, beui_paint_snapshot::HOW_TO_ACCEPT);
        let mut found = std::mem::take(&mut self.overflows);
        found.sort();
        found.dedup();
        if !found.is_empty() {
            self.fail(&format!(
                "the painting {name} shows {} element(s) out of bounds, cut off where nothing scrolls to them, drawn outside their own box, or with an outline cut while their box shows (guides/testing_a_gui.md, 4):\n  {}",
                found.len(),
                found.join("\n  ")
            ));
        }
    }

    pub fn close(mut self) {
        self.steps.push("close".to_owned());
        let host = self.host.take().expect("the host is still open");
        if let Err(error) = host.close() {
            self.fail(&format!("the host did not exit cleanly: {error}"));
        }
        eprintln!(
            "the host ran for {:.1}s, {} steps",
            self.started.elapsed().as_secs_f32(),
            self.steps.len()
        );
    }

    fn paint(&mut self) -> Snapshot {
        self.gone("#editor.loading");
        let mut words = vec!["paint"];
        words.extend(self.crop.as_deref());
        if self.space {
            words.extend(["in", "platform"]);
        }
        self.steps
            .push(line(&words).replacen("paint", "paint painted.paint", 1));
        let painted = match self.driven().request(&words) {
            Ok(Reply::Painting(bytes)) => Painting::decode(&bytes).and_then(Snapshot::of_painting),
            Ok(_) => Err("paint answered with something other than a painting".to_owned()),
            Err(error) => Err(error),
        };
        painted.unwrap_or_else(|error| self.fail(&error))
    }

    fn driven(&self) -> &Driven {
        self.host.as_ref().expect("the host is still open")
    }

    fn fail(&self, error: &str) -> ! {
        panic!("{error}\n\n{}", self.context());
    }

    fn context(&self) -> String {
        self.reported.set(true);
        let log = self
            .host
            .as_ref()
            .map(|host| {
                let tree = match host.request(&["tree"]) {
                    Ok(Reply::Text(tree)) => tree,
                    _ => "(the host did not answer)".to_owned(),
                };
                format!("\n\nWhat the host shows:\n{tree}\n\n{}", host.log_report())
            })
            .unwrap_or_default();
        let editors = self.editors.as_ref().map_or_else(String::new, |editors| {
            format!(
                "\n\nThis test's host stages only the apps {editors}. If it fails for want of another, or a painting changes because the add menu offers fewer types, add the app to this test's apps in crates/be-host-tests/BUCK, or remove the test from there to stage every app."
            )
        });
        format!(
            "The steps so far, which `drive -` replays against ./scripts/buck run //crates/be-host:dev -- --fresh:\n{}{log}{editors}",
            self.steps.join("\n")
        )
    }
}

impl Drop for HostTest {
    fn drop(&mut self) {
        match std::thread::panicking() {
            true if !self.reported.get() => eprintln!("\n{}", self.context()),
            true => {}
            false if self.host.is_some() => self.fail("the test ended with the host still open; end it with close(), which fails unless the host exits cleanly"),
            false => {}
        }
    }
}

fn editors_naming(text: &str) -> Vec<String> {
    let manifests = std::env::var("HOST_TEST_MANIFESTS").unwrap_or_default();
    manifests
        .split(':')
        .filter(|path| std::fs::read_to_string(path).is_ok_and(|manifest| manifest.contains(text)))
        .filter_map(|path| {
            path.split('/')
                .find_map(|part| part.strip_prefix("be-app-"))
                .map(str::to_owned)
        })
        .collect()
}

fn missing(error: &str) -> bool {
    error.starts_with("no line of the tree contains")
        || error.starts_with("no node on screen has the test id")
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
