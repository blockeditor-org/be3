use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use be_block_fault_relay::Relay;
use be_tool_drive::Driven;
use beui_layer_drive::Reply;
use beui_paint_format::Painting;
use beui_paint_snapshot::Snapshot;

mod player;

pub use player::Player;
pub use be_host_server::testing::{Account, Server};

/// A test's own server, on the test's clock: its devices' clocks start at the test's moment,
/// so the server's does too, since it stamps when it took each edit, which a device reads
/// against when the edit was made.
pub fn server() -> Server {
    Server::with(be_host_server::testing::Options {
        starts_at: Some(
            i64::try_from(beui_core::timer::TEST_WALL_CLOCK.as_millis()).unwrap_or(i64::MAX),
        ),
        ..be_host_server::testing::Options::default()
    })
}

/// How long `wait_still` moves the clock on between two looks, and how many it takes.
const STILL_STEP: u32 = 100;
const STILL_STEPS: usize = 50;
const SIZE: &str = "1100x720";
const MISSING_EDITOR: [&str; 2] = ["Unsupported block type", "No block types match."];
const SLOW_WAIT: Duration = Duration::from_secs(5);
/// How long a failed test's report waits for the host to say what it is out of step with.
const OUT_OF_STEP_WAIT: Duration = Duration::from_secs(5);
const FILES: &str = "Pane \"Files\"";
const SHOW_A_CODE: &str = "Button \"Show a code\"";
const FILE_TREE: &str = "#file-tree.filter";
/// What a host running a root shows once it runs: the drawing of what the root shows
/// (crates/be-host/src/root.rs).
const ROOT_SHOWN: &str = "#root";

/// be-host started headless with the dev workspace open, driven as `drive` drives it, for a test
/// of what happens between editors (guides/testing_a_gui.md, 6).
///
/// The clock is stopped as the host starts, so time passes only with [`HostTest::pause`], which
/// costs only the frames it runs; a closing menu or sheet stays until a pause as long as its
/// animation. The wall clock is the headless platform's fixed moment plus the frame clock.
/// A test that does not end with [`HostTest::close`] fails.
///
/// Each host embeds a server of its own, except a device ([`HostTest::device`]), which joins a
/// [`Server`] the test owns as a device of one of its accounts: a test of what happens between
/// devices (sync, going offline, restarting) runs two of them.
pub struct HostTest {
    host: Option<Driven>,
    steps: Vec<String>,
    recorded: Option<Snapshot>,
    overflows: Vec<String>,
    crop: Option<String>,
    started: Instant,
    reported: Cell<bool>,
    space: bool,
    /// Whether it runs the desktop (`--desktop`), which a restart as a root keeps.
    desktop: bool,
    editors: Option<String>,
    device: Option<Device>,
    /// The programs it started in the desktop's compositor (`open_test_window`), stopped as it
    /// ends.
    programs: Vec<std::process::Child>,
}

/// How [`HostTest::start`] starts the host. The default is the Files pane, painted without a
/// GPU: every start but `space` and `previews` runs with `BEUI_RENDERER=none`, and apps then
/// draw nothing on the GPU either.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// The desktop (`--desktop`) rather than the Files pane.
    pub desktop: bool,
    /// The VR simulator (`BEUI_LAYERS=space`): the painting is the 3D view, targets and points
    /// are the main panel's. It draws through lavapipe, since the 3D view is a texture.
    pub space: bool,
    /// A GPU renderer (lavapipe), which block previews need since they are textures.
    pub previews: bool,
}

/// What a host joined to a test's [`Server`] keeps: its account and its own network to the
/// server.
struct Device {
    account: Account,
    relay: Arc<Relay>,
}

impl HostTest {
    /// Starts the host with the dev workspace open, as `options` says.
    pub fn start(options: Options) -> Self {
        let (arguments, ready): (&[&str], &str) = match options.desktop {
            true => (&["--desktop"], "Pane \"Desktop\""),
            false => (&[], FILES),
        };
        let mut start = Self::program(options.space, options.space || options.previews);
        start.arguments = std::iter::once("--dev-workspace")
            .chain(arguments.iter().copied())
            .map(str::to_owned)
            .collect();
        Self::begin(start, ready, options.space, None)
    }

    /// Starts the host with the staged app `app`'s entry point as the root guest in place of
    /// its own UI (`--root=APP`), with no workspace. It draws through lavapipe, since the root's
    /// apps draw on the window's device, and shows only what the root draws, so look at it with
    /// [`HostTest::shot`]: the window has no painting of it yet.
    pub fn root(app: &str) -> Self {
        let mut start = Self::program(false, true);
        start.arguments = vec![format!("--root={app}")];
        Self::begin(start, ROOT_SHOWN, false, None)
    }

    /// Starts the host with the staged app `app` as its root, as [`HostTest::root`] does, with
    /// the dev workspace open (`--dev-workspace`): the root shows be-host's block stack signed
    /// in. Wait for what the root shows with `settle`.
    pub fn root_with_dev_workspace(app: &str) -> Self {
        let mut start = Self::program(false, true);
        start.arguments = vec![format!("--root={app}"), "--dev-workspace".to_owned()];
        Self::begin(start, ROOT_SHOWN, false, None)
    }

    /// Starts a device of `account` on its test [`Server`], in a data directory of its own,
    /// and opens its My files as `start` does: the account's first device, which sets its key.
    /// Start its other devices with [`HostTest::another_device`], which unlocks them with that
    /// key.
    pub fn device(account: &Account) -> Self {
        Self::join(account, FILES)
    }

    /// The address this device reaches its test [`Server`] at: a link made on another device
    /// names that one's, which this device rewrites to its own to open it.
    pub fn server_url(&self) -> String {
        format!("http://{}", self.joined("server_url").relay.address())
    }

    /// Starts another device of this one's account, as [`HostTest::device`] does, and pairs it
    /// with this one, as a person adding a device does: the new device shows a code, which is
    /// typed into this one. It returns once the new device shows its My files.
    pub fn another_device(&mut self) -> Self {
        let account = self.joined("another_device").account.clone();
        let mut other = Self::join(&account, SHOW_A_CODE);
        other.click(SHOW_A_CODE);
        other.wait("Type this code on the other device:");
        let tree = other.tree();
        let code = tree
            .split('"')
            .find(|word| pairing_code(word))
            .unwrap_or_else(|| other.fail("the new device shows no pairing code"))
            .to_owned();
        self.wait("Add a device");
        self.click("TextInput \"Code\"");
        self.type_text(&code);
        self.click("Button \"Approve\"");
        self.gone("Add a device");
        other.wait(FILE_TREE);
        other
    }

    /// Closes the host, failing unless it exits cleanly, and starts it again on the data it
    /// kept, as a person quitting it and opening it again. A device keeps its network: one
    /// [`HostTest::offline`] starts offline.
    pub fn restart(&mut self) {
        self.run(&["settle"]);
        self.steps.push("restart".to_owned());
        let restarted = self
            .host
            .as_mut()
            .expect("the host is still open")
            .restart();
        if let Err(error) = restarted {
            self.fail(&format!("the host did not restart: {error}"));
        }
        self.wait(FILES);
        self.run(&["clock", "stop"]);
    }

    /// Closes the host, failing unless it exits cleanly, and starts it again on the data it kept
    /// with the staged app `app`'s entry point as the root guest in place of its own UI, on the
    /// dev workspace (`--root=APP --dev-workspace`), as [`HostTest::root_with_dev_workspace`]
    /// does: what be-host's own UI made, the root shows. Start the host with
    /// [`Options::previews`], since the root's apps draw on the window's device.
    pub fn restart_as_root(&mut self, app: &str) {
        self.end_programs();
        self.run(&["settle"]);
        self.steps.push(format!("restart --root={app}"));
        let restarted = self
            .host
            .as_mut()
            .expect("the host is still open")
            .restart_with(
                [format!("--root={app}"), "--dev-workspace".to_owned()]
                    .into_iter()
                    .chain(self.desktop.then(|| "--desktop".to_owned()))
                    .collect(),
            );
        if let Err(error) = restarted {
            self.fail(&format!("the host did not restart: {error}"));
        }
        self.wait(ROOT_SHOWN);
        self.run(&["clock", "stop"]);
    }

    /// Takes this device's network to the server away: its connections drop and it cannot
    /// connect, as a laptop that lost its Wi-Fi. It returns once the device knows
    /// (`drive offline`). What it does meanwhile waits in its outbox.
    pub fn offline(&mut self) {
        self.steps.push("# offline".to_owned());
        self.joined("offline").relay.offline(true);
        self.run(&["offline"]);
    }

    /// Takes this device's network away without a word, as a network that drops what it is sent
    /// and answers nothing: its connections stay open but carry nothing, and a new one is
    /// accepted and never answered. Unlike [`HostTest::offline`], the device is not told and does
    /// not know at once; it returns at once.
    pub fn unreachable(&mut self) {
        self.steps.push("# unreachable".to_owned());
        let relay = &self.joined("unreachable").relay;
        relay.hold_new(true);
        relay.stall();
    }

    /// Gives this device its network back and tells it so (`drive network`), as the system tells
    /// a program the network came back, so it reconnects at once instead of when its wait for
    /// the next try runs out. Wait for what it should catch up on with [`HostTest::synced`].
    pub fn online(&mut self) {
        self.steps.push("# online".to_owned());
        let relay = &self.joined("online").relay;
        // Connections held by `unreachable` never reached the server: end them, as a
        // network that comes back resets what timed out on it.
        relay.hold_new(false);
        relay.cut();
        relay.offline(false);
        self.run(&["network"]);
    }

    /// Waits until the device is connected to its server, has sent everything it kept, and has
    /// asked for what changed while it was away (`drive synced`). What another device changes
    /// arrives as the server pushes it, after that: wait for it to show.
    pub fn synced(&mut self) {
        self.run(&["synced"]);
    }

    fn joined(&self, what: &str) -> &Device {
        self.device.as_ref().unwrap_or_else(|| {
            self.fail(&format!(
                "{what} is for a device of a test's server; start it with HostTest::device"
            ))
        })
    }

    fn join(account: &Account, ready: &str) -> Self {
        let relay = account.server.relay();
        let url = format!("http://{}", relay.address());
        let token = account.server.token(account.id);
        let start = be_tool_drive::Start {
            arguments: vec![format!("--dev-workspace={url}")],
            environment: vec![("BE_DEV_TOKEN".to_owned(), token)],
            ..Self::program(false, false)
        };
        Self::begin(
            start,
            ready,
            false,
            Some(Device {
                account: account.clone(),
                relay,
            }),
        )
    }

    fn program(space: bool, render: bool) -> be_tool_drive::Start {
        be_tool_drive::Start {
            program: std::env::var("BE_HOST")
                .expect("BE_HOST names the host to test; run these through ./scripts/buck test //crates/be-host-tests:test"),
            size: SIZE.to_owned(),
            libraries: std::env::var("HOST_LIBRARIES").ok(),
            space,
            render,
            ..be_tool_drive::Start::default()
        }
    }

    fn begin(
        start: be_tool_drive::Start,
        ready: &str,
        space: bool,
        device: Option<Device>,
    ) -> Self {
        let started = Instant::now();
        let desktop = start
            .arguments
            .iter()
            .any(|argument| argument == "--desktop");
        let host = Driven::start(start)
            .unwrap_or_else(|error| panic!("be-host did not start: {error}{}", log_named(&error)));
        let mut test = Self {
            host: Some(host),
            steps: Vec::new(),
            recorded: None,
            overflows: Vec::new(),
            crop: None,
            started,
            reported: Cell::new(false),
            space,
            desktop,
            programs: Vec::new(),
            editors: std::env::var("HOST_TEST_APPS").ok(),
            device,
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
        self.check_stopped();
        self.run(&["wait", target]);
    }

    /// Fails at once, naming the app, when one has stopped (it panicked, trapped or stopped
    /// responding, guides/apps/runtime.md, When an app stops), rather than when a wait for what
    /// it would have shown runs out: the host says so in a toast, `NAME stopped: REASON`.
    fn check_stopped(&mut self) {
        let tree = self.tree();
        let Some(toast) = tree.lines().find_map(|line| {
            let said = line.trim().strip_prefix("Alert \"")?.split('"').next()?;
            said.contains(" stopped: ").then_some(said)
        }) else {
            return;
        };
        self.fail(&format!(
            "an app stopped while this test waited: {toast}. \"The app stopped responding\" is a step that ran past STEP_TIMEOUT, which a crowded build server can cause."
        ));
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
        self.run(&["wait", "not", target]);
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

    /// Moves the stopped clock on until `target` stays put between two steps: a sheet that
    /// has come to rest on its stop.
    pub fn wait_still(&mut self, target: &str) {
        // Read from the tree, which names it off screen too (a sheet yet to rise).
        let placed = |host: &mut Self| {
            let tree = host.tree();
            tree.lines()
                .find(|line| line.split_whitespace().any(|word| word == target))
                .and_then(|line| line.split(" at ").nth(1))
                .map(|place| {
                    place
                        .split_whitespace()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
        };
        let mut was = placed(self);
        for _ in 0..STILL_STEPS {
            self.pause(STILL_STEP);
            let now = placed(self);
            if now == was {
                return;
            }
            was = now;
        }
        self.fail(&format!(
            "{target} was still moving after {STILL_STEPS} steps of {STILL_STEP} ms"
        ));
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

    /// What the window shows, as the GPU drew it (`drive shot`): its width and height in
    /// pixels and its RGBA, row by row. Only a host drawing through lavapipe has one
    /// ([`Options::previews`], [`HostTest::root`]).
    pub fn shot(&mut self) -> (u32, u32, Vec<u8>) {
        let words = ["shot"];
        self.steps.push(line(&words));
        match self.driven().request(&words) {
            Ok(Reply::Image {
                width,
                height,
                rgba,
            }) => (width, height, rgba),
            Ok(_) => self.fail("shot answered with something other than an image"),
            Err(error) => self.fail(&error),
        }
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
        if painted != again {
            self.fail(
                "the painting is not settled: it changed between two frames, so something was still arriving. Wait for it (`wait`, `wait not`) before the snapshot, or park the pointer (`leave`) if it is a hover",
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

    /// Runs the test's program window (be-tool-wayland-window) titled `title` in the desktop's
    /// compositor, as `drive exec` runs a program: with the variables the host gives programs
    /// (`drive env`), under `--desktop` only. It runs until the host closes it or the test ends.
    pub fn open_test_window(&mut self, title: &str) {
        assert!(
            self.desktop,
            "a test window runs only in the desktop's compositor: start the host with Options::desktop"
        );
        self.steps
            .push(format!("exec $WAYLAND_TEST_WINDOW {title}"));
        let environment = self.run(&["env"]);
        let program = std::env::var("WAYLAND_TEST_WINDOW").expect(
            "WAYLAND_TEST_WINDOW names the test's program window; run these through ./scripts/buck test //crates/be-host-tests:test",
        );
        let mut command = std::process::Command::new(program);
        command
            .arg(title)
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY");
        // It loads the Wayland client library from the test's sysroot, as the host loads its own.
        if let Ok(libraries) = std::env::var("HOST_LIBRARIES") {
            command.env("LD_LIBRARY_PATH", libraries);
        }
        for (name, value) in environment.lines().filter_map(|line| line.split_once('=')) {
            command.env(name, value);
        }
        match command.spawn() {
            Ok(child) => self.programs.push(child),
            Err(error) => self.fail(&format!("the test window did not start: {error}")),
        }
    }

    /// Closes the host once it has settled, so nothing is left unsaved for it to ask about, and
    /// fails unless it exits cleanly.
    pub fn close(mut self) {
        self.end_programs();
        self.run(&["settle"]);
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
                // What it is out of step with, which a wait for something another device did
                // may be waiting on, though a wait's own timeout names only what keeps it busy.
                let synced = match host.request_within(&["synced"], OUT_OF_STEP_WAIT) {
                    Ok(_) => "in step".to_owned(),
                    Err(error) => error,
                };
                format!(
                    "\n\nWhat the host shows:\n{tree}\n\nWhether it is in step with the server: {synced}\n\n{}",
                    host.log_report()
                )
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

impl HostTest {
    /// Ends the test's programs, before the host closes or restarts, once each window is gone
    /// from the desktop, so the host does not close while it changes.
    fn end_programs(&mut self) {
        if self.programs.is_empty() {
            return;
        }
        self.stop_programs();
        self.run(&["wait", " #desktop.windows.none"]);
    }

    fn stop_programs(&mut self) {
        for mut program in self.programs.drain(..) {
            let _ = program.kill();
            let _ = program.wait();
        }
    }
}

impl Drop for HostTest {
    fn drop(&mut self) {
        self.stop_programs();
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

/// A pairing code as the unlock screen shows it: `XXXX-XXXX`.
fn pairing_code(word: &str) -> bool {
    let bytes = word.as_bytes();
    bytes.len() == 9
        && bytes.iter().enumerate().all(|(at, byte)| match at {
            4 => *byte == b'-',
            _ => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
        })
}

fn missing(error: &str) -> bool {
    error.starts_with("no line of the tree contains")
        || error.starts_with("no node on screen has the test id")
}

/// The end of the log an error from `Driven::start` names (`see PATH`), which says why the host
/// stopped before it answered.
fn log_named(error: &str) -> String {
    let Some((_, path)) = error.rsplit_once("see ") else {
        return String::new();
    };
    match std::fs::read_to_string(path.trim()) {
        Ok(log) => {
            let lines: Vec<&str> = log.lines().collect();
            let tail = &lines[lines.len().saturating_sub(40)..];
            format!("\n\nThe end of its log:\n{}", tail.join("\n"))
        }
        Err(_) => String::new(),
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
