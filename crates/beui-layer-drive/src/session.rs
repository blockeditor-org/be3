use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use beui_core::app::FrameTimes;
use beui_core::app::accessibility_dump::{ACTIONS, AccessibilityDump, Line, Rendered};
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::context::{ActionGroup, Context, FrameOutput};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest, PickedFile};
use beui_core::file_saver::{FileSave, FileSaveAnswer, FileSaveRequest};
use beui_core::geometry::{Pos2, Rect, Vec2, vec2};
use beui_core::input::Keyboard;
use beui_core::input::{BackEdge, DroppedFile, FileDrag, Input, Key, PointerButton, TouchPhase};
use beui_core::renderer::Capture;
use beui_core::runner::{Frame, Incoming, Layer, LayerFrame, PlatformServices, Shown};
use beui_core::window::{Resize, WindowId, WindowSize, WindowState};

use crate::gestures::{Frames, Gestures};
use crate::target::{coordinates, describe, locate, node, point, scaled};
use crate::{Inbox, Reply, Request, changes};

const CHANGES: usize = 60;
const FRAME: Duration = Duration::from_micros(16_667);
const CLIPBOARD_WAIT: Duration = Duration::from_secs(5);

pub const USAGE: &str = "Every command names where it goes: the screen tap, nearest the platform, takes
coordinates and stands in for the platform's services; the app tap, nearest the app, reads the
accessibility tree and names targets in it; the session's own commands name no tap. With one
drive layer in the stack both taps are that one layer, and either name reaches every command.
Session:
  settle                    wait until nothing is left to draw
  pause MILLISECONDS        let that much time pass, by the app's clock
  clock stop / clock run    stop the app's clock, so time passes only with pause, frame by frame
  hurry off / hurry on      stop or start the app's shortcuts for a driven run, such as saving at once,
                            so it waits as it does for a person
  frames                    how many frames ran since the last frames, and the longest one's wall time:
                            the window's, and those the app ran elsewhere, such as a plugin's
  close                     ask the window to close, as its close button does; an app that stays
                            open, to ask about unsaved changes, answers with what changed
  help                      this
app (reading):
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  comps [--internals]       component tree, optionally including component internals
  hit TARGET                what a press at TARGET would reach: the Interactives along its path,
                            the components that made them and what each handles
  actions [all]             the actions the command palette would list (all: every one)
  act ID [PANE]             run the action ID as the command palette would, in PANE's plugin when given
  a11y TARGET               the node a screen reader would act on, and the actions it offers
  a11y TARGET ACTION [VALUE]   do what a screen reader does: click, focus, increment, set VALUE, ...
  wait TEXT|#TEST_ID [over X,Y]   wait until a line of the tree contains TEXT, or the test id is on
                            screen, with its node over the point X,Y if given
  gone TEXT|#TEST_ID [over X,Y]   wait until it is not
  env                       NAME=VALUE lines a program needs to show its windows inside the app,
                            such as WAYLAND_DISPLAY; empty when the app hosts no programs
  windows                   the app's windows, each one's number and size
  paint [TARGET] [in WINDOW]   what a window of the app paints (the main one unless WINDOW names
                            another), plugins included, as a paint snapshot, of TARGET only when given
  overflows                 what the app shows out of bounds, plugins included, one per line:
                            cut off where nothing scrolls to it, drawn outside its own box, or
                            with its outline cut while its box shows
screen (the platform's side):
  shot [TARGET]             a screenshot of what the platform shows, of TARGET only when given
  resize WIDTHxHEIGHT[@SCALE]   resize the headless window, and its scale when given
  resize screen WIDTHxHEIGHT    the screen a fullscreen headless window fills
  focus / blur              the window gains or loses focus; blur cancels the pointers and lets go of held keys
  clipboard [TEXT]          read the clipboard, or put TEXT on it
  upload FILE [TARGET]      answer the file dialog with FILE, clicking TARGET first to open it
  save FILE [TARGET]        answer the save dialog by writing what it saves to FILE, clicking
                            TARGET first to open it
  dismiss                   cancel the oldest open file or save dialog
  grab FILE...              drag FILEs in from outside, over where the pointer is; move carries them
  drop [TARGET]             drop the grabbed files; ungrab takes them away again
Either tap (input enters the stack at the tap; state is what that tap sees):
  state                     window, cursor, pointer, fingers, held keys, input method, dialogs
  click TARGET [right|middle|double]
  move TARGET               move the pointer onto TARGET (hover)
  down TARGET [BUTTON] / up [TARGET] [BUTTON]   press and let go, for any gesture; BUTTON is
                            right, middle, back, forward or button:CODE (an evdev code,
                            such as button:0x115)
  drag FROM TO [STEPS]      press, move in STEPS (2), let go
  wheel TARGET TICKS [XTICKS]   turn a mouse wheel, down (right) when positive
  scroll TARGET DY [DX]     a touchpad scroll of DY pixels, down when positive
  zoom TARGET FACTOR        a touchpad pinch, such as 2 to zoom in or 0.5 out
  leave                     the pointer leaves the window
  motion DX DY              a mouse moving DX, DY pixels with the pointer locked, as a game's look
                            or a host's steering reads it
  tap TARGET [FINGER]
  swipe FROM TO [FINGER]
  pinch TARGET FACTOR       two fingers spreading (FACTOR > 1) or closing around TARGET
  touch down|move|up|cancel [FINGER=]TARGET...   fingers 0, 1, ... step by step, all in one
                            frame; cancel is the system taking a finger back, as for a gesture
  touch up / touch cancel   lift or cancel every finger
  pen move TARGET [PRESSURE [STEPS [PER_FRAME]]]   the pen hovers over TARGET, or moves there
                            pressed down with PRESSURE, in STEPS (1) samples, PER_FRAME (1) in
                            each frame
  pen down TARGET [PRESSURE]   the tip touches TARGET, PRESSURE from 0 to 1 (0.5)
  pen up [TARGET]           the tip lifts; the pen stays in range
  pen draw FROM TO [STEPS [PER_FRAME]]   the tip down at FROM, along in STEPS (8) with the
                            pressure rising and falling, PER_FRAME (1) samples in each frame, and
                            up at TO
  pen leave                 the pen goes out of range
  back                      the system back gesture, whole
  back [left|right] / back PROGRESS / back commit|cancel   step by step, PROGRESS from 0 to 1
  type TEXT                 type TEXT key by key, as a US keyboard would (a newline presses Enter);
                            a character no key makes arrives as text alone
  key CHORD...              press and let go of each chord, such as ctrl+z or Enter
  keydown KEY... / keyup KEY...   hold keys down and let go; keydown on a held key repeats it
  ime compose TEXT / ime commit [TEXT] / ime cancel   an input method's composition
A TARGET is #TEST_ID, #TEST_ID+X,Y (that far from its top left corner), X,Y or X,Y,WIDTH,HEIGHT
in the tree's pixels, or text found in one line of the tree. Where another layer sits between
the taps, the screen tap takes only X,Y and X,Y,WIDTH,HEIGHT, in the platform window's pixels.
Every command that gives input waits for the app to settle and answers with what changed in the tree,
at most 60 lines of it; before the tap, --changes=N|all changes that (--changes=0 answers with
nothing, without comparing the trees), and --no-settle answers after the frame that took the input
instead of waiting.";

pub const PANE: char = '\u{1f}';

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tap {
    Screen,
    App,
}

impl Tap {
    fn name(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::App => "app",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Session,
    Only(Tap),
    Either,
}

fn kind(command: &str) -> Option<Kind> {
    Some(match command {
        "help" | "settle" | "pause" | "clock" | "hurry" | "frames" | "close" => Kind::Session,
        "tree" | "ids" | "find" | "comps" | "hit" | "actions" | "act" | "a11y" | "wait" | "gone"
        | "env" | "windows" | "paint" | "overflows" => Kind::Only(Tap::App),
        "shot" | "resize" | "focus" | "blur" | "clipboard" | "upload" | "save" | "dismiss"
        | "grab" | "drop" | "ungrab" => Kind::Only(Tap::Screen),
        "state" | "click" | "move" | "down" | "up" | "drag" | "wheel" | "scroll" | "zoom"
        | "leave" | "motion" | "tap" | "swipe" | "pinch" | "touch" | "pen" | "back" | "type"
        | "key" | "keydown" | "keyup" | "ime" => Kind::Either,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Serves {
    Both,
    Only(Tap),
}

impl Serves {
    fn serves(self, tap: Tap) -> bool {
        match self {
            Self::Both => true,
            Self::Only(only) => only == tap,
        }
    }
}

pub struct Drive {
    session: Rc<RefCell<Session>>,
    serves: Serves,
    context: Option<Context>,
}

impl Drive {
    pub fn new(inbox: Inbox) -> Self {
        Self {
            session: Rc::new(RefCell::new(Session::new(inbox, false))),
            serves: Serves::Both,
            context: None,
        }
    }

    pub fn taps(inbox: Inbox) -> (Self, Self) {
        let session = Rc::new(RefCell::new(Session::new(inbox, true)));
        let tap = |tap: Tap| Self {
            session: Rc::clone(&session),
            serves: Serves::Only(tap),
            context: None,
        };
        (tap(Tap::Screen), tap(Tap::App))
    }

    fn screen(&self) -> bool {
        self.serves.serves(Tap::Screen)
    }
}

impl Layer for Drive {
    fn attach(&mut self, context: &Context) {
        context.keep_accessibility(true);
        context.set_automated(true);
        self.context = Some(context.clone());
    }

    fn setup(&mut self, setup: &beui_core::app::Setup) {
        self.session.borrow().inbox.set_waker(setup.waker.clone());
    }

    fn now(&mut self, now: Instant) -> Instant {
        match (self.screen(), &self.context) {
            (true, Some(context)) => self.session.borrow().step_clock(now, context),
            _ => now,
        }
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, frame: &mut LayerFrame<'_>) {
        let mut session = self.session.borrow_mut();
        if self.screen() {
            let context = frame.context;
            if let Some(dump) = context.kept_accessibility() {
                let view = View::of(&dump, frame);
                session.begin(&view, frame);
            }
        }
        session.push_steps(self.serves, incoming);
    }

    fn output(&mut self, shown: &mut Shown, _output: &FrameOutput, _frame: &mut LayerFrame<'_>) {
        if self.serves == Serves::Only(Tap::Screen)
            && let Some(main) = shown.main()
        {
            self.session.borrow_mut().screen_window = Some(main.window.clone());
        }
    }

    fn framed(&mut self, run: &mut Frame, output: &FrameOutput, frame: &mut LayerFrame<'_>) {
        if !self.screen() {
            return;
        }
        let context = frame.context;
        let Some(dump) = context.kept_accessibility() else {
            return;
        };
        let mut session = self.session.borrow_mut();
        session.app_window = output.main().window.clone();
        let view = View::of(&dump, frame);
        session.end(&view, run, output, frame);
    }

    fn wants_sight(&self) -> bool {
        self.screen() && self.session.borrow().wants_sight()
    }

    fn wants_capture(&self) -> bool {
        self.screen() && self.session.borrow().wants_capture()
    }

    fn captured(&mut self, capture: Option<Capture>) {
        self.session.borrow_mut().captured(capture);
    }

    fn pick_file(
        &mut self,
        request: FilePickRequest,
        answer: FilePickAnswer,
        below: &mut dyn PlatformServices,
    ) {
        match self.screen() && !cfg!(target_arch = "wasm32") {
            true => self.session.borrow_mut().pick_file(request, answer),
            false => below.pick_file(request, answer),
        }
    }

    fn save_file(
        &mut self,
        request: FileSaveRequest,
        answer: FileSaveAnswer,
        below: &mut dyn PlatformServices,
    ) {
        match self.screen() && !cfg!(target_arch = "wasm32") {
            true => self.session.borrow_mut().save_file(request, answer),
            false => below.save_file(request, answer),
        }
    }
}

struct View<'a> {
    accessibility: &'a AccessibilityDump,
    pixels_per_point: f32,
    now: Instant,
    platform: Vec2,
    app: Vec2,
    fullscreen: bool,
}

impl<'a> View<'a> {
    fn of(accessibility: &'a AccessibilityDump, frame: &LayerFrame<'_>) -> Self {
        let pixels_per_point = frame.view.pixels_per_point;
        let platform = frame.view.platform / pixels_per_point;
        Self {
            accessibility,
            pixels_per_point,
            now: frame.context.now(),
            platform,
            app: frame
                .context
                .window_size(WindowId::MAIN)
                .unwrap_or(platform),
            fullscreen: frame.context.fullscreen(),
        }
    }

    fn rendered(&self) -> Rc<Rendered> {
        self.accessibility.rendered()
    }
}

#[derive(Clone, Copy)]
struct Settled {
    busy: bool,
    again: bool,
    drawing: bool,
}

enum Phase {
    Input {
        steps: VecDeque<Vec<Incoming>>,
        before: Rc<Rendered>,
        then: Option<Box<Phase>>,
    },
    Settling {
        before: Option<Rc<Rendered>>,
    },
    Waiting {
        text: String,
        present: bool,
        over: Option<Pos2>,
    },
    Pausing {
        until: Instant,
    },
    Acting {
        id: String,
        before: Rc<Rendered>,
    },
    Actions {
        all: bool,
    },
    Windows,
    Hitting,
    Components,
    Uploading {
        file: PathBuf,
        before: Rc<Rendered>,
    },
    Saving {
        file: PathBuf,
        before: Rc<Rendered>,
    },
    Capturing {
        target: Option<String>,
        crop: Option<Rect>,
        drawn: bool,
    },
    Painting {
        asked: bool,
        ready: bool,
        target: Option<String>,
        crop: Option<[f32; 4]>,
        window: u64,
        overflows: bool,
    },
    Reading {
        pasted: std::sync::mpsc::Receiver<Pasted>,
        until: Instant,
    },
}

struct Active {
    request: Request,
    phase: Phase,
    tap: Tap,
    immediate: bool,
    changes: Option<usize>,
}

enum Started {
    Done(Reply),
    Phase(Phase),
}

struct Session {
    inbox: Inbox,
    split: bool,
    active: Option<Active>,
    flush: Option<(Tap, Vec<Incoming>)>,
    gestures: [Gestures; 2],
    grabbed: Vec<DroppedFile>,
    clock_stopped: bool,
    unhurried: bool,
    focused: bool,
    picks: Vec<(u64, FilePickRequest, FilePickAnswer)>,
    saves: Vec<(u64, FileSaveRequest, FileSaveAnswer)>,
    dialogs: u64,
    frames: FrameTimes,
    other_frames: BTreeMap<String, FrameTimes>,
    screen_window: Option<WindowState>,
    app_window: WindowState,
}

impl Session {
    fn new(inbox: Inbox, split: bool) -> Self {
        Self {
            inbox,
            split,
            active: None,
            flush: None,
            gestures: [Gestures::default(), Gestures::default()],
            grabbed: Vec::new(),
            clock_stopped: false,
            unhurried: false,
            focused: true,
            picks: Vec::new(),
            saves: Vec::new(),
            dialogs: 0,
            frames: FrameTimes::default(),
            other_frames: BTreeMap::new(),
            screen_window: None,
            app_window: WindowState::default(),
        }
    }

    fn reads_tree(&self, tap: Tap) -> bool {
        tap == Tap::App || !self.split
    }

    fn gesture_index(&self, tap: Tap) -> usize {
        match (self.split, tap) {
            (true, Tap::App) => 1,
            _ => 0,
        }
    }

    fn take_frames(&mut self) -> String {
        let window = std::mem::take(&mut self.frames);
        let others = std::mem::take(&mut self.other_frames);
        std::iter::once(("window".to_owned(), window))
            .chain(others)
            .map(|(name, times)| {
                format!(
                    "{name}: {} frames, the longest {:.3} ms\n",
                    times.count,
                    times.longest.as_secs_f64() * 1000.0
                )
            })
            .collect()
    }

    fn step_clock(&self, now: Instant, context: &Context) -> Instant {
        if let Some(Active {
            phase: Phase::Pausing { until },
            ..
        }) = &self.active
            && self.clock_stopped
        {
            let step = until.saturating_duration_since(context.now()).min(FRAME);
            context.advance_clock(step);
            return now + step;
        }
        now
    }

    fn begin(&mut self, view: &View<'_>, frame: &mut LayerFrame<'_>) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request.cancelled.load(Ordering::SeqCst))
            && let Some(active) = self.active.take()
            && let Phase::Input { steps, .. } = active.phase
        {
            self.flush = Some((active.tap, steps.into_iter().flatten().collect()));
        }
        while self.active.is_none() {
            let Some(request) = self.inbox.take() else {
                break;
            };
            if request.cancelled.load(Ordering::SeqCst) {
                continue;
            }
            let mut immediate = false;
            let mut changes = Some(CHANGES);
            let mut skipped = 0;
            let mut failed = None;
            for word in &request.words {
                if word == "--no-settle" {
                    immediate = true;
                } else if let Some(count) = word.strip_prefix("--changes=") {
                    changes = match count {
                        "all" => None,
                        count => match count.parse() {
                            Ok(count) => Some(count),
                            Err(_) => {
                                failed =
                                    Some(format!("--changes takes a number or all, not {count}"));
                                break;
                            }
                        },
                    };
                } else {
                    break;
                }
                skipped += 1;
            }
            let started = match failed {
                Some(error) => Err(error),
                None => self.start(&request.words[skipped..], view, frame),
            };
            match started {
                Ok((_, Started::Done(reply))) => (request.reply)(reply),
                Ok((tap, Started::Phase(phase))) => {
                    self.active = Some(Active {
                        request,
                        phase,
                        tap,
                        immediate,
                        changes,
                    });
                }
                Err(error) => (request.reply)(Reply::Error(error)),
            }
        }
        if let Some(Active {
            phase: Phase::Painting { asked, .. },
            ..
        }) = &mut self.active
            && !*asked
        {
            *asked = true;
            frame.app.request_painting();
        }
    }

    fn push_steps(&mut self, serves: Serves, incoming: &mut Vec<Incoming>) {
        if let Some((tap, _)) = &self.flush
            && serves.serves(*tap)
            && let Some((_, events)) = self.flush.take()
        {
            incoming.extend(events);
        }
        let Some(Active {
            phase: Phase::Input { steps, .. },
            tap,
            ..
        }) = &mut self.active
        else {
            return;
        };
        if !serves.serves(*tap) {
            return;
        }
        let Some(step) = steps.pop_front() else {
            return;
        };
        for event in &step {
            if let Incoming::Input {
                input: Input::Focus(focused),
                ..
            } = event
            {
                self.focused = *focused;
            }
        }
        incoming.extend(step);
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        self.dialogs += 1;
        self.picks.push((self.dialogs, request, answer));
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        self.dialogs += 1;
        self.saves.push((self.dialogs, request, answer));
    }

    fn wants_frame(&self, now: Instant) -> Option<Duration> {
        let stopped = self.clock_stopped;
        match &self.active.as_ref()?.phase {
            Phase::Input { steps, .. } if !steps.is_empty() => Some(Duration::ZERO),
            Phase::Pausing { .. } if stopped => Some(Duration::ZERO),
            Phase::Pausing { until } => Some(until.saturating_duration_since(now)),
            Phase::Reading { .. } => Some(FRAME),
            _ => None,
        }
    }

    fn end(
        &mut self,
        view: &View<'_>,
        run: &mut Frame,
        output: &FrameOutput,
        frame: &mut LayerFrame<'_>,
    ) {
        let took = frame.started.elapsed();
        self.frames.add(took);
        for (name, times) in frame.app.other_frames() {
            self.other_frames.entry(name).or_default().merge(times);
        }
        let settled = Settled {
            busy: frame.app.busy(),
            drawing: frame.app.drawing(),
            again: match self.clock_stopped {
                true => run.deferred,
                false => run.again(),
            },
        };
        self.settle(view, settled, output);
        if let Some(after) = self.wants_frame(view.now) {
            run.repaint |= after.is_zero();
            run.repaint_after = run.repaint_after.min(after);
        }
        if let Some(window) = self.wants_painting() {
            let painting = match output.windows.iter().find(|shown| shown.id.raw() == window) {
                Some(shown) => beui_core::painting::painting(
                    shown,
                    shown.size,
                    view.pixels_per_point,
                    shown.window.appearance.clear_color,
                    beui_core::painting::Punches::Erase,
                ),
                None => Err(format!(
                    "the app has no window {window}; `windows` lists them"
                )),
            };
            self.painted(painting);
        }
        if self.wants_overflows() {
            self.overflowed(beui_core::painting::overflows(output, view.pixels_per_point));
        }
    }

    fn settle(&mut self, view: &View<'_>, settled: Settled, output: &FrameOutput) {
        let quiet = !settled.again && !settled.busy;
        let Some(active) = &mut self.active else {
            return;
        };
        let reads = !self.split || active.tap == Tap::App;
        let settling = |before: &mut Rc<Rendered>| Phase::Settling {
            before: Some(Rc::clone(before)),
        };
        let mut failed = None;
        match &mut active.phase {
            Phase::Capturing {
                target,
                crop,
                drawn,
            } => {
                *drawn = !settled.drawing;
                if *drawn && let Some(target) = target {
                    match place(reads, target, view) {
                        Ok(rect) => *crop = Some(rect),
                        Err(error) => failed = Some(error),
                    }
                }
            }
            Phase::Painting {
                asked: true,
                ready,
                target,
                crop,
                ..
            } => {
                *ready = quiet;
                if *ready && let Some(target) = target {
                    match place(reads, target, view) {
                        Ok(rect) => *crop = Some(in_points(rect, view.pixels_per_point)),
                        Err(error) => failed = Some(error),
                    }
                }
            }
            _ => {}
        }
        match &mut active.phase {
            Phase::Input {
                steps,
                before,
                then,
            } if steps.is_empty() => {
                active.phase = match then.take() {
                    Some(then) => *then,
                    None => settling(before),
                };
            }
            Phase::Acting { id, before } => {
                match output.unran_actions.iter().any(|unran| unran == id) {
                    true => {
                        let named = id.replace(PANE, " in ");
                        failed = Some(format!(
                            "no action is called {named}; `actions all` lists them"
                        ));
                    }
                    false => active.phase = settling(before),
                }
            }
            _ => {}
        }
        if let Phase::Uploading { file, before } = &mut active.phase {
            let picked = (!self.picks.is_empty()).then(|| self.picks.remove(0));
            match picked {
                Some((_, _, answer)) => match std::fs::read(&*file) {
                    Ok(data) => {
                        let name = file
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        answer.send(Ok(Some(PickedFile { name, data })));
                        active.phase = settling(before);
                        return;
                    }
                    Err(error) => {
                        answer.send(Ok(None));
                        failed = Some(format!("could not read {}: {error}", file.display()));
                    }
                },
                None if quiet => {
                    failed = Some(
                        "no file dialog opened; give upload a TARGET that opens one".to_owned(),
                    );
                }
                None => {}
            }
        }
        if let Phase::Saving { file, before } = &mut active.phase {
            let saved = (!self.saves.is_empty()).then(|| self.saves.remove(0));
            match saved {
                Some((_, request, answer)) => match std::fs::write(&*file, &request.file.data) {
                    Ok(()) => {
                        answer.send(FileSave::Saved);
                        active.phase = settling(before);
                        return;
                    }
                    Err(error) => {
                        let error = format!("could not write {}: {error}", file.display());
                        answer.send(FileSave::Failed(error.clone()));
                        failed = Some(error);
                    }
                },
                None if quiet => {
                    failed =
                        Some("no save dialog opened; give save a TARGET that opens one".to_owned());
                }
                None => {}
            }
        }
        let immediate = active.immediate;
        let limit = active.changes;
        let reply = match failed {
            Some(error) => Some(Reply::Error(error)),
            None => match &mut active.phase {
                Phase::Settling { before } if quiet || (immediate && before.is_some()) => {
                    Some(Reply::Text(match (before.as_deref(), limit) {
                        (None, _) | (Some(_), Some(0)) => String::new(),
                        (Some(before), limit) => {
                            capped(changes(before.text(), view.rendered().text()), limit)
                        }
                    }))
                }
                Phase::Waiting {
                    text,
                    present,
                    over,
                } if shown(view, text, *over) == *present => Some(Reply::Text(String::new())),
                Phase::Pausing { until } if view.now >= *until => Some(Reply::Text(String::new())),
                Phase::Components => output
                    .comps
                    .as_deref()
                    .map(|trees| Reply::Text(trees.join("\n"))),
                Phase::Hitting => output.hits.as_deref().map(|hits| {
                    Reply::Text(match hits.is_empty() {
                        true => "nothing in the window is there\n".to_owned(),
                        false => hits.join("\n"),
                    })
                }),
                Phase::Actions { all } => Some(Reply::Text(actions(&output.actions, *all))),
                Phase::Windows => Some(Reply::Text(
                    output
                        .windows
                        .iter()
                        .map(|window| {
                            format!(
                                "window {}{} {}x{}\n",
                                window.id.raw(),
                                if window.id.is_main() { " main" } else { "" },
                                (window.size.x * view.pixels_per_point).round(),
                                (window.size.y * view.pixels_per_point).round()
                            )
                        })
                        .collect(),
                )),
                Phase::Reading { pasted, until } => match clipboard_text(pasted) {
                    Some(text) => Some(Reply::Text(text)),
                    None if view.now >= *until => Some(Reply::Error(
                        "the window did not answer with its clipboard in time".to_owned(),
                    )),
                    None => None,
                },
                _ => None,
            },
        };
        if let Some(reply) = reply {
            self.finish(reply);
        }
    }

    fn wants_sight(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Capturing { .. },
                ..
            })
        )
    }

    fn wants_capture(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Capturing { drawn: true, .. },
                ..
            })
        )
    }

    fn wants_painting(&self) -> Option<u64> {
        match self.active {
            Some(Active {
                phase:
                    Phase::Painting {
                        ready: true,
                        overflows: false,
                        window,
                        ..
                    },
                ..
            }) => Some(window),
            _ => None,
        }
    }

    fn wants_overflows(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Painting {
                    ready: true,
                    overflows: true,
                    ..
                },
                ..
            })
        )
    }

    fn overflowed(&mut self, found: Vec<String>) {
        if self.wants_overflows() {
            self.finish(Reply::Text(
                found.into_iter().map(|line| line + "\n").collect(),
            ));
        }
    }

    fn painted(&mut self, painting: Result<beui_core::painting::Painting, String>) {
        let Some(Active {
            phase: Phase::Painting { crop, .. },
            ..
        }) = &self.active
        else {
            return;
        };
        let crop = *crop;
        self.finish(match painting {
            Ok(painting) => match crop {
                Some(crop) => Reply::Painting(painting.cropped(crop).encode()),
                None => Reply::Painting(painting.encode()),
            },
            Err(error) => Reply::Error(error),
        });
    }

    fn captured(&mut self, capture: Option<Capture>) {
        let Some(Active {
            phase: Phase::Capturing { crop, .. },
            ..
        }) = &self.active
        else {
            return;
        };
        let reply = match capture {
            Some(capture) => cropped(capture, *crop),
            None => Reply::Error(
                "this renderer cannot take screenshots; run the app headless with the wgpu renderer, or use paint".to_owned(),
            ),
        };
        self.finish(reply);
    }

    fn finish(&mut self, reply: Reply) {
        if let Some(active) = self.active.take() {
            (active.request.reply)(reply);
        }
    }

    fn start(
        &mut self,
        words: &[String],
        view: &View<'_>,
        frame: &mut LayerFrame<'_>,
    ) -> Result<(Tap, Started), String> {
        let (first, rest) = words.split_first().ok_or_else(|| USAGE.to_owned())?;
        let named = match first.as_str() {
            "screen" => Some(Tap::Screen),
            "app" => Some(Tap::App),
            _ => None,
        };
        let (command, arguments) = match named {
            Some(tap) => rest.split_first().ok_or_else(|| {
                format!("{} needs a command; `help` lists them", tap.name())
            })?,
            None => (first, rest),
        };
        let Some(kind) = kind(command) else {
            return Err(format!("unknown command {command}\n{USAGE}"));
        };
        let tap = match (kind, named) {
            (Kind::Session, None) => {
                return self
                    .session(command, arguments, view, frame)
                    .map(|started| (Tap::Screen, started));
            }
            (Kind::Session, Some(tap)) => {
                return Err(format!(
                    "{command} is the session's, not the {} tap's: drive {command}",
                    tap.name()
                ));
            }
            (Kind::Only(only), None) => {
                return Err(format!(
                    "{command} goes to the {} tap: drive {} {command}",
                    only.name(),
                    only.name()
                ));
            }
            (Kind::Either, None) => {
                return Err(format!(
                    "{command} goes to a tap: drive screen {command}, where the platform's input enters, or drive app {command}, just below the app"
                ));
            }
            (Kind::Only(only), Some(tap)) if only != tap && self.split => {
                return Err(format!(
                    "the {} tap cannot {command}; it is the {} tap's: drive {} {command}",
                    tap.name(),
                    only.name(),
                    only.name()
                ));
            }
            (_, Some(tap)) => tap,
        };
        self.command(tap, command, arguments, view, frame)
            .map(|started| (tap, started))
    }

    fn session(
        &mut self,
        command: &str,
        arguments: &[String],
        view: &View<'_>,
        frame: &mut LayerFrame<'_>,
    ) -> Result<Started, String> {
        let argument = |index: usize| -> Result<&str, String> {
            arguments
                .get(index)
                .map(String::as_str)
                .ok_or_else(|| format!("{command} needs more arguments\n{USAGE}"))
        };
        match command {
            "help" => Ok(Started::Done(Reply::Text(format!("{USAGE}\n")))),
            "frames" => Ok(Started::Done(Reply::Text(self.take_frames()))),
            "settle" => Ok(Started::Phase(Phase::Settling { before: None })),
            "close" => match frame.app.close_requested() {
                true => {
                    frame.close();
                    Ok(Started::Done(Reply::Text("The window closed.\n".to_owned())))
                }
                false => Ok(Started::Phase(Phase::Settling {
                    before: Some(view.rendered()),
                })),
            },
            "clock" => {
                match argument(0)? {
                    "stop" => frame.context.stop_clock(),
                    "run" => frame.context.run_clock(),
                    other => return Err(format!("clock takes stop or run, not {other}")),
                }
                self.clock_stopped = argument(0)? == "stop";
                Ok(Started::Phase(Phase::Settling { before: None }))
            }
            "hurry" => {
                self.unhurried = match argument(0)? {
                    "on" => false,
                    "off" => true,
                    other => return Err(format!("hurry takes on or off, not {other}")),
                };
                frame.context.set_hurried(!self.unhurried);
                Ok(Started::Phase(Phase::Settling { before: None }))
            }
            "pause" => {
                let milliseconds: u64 = argument(0)?
                    .parse()
                    .map_err(|_| "pause takes milliseconds".to_owned())?;
                Ok(Started::Phase(Phase::Pausing {
                    until: view.now + Duration::from_millis(milliseconds),
                }))
            }
            other => Err(format!("unknown command {other}\n{USAGE}")),
        }
    }

    fn command(
        &mut self,
        tap: Tap,
        command: &String,
        arguments: &[String],
        view: &View<'_>,
        frame: &mut LayerFrame<'_>,
    ) -> Result<Started, String> {
        let reads = self.reads_tree(tap);
        let gesture = self.gesture_index(tap);
        let argument = |index: usize| -> Result<&str, String> {
            arguments
                .get(index)
                .map(String::as_str)
                .ok_or_else(|| format!("{command} needs more arguments\n{USAGE}"))
        };
        let at = |target: &str| aim(reads, target, view);
        let incoming = |steps: Vec<Vec<Incoming>>| {
            Ok(Started::Phase(Phase::Input {
                steps: steps.into(),
                before: view.rendered(),
                then: None,
            }))
        };
        let input = |frames: Frames| incoming(points(frames));
        match command.as_str() {
            "tree" => Ok(Started::Done(Reply::Text(
                view.rendered().text().to_owned(),
            ))),
            "ids" => {
                let rendered = view.rendered();
                let mut ids: Vec<(&str, &Line)> = rendered
                    .lines()
                    .iter()
                    .flat_map(|line| line.test_ids.iter().map(move |id| (id.as_str(), &**line)))
                    .collect();
                ids.sort_by(|a, b| a.0.cmp(b.0));
                let text = ids
                    .into_iter()
                    .map(|(id, line)| {
                        let place = line.bounds.map(describe).unwrap_or_default();
                        let hidden = if line.visible { "" } else { " offscreen" };
                        format!("#{id} {place}{hidden}\n")
                    })
                    .collect();
                Ok(Started::Done(Reply::Text(text)))
            }
            "find" => {
                let rect = place(reads, argument(0)?, view)?;
                Ok(Started::Done(Reply::Text(format!("{}\n", describe(rect)))))
            }
            "comps" => {
                if arguments.len() > 1 || arguments.first().is_some_and(|arg| arg != "--internals")
                {
                    return Err("usage: comps [--internals]".to_owned());
                }
                frame.context.request_comps(!arguments.is_empty());
                Ok(Started::Phase(Phase::Components))
            }
            "hit" => {
                frame.context.request_hit(at(argument(0)?)?);
                Ok(Started::Phase(Phase::Hitting))
            }
            "state" => Ok(Started::Done(Reply::Text(self.state(tap, view)))),
            "env" => Ok(Started::Done(Reply::Text(
                frame
                    .app
                    .environment()
                    .into_iter()
                    .map(|(name, value)| format!("{name}={value}\n"))
                    .collect(),
            ))),
            "actions" => Ok(Started::Phase(Phase::Actions {
                all: arguments.first().map(String::as_str) == Some("all"),
            })),
            "windows" => Ok(Started::Phase(Phase::Windows)),
            "act" => {
                let id = match arguments.get(1) {
                    Some(pane) => format!("{pane}{PANE}{}", argument(0)?),
                    None => argument(0)?.to_owned(),
                };
                frame.context.request_action(id.clone());
                Ok(Started::Phase(Phase::Acting {
                    id,
                    before: view.rendered(),
                }))
            }
            "click" => {
                let at = at(argument(0)?)?;
                let (button, count) = match arguments.get(1).map(String::as_str) {
                    None => (PointerButton::Primary, 1),
                    Some("right") => (PointerButton::Secondary, 1),
                    Some("middle") => (PointerButton::Middle, 1),
                    Some("double") => (PointerButton::Primary, 2),
                    Some(other) => {
                        return Err(format!("click takes right, middle or double, not {other}"));
                    }
                };
                input(self.gestures[gesture].click(at, button, count))
            }
            "move" => input(self.gestures[gesture].hover(at(argument(0)?)?)),
            "down" | "up" => {
                let pressed = command == "down";
                let (target, button) = match (arguments.first(), arguments.get(1)) {
                    (Some(first), _) if button_named(first).is_some() => {
                        (None, button_named(first))
                    }
                    (target, button) => (target, button.and_then(|name| button_named(name))),
                };
                let button = button.unwrap_or(PointerButton::Primary);
                let target = target.map(|target| at(target)).transpose()?;
                match (pressed, target) {
                    (true, Some(at)) => input(self.gestures[gesture].down(at, button)),
                    (true, None) => Err("down needs a TARGET".to_owned()),
                    (false, at) => input(self.gestures[gesture].up(at, button)),
                }
            }
            "drag" => {
                let from = at(argument(0)?)?;
                let to = at(argument(1)?)?;
                let steps: usize = match arguments.get(2) {
                    Some(steps) => steps
                        .parse()
                        .map_err(|_| format!("{steps} is not a number of steps"))?,
                    None => 2,
                };
                input(self.gestures[gesture].drag(from, to, steps))
            }
            "wheel" => {
                let at = at(argument(0)?)?;
                let ticks = |index: usize| -> Result<i32, String> {
                    arguments.get(index).map_or(Ok(0), |ticks| {
                        ticks
                            .parse()
                            .map_err(|_| format!("{ticks} is not a number of ticks"))
                    })
                };
                input(self.gestures[gesture].wheel(at, ticks(1)?, ticks(2)?))
            }
            "zoom" => {
                let at = at(argument(0)?)?;
                input(self.gestures[gesture].zoom(at, factor(argument(1)?)?))
            }
            "pinch" => {
                let center = at(argument(0)?)?;
                input(self.gestures[gesture].pinch(center, factor(argument(1)?)?))
            }
            "scroll" => {
                let at = at(argument(0)?)?;
                let number = |index: usize| -> Result<f32, String> {
                    match arguments.get(index) {
                        None => Ok(0.0),
                        Some(text) => text
                            .parse::<f32>()
                            .map(|value| value / view.pixels_per_point)
                            .map_err(|_| format!("{text} is not a number")),
                    }
                };
                input(self.gestures[gesture].scroll(at, vec2(number(2)?, number(1)?)))
            }
            "leave" => input(self.gestures[gesture].leave()),
            "motion" => {
                let pixels = |index: usize| -> Result<f32, String> {
                    let text = argument(index)?;
                    text.parse::<f32>()
                        .map(|value| value / view.pixels_per_point)
                        .map_err(|_| format!("{text} is not a number of pixels"))
                };
                input(self.gestures[gesture].motion(vec2(pixels(0)?, pixels(1)?)))
            }
            "tap" => {
                let at = at(argument(0)?)?;
                input(self.gestures[gesture].tap(at, finger(arguments.get(1))?))
            }
            "swipe" => {
                let from = at(argument(0)?)?;
                let to = at(argument(1)?)?;
                input(self.gestures[gesture].swipe(from, to, finger(arguments.get(2))?))
            }
            "pen" => {
                let pressure = |index: usize| {
                    arguments
                        .get(index)
                        .map(|text| {
                            text.parse::<f32>()
                                .ok()
                                .filter(|pressure| (0.0..=1.0).contains(pressure))
                                .ok_or_else(|| format!("{text} is not a pressure from 0 to 1"))
                        })
                        .transpose()
                };
                let count = |index: usize, default: usize, what: &str| match arguments.get(index) {
                    Some(text) => text
                        .parse::<usize>()
                        .ok()
                        .filter(|count| *count > 0)
                        .ok_or_else(|| format!("{text} is not a number of {what} above 0")),
                    None => Ok(default),
                };
                let frames = match argument(0)? {
                    "move" => self.gestures[gesture].pen_move(
                        at(argument(1)?)?,
                        pressure(2)?,
                        count(3, 1, "steps")?,
                        count(4, 1, "samples a frame")?,
                    ),
                    "down" => self.gestures[gesture].pen_down(at(argument(1)?)?, pressure(2)?)?,
                    "up" => {
                        let to = arguments.get(1).map(|target| at(target)).transpose()?;
                        self.gestures[gesture].pen_up(to)?
                    }
                    "draw" => {
                        let from = at(argument(1)?)?;
                        let to = at(argument(2)?)?;
                        let steps = count(3, 8, "steps")?;
                        let per_frame = count(4, 1, "samples a frame")?;
                        self.gestures[gesture].pen_draw(from, to, steps, per_frame)?
                    }
                    "leave" => self.gestures[gesture].pen_leave()?,
                    other => {
                        return Err(format!(
                            "pen takes move, down, up, draw or leave, not {other}"
                        ));
                    }
                };
                input(frames)
            }
            "touch" => {
                let phase = match argument(0)? {
                    "down" => TouchPhase::Start,
                    "move" => TouchPhase::Move,
                    "up" => TouchPhase::End,
                    "cancel" => TouchPhase::Cancel,
                    other => {
                        return Err(format!("touch takes down, move, up or cancel, not {other}"));
                    }
                };
                let fingers = arguments[1..]
                    .iter()
                    .map(|spec| {
                        let (finger, target) = finger_target(spec);
                        Ok((finger, target.map(&at).transpose()?))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                input(self.gestures[gesture].touch(phase, &fingers)?)
            }
            "back" => {
                let frames = match arguments.first().map(String::as_str) {
                    None => self.gestures[gesture].back(),
                    Some("left") => self.gestures[gesture].back_from(BackEdge::Left)?,
                    Some("right") => self.gestures[gesture].back_from(BackEdge::Right)?,
                    Some("commit") => self.gestures[gesture].back_end(true)?,
                    Some("cancel") => self.gestures[gesture].back_end(false)?,
                    Some(progress) => {
                        let progress: f32 = progress
                            .parse()
                            .ok()
                            .filter(|progress: &f32| (0.0..=1.0).contains(progress))
                            .ok_or(
                                "back takes left, right, a progress from 0 to 1, commit or cancel",
                            )?;
                        self.gestures[gesture].back_to(progress)
                    }
                };
                input(frames)
            }
            "type" => input(self.gestures[gesture].type_text(&arguments.join(" "))),
            "key" => {
                if arguments.is_empty() {
                    return Err(format!("key needs a chord, such as ctrl+z\n{USAGE}"));
                }
                input(self.gestures[gesture].chords(arguments)?)
            }
            "keydown" | "keyup" => {
                let pressed = command == "keydown";
                if arguments.is_empty() {
                    return Err(format!("{command} needs a KEY"));
                }
                let mut events = Vec::new();
                for name in arguments {
                    let key = Key::from_name(name)
                        .ok_or_else(|| format!("{name:?} is not a key beui knows"))?;
                    events.extend(self.gestures[gesture].key(key, pressed)?.into_iter().flatten());
                }
                input(vec![events])
            }
            "ime" => {
                let frames = match argument(0)? {
                    "compose" => self.gestures[gesture].compose(arguments[1..].join(" ")),
                    "commit" => self.gestures[gesture]
                        .commit((arguments.len() > 1).then(|| arguments[1..].join(" "))),
                    "cancel" => self.gestures[gesture].cancel_composing(),
                    other => {
                        return Err(format!("ime takes compose, commit or cancel, not {other}"));
                    }
                };
                input(frames)
            }
            "focus" | "blur" => input(self.gestures[gesture].focus(command == "focus")),
            "resize" => {
                let resize = match argument(0)? {
                    "screen" => Resize::Screen(
                        WindowSize::parse(argument(1)?)
                            .ok_or("resize screen takes WIDTHxHEIGHT")?
                            .size,
                    ),
                    size => Resize::Window(
                        WindowSize::parse(size)
                            .ok_or("resize takes WIDTHxHEIGHT or WIDTHxHEIGHT@SCALE")?,
                    ),
                };
                frame.below.resize(resize)?;
                input(vec![Vec::new()])
            }
            "clipboard" => {
                if !arguments.is_empty() {
                    frame.below.copy(arguments.join(" "));
                    return Ok(Started::Done(Reply::Text(String::new())));
                }
                let (sender, pasted) = std::sync::mpsc::channel();
                frame
                    .below
                    .paste(&[PasteKind::Text], PasteAnswer::new(sender, None));
                match clipboard_text(&pasted) {
                    Some(text) => Ok(Started::Done(Reply::Text(text))),
                    None => Ok(Started::Phase(Phase::Reading {
                        pasted,
                        until: view.now + CLIPBOARD_WAIT,
                    })),
                }
            }
            "upload" => {
                let file = PathBuf::from(argument(0)?);
                let before = view.rendered();
                match arguments.get(1) {
                    Some(target) => {
                        let at = at(target)?;
                        Ok(Started::Phase(Phase::Input {
                            steps: points(self.gestures[gesture].click(at, PointerButton::Primary, 1))
                                .into(),
                            before: before.clone(),
                            then: Some(Box::new(Phase::Uploading { file, before })),
                        }))
                    }
                    None if self.picks.is_empty() => Err(
                        "no file dialog is open; give upload a TARGET that opens one".to_owned(),
                    ),
                    None => Ok(Started::Phase(Phase::Uploading { file, before })),
                }
            }
            "save" => {
                let file = PathBuf::from(argument(0)?);
                let before = view.rendered();
                match arguments.get(1) {
                    Some(target) => {
                        let at = at(target)?;
                        Ok(Started::Phase(Phase::Input {
                            steps: points(self.gestures[gesture].click(at, PointerButton::Primary, 1))
                                .into(),
                            before: before.clone(),
                            then: Some(Box::new(Phase::Saving { file, before })),
                        }))
                    }
                    None if self.saves.is_empty() => {
                        Err("no save dialog is open; give save a TARGET that opens one".to_owned())
                    }
                    None => Ok(Started::Phase(Phase::Saving { file, before })),
                }
            }
            "dismiss" => {
                let pick = self.picks.first().map(|(opened, ..)| *opened);
                let save = self.saves.first().map(|(opened, ..)| *opened);
                match (pick, save) {
                    (None, None) => return Err("no file or save dialog is open".to_owned()),
                    (Some(pick), save) if save.is_none_or(|save| pick < save) => {
                        let (_, _, answer) = self.picks.remove(0);
                        answer.send(Ok(None));
                    }
                    _ => {
                        let (_, _, answer) = self.saves.remove(0);
                        answer.send(FileSave::Cancelled);
                    }
                }
                input(vec![Vec::new()])
            }
            "grab" => {
                if arguments.is_empty() {
                    return Err("grab needs at least one FILE".to_owned());
                }
                self.grabbed = arguments
                    .iter()
                    .map(|path| {
                        let path = PathBuf::from(path);
                        DroppedFile {
                            name: path
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            path: Some(path),
                            bytes: None,
                        }
                    })
                    .collect();
                incoming(vec![vec![
                    Incoming::points(Input::PointerMoved(self.gestures[gesture].pointer)),
                    FileDrag::Hovered.into(),
                ]])
            }
            "drop" => {
                if self.grabbed.is_empty() {
                    return Err("nothing is grabbed: grab FILE first".to_owned());
                }
                if let Some(target) = arguments.first() {
                    self.gestures[gesture].pointer = at(target)?;
                }
                let files = std::mem::take(&mut self.grabbed);
                incoming(vec![
                    vec![Incoming::points(Input::PointerMoved(self.gestures[gesture].pointer))],
                    files
                        .into_iter()
                        .map(|file| FileDrag::Dropped(file).into())
                        .collect(),
                ])
            }
            "ungrab" => {
                if std::mem::take(&mut self.grabbed).is_empty() {
                    return Err("nothing is grabbed".to_owned());
                }
                incoming(vec![vec![FileDrag::Cancelled.into()]])
            }
            "a11y" => {
                let rendered = view.rendered();
                let line = node(argument(0)?, rendered.lines())?;
                let offered = || {
                    line.actions
                        .iter()
                        .map(|action| action_name(*action))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let Some(name) = arguments.get(1) else {
                    return Ok(Started::Done(Reply::Text(format!(
                        "{}\nactions: {}\n",
                        line.text,
                        offered()
                    ))));
                };
                let action = ACTIONS
                    .into_iter()
                    .find(|action| action_name(*action) == name)
                    .ok_or_else(|| {
                        let names: Vec<&str> = ACTIONS.into_iter().map(action_name).collect();
                        format!("{name} is not one of {}", names.join(", "))
                    })?;
                if !line.actions.contains(&action) {
                    return Err(format!(
                        "{:?} does not offer {name}; it offers {}",
                        line.text,
                        offered()
                    ));
                }
                let value = arguments[2..].join(" ");
                let data = match action {
                    accesskit::Action::SetValue => Some(match value.parse::<f64>() {
                        Ok(number) => accesskit::ActionData::NumericValue(number),
                        Err(_) => accesskit::ActionData::Value(value.into()),
                    }),
                    accesskit::Action::ReplaceSelectedText => {
                        Some(accesskit::ActionData::Value(value.into()))
                    }
                    _ => None,
                };
                frame
                    .context
                    .accessibility_action(accesskit::ActionRequest {
                        action,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: line.id,
                        data,
                    });
                input(vec![Vec::new()])
            }
            "wait" | "gone" => {
                let (text, over) = match arguments {
                    [text @ .., over, at] if !text.is_empty() && over == "over" => {
                        let at = place(reads, at, view)?;
                        (text, Some(at.center()))
                    }
                    text => (text, None),
                };
                Ok(Started::Phase(Phase::Waiting {
                    text: text.join(" "),
                    present: command == "wait",
                    over,
                }))
            }
            "shot" => {
                let target = arguments.first().cloned();
                let crop = match &target {
                    Some(target) => Some(place(reads, target, view)?),
                    None => None,
                };
                Ok(Started::Phase(Phase::Capturing {
                    target,
                    crop,
                    drawn: false,
                }))
            }
            "paint" => {
                let (target, window) = match arguments {
                    [target @ .., word, window] if word == "in" => (
                        target,
                        window
                            .parse()
                            .map_err(|_| format!("paint takes in WINDOW, a number `windows` lists, not {window}"))?,
                    ),
                    target => (target, WindowId::MAIN.raw()),
                };
                let target = match target {
                    [] => None,
                    [target] => Some(target.clone()),
                    _ => return Err("usage: paint [TARGET] [in WINDOW]".to_owned()),
                };
                let crop = match &target {
                    Some(target) => Some(in_points(place(reads, target, view)?, view.pixels_per_point)),
                    None => None,
                };
                Ok(Started::Phase(Phase::Painting {
                    asked: false,
                    ready: false,
                    target,
                    crop,
                    window,
                    overflows: false,
                }))
            }
            "overflows" => Ok(Started::Phase(Phase::Painting {
                asked: false,
                ready: false,
                target: None,
                crop: None,
                window: WindowId::MAIN.raw(),
                overflows: true,
            })),
            other => Err(format!("unknown command {other}\n{USAGE}")),
        }
    }

    fn state(&self, tap: Tap, view: &View<'_>) -> String {
        let gestures = &self.gestures[self.gesture_index(tap)];
        let screen = !self.split || tap == Tap::Screen;
        let window = match (self.split, tap, &self.screen_window) {
            (true, Tap::Screen, Some(shown)) => shown,
            _ => &self.app_window,
        };
        let size = match (self.split, tap) {
            (true, Tap::App) => view.app,
            _ => view.platform,
        };
        let yes = |value: bool| if value { "yes" } else { "no" };
        let mut text = String::new();
        let scale = view.pixels_per_point;
        text.push_str(&format!(
            "window {}x{} at scale {scale}, fullscreen {}, focused {}\n",
            size.x,
            size.y,
            yes(view.fullscreen),
            yes(self.focused),
        ));
        text.push_str(&format!("cursor {:?}\n", window.requests.cursor));
        let held: Vec<String> = gestures
            .buttons
            .iter()
            .map(|button| format!("{button:?}"))
            .chain(gestures.fingers.iter().map(|(finger, pos)| {
                format!(
                    "finger {finger} at {},{}",
                    (pos.x * scale).round(),
                    (pos.y * scale).round()
                )
            }))
            .chain(gestures.pen.map(|(pos, down)| {
                format!(
                    "the pen {} at {},{}",
                    if down { "down" } else { "in range" },
                    (pos.x * scale).round(),
                    (pos.y * scale).round()
                )
            }))
            .collect();
        text.push_str(&format!(
            "pointer {},{}{}\n",
            (gestures.pointer.x * scale).round(),
            (gestures.pointer.y * scale).round(),
            match held.is_empty() {
                true => String::new(),
                false => format!(", holding {}", held.join(", ")),
            }
        ));
        let modifiers = [
            ("shift", gestures.held.shift),
            ("ctrl", gestures.held.ctrl),
            ("alt", gestures.held.alt),
            ("super", gestures.held.logo),
        ]
        .into_iter()
        .filter(|(_, held)| *held)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
        let keys: Vec<String> = modifiers
            .iter()
            .map(|name| (*name).to_owned())
            .chain(gestures.keys.iter().map(|key| format!("{key:?}")))
            .collect();
        if !keys.is_empty() {
            text.push_str(&format!("keys down {}\n", keys.join(", ")));
        }
        match (&gestures.composing, window.requests.ime.as_ref()) {
            (Some(composing), _) => text.push_str(&format!("composing {composing:?}\n")),
            (None, Some(area)) => text.push_str(&format!(
                "the app asks for an input method, its caret {}, {}\n",
                describe(scaled(area.cursor, scale)),
                match area.keyboard {
                    Keyboard::Shown { asks } => format!("its keyboard shown (ask {asks})"),
                    Keyboard::Held => "its keyboard held back".to_owned(),
                }
            )),
            (None, None) => {}
        }
        if screen && !self.grabbed.is_empty() {
            let names: Vec<&str> = self.grabbed.iter().map(|file| file.name.as_str()).collect();
            text.push_str(&format!("dragging the files {}\n", names.join(", ")));
        }
        if self.clock_stopped {
            text.push_str("clock stopped: time passes only with pause\n");
        }
        if self.unhurried {
            text.push_str("hurry off: the app waits as it does for a person\n");
        }
        if let Some(progress) = gestures.back {
            text.push_str(&format!("back gesture at {progress}\n"));
        }
        text.push_str(&format!(
            "wants the keyboard {}, pointer locked {}\n",
            yes(window.requests.wants_keyboard),
            yes(window.requests.pointer_locked)
        ));
        for (_, pick, _) in self.picks.iter().filter(|_| screen) {
            let filter = &pick.filter;
            text.push_str(&format!(
                "file dialog open for {} ({})\n",
                match filter.name.is_empty() {
                    true => "any file",
                    false => &filter.name,
                },
                filter.extensions.join(", ")
            ));
        }
        for (_, save, _) in self.saves.iter().filter(|_| screen) {
            text.push_str(&format!(
                "save dialog open for {} ({}, {} bytes)\n",
                save.file.name,
                save.file.mime_type,
                save.file.data.len()
            ));
        }
        text
    }
}

fn button_named(name: &str) -> Option<PointerButton> {
    match name {
        "left" => Some(PointerButton::Primary),
        "right" => Some(PointerButton::Secondary),
        "middle" => Some(PointerButton::Middle),
        "back" => Some(PointerButton::Back),
        "forward" => Some(PointerButton::Forward),
        _ => {
            let code = name.strip_prefix("button:")?;
            let code = match code.strip_prefix("0x") {
                Some(hex) => u16::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            Some(PointerButton::Other(code))
        }
    }
}

fn action_name(action: accesskit::Action) -> &'static str {
    match action {
        accesskit::Action::Click => "click",
        accesskit::Action::Focus => "focus",
        accesskit::Action::Blur => "blur",
        accesskit::Action::Expand => "expand",
        accesskit::Action::Collapse => "collapse",
        accesskit::Action::Increment => "increment",
        accesskit::Action::Decrement => "decrement",
        accesskit::Action::SetValue => "set",
        accesskit::Action::ReplaceSelectedText => "replace",
        accesskit::Action::ScrollUp => "scroll-up",
        accesskit::Action::ScrollDown => "scroll-down",
        accesskit::Action::ScrollLeft => "scroll-left",
        accesskit::Action::ScrollRight => "scroll-right",
        accesskit::Action::ScrollIntoView => "scroll-into-view",
        accesskit::Action::ShowContextMenu => "context-menu",
        accesskit::Action::ShowTooltip => "tooltip",
        _ => "other",
    }
}

fn clipboard_text(pasted: &std::sync::mpsc::Receiver<Pasted>) -> Option<String> {
    match pasted.try_recv() {
        Ok(Pasted::Text(text)) => Some(format!("{text}\n")),
        Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(String::new()),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
    }
}

fn capped(changes: String, limit: Option<usize>) -> String {
    let Some(limit) = limit else {
        return changes;
    };
    let total = changes.lines().count();
    if total <= limit {
        return changes;
    }
    let mut text: String = changes
        .lines()
        .take(limit)
        .flat_map(|line| [line, "\n"])
        .collect();
    text.push_str(&format!(
        "... and {} more changed lines; --changes=all prints them, and `drive app tree` the whole tree\n",
        total - limit
    ));
    text
}

fn factor(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|factor| *factor > 0.0)
        .ok_or_else(|| format!("{text} is not a factor above 0"))
}

fn finger_target(spec: &str) -> (u64, Option<&str>) {
    if let Some((finger, target)) = spec.split_once('=')
        && let Ok(finger) = finger.parse()
    {
        return (finger, Some(target));
    }
    match spec.parse() {
        Ok(finger) => (finger, None),
        Err(_) => (0, Some(spec)),
    }
}

fn shown(view: &View<'_>, text: &str, over: Option<Pos2>) -> bool {
    let matches = |line: &Line| match text.strip_prefix('#') {
        Some(id) => line.test_ids.iter().any(|named| named == id),
        None => line.text.contains(text),
    };
    let reaches = |line: &Line| {
        over.is_none_or(|over| line.bounds.is_some_and(|bounds| bounds.contains(over)))
    };
    view.rendered()
        .lines()
        .iter()
        .any(|line| matches(line) && reaches(line))
}

fn finger(argument: Option<&String>) -> Result<u64, String> {
    argument.map_or(Ok(0), |finger| {
        finger
            .parse()
            .map_err(|_| format!("{finger} is not a finger number"))
    })
}

fn actions(groups: &[ActionGroup], all: bool) -> String {
    let mut text = String::new();
    for group in groups.iter().filter(|group| group.window.is_none()) {
        let listed: Vec<_> = group
            .actions
            .iter()
            .filter(|action| all || action.live)
            .collect();
        if listed.is_empty() {
            continue;
        }
        let indent = match group.name.is_empty() {
            true => "",
            false => {
                text.push_str(&format!("Pane {:?}:\n", group.name));
                "  "
            }
        };
        for action in listed {
            text.push_str(&format!("{indent}{} {:?}", action.id, action.label));
            if let Some(shortcut) = &action.shortcut {
                text.push_str(&format!(" {shortcut}"));
            }
            if let Some(checked) = action.checked {
                text.push_str(&format!(" checked={checked}"));
            }
            if !action.enabled {
                text.push_str(" disabled");
            }
            if !action.live {
                text.push_str(" (not live where the focus is)");
            }
            text.push('\n');
        }
    }
    text
}

fn in_points(rect: Rect, scale: f32) -> [f32; 4] {
    [
        rect.min.x / scale,
        rect.min.y / scale,
        rect.max.x / scale,
        rect.max.y / scale,
    ]
}

fn cropped(capture: Capture, crop: Option<Rect>) -> Reply {
    let Some(crop) = crop else {
        return Reply::Image {
            width: capture.width,
            height: capture.height,
            rgba: capture.rgba,
        };
    };
    let x0 = (crop.min.x.floor().max(0.0) as u32).min(capture.width);
    let y0 = (crop.min.y.floor().max(0.0) as u32).min(capture.height);
    let x1 = (crop.max.x.ceil().max(0.0) as u32).clamp(x0, capture.width);
    let y1 = (crop.max.y.ceil().max(0.0) as u32).clamp(y0, capture.height);
    if x1 == x0 || y1 == y0 {
        return Reply::Error("the target has no area on screen".to_owned());
    }
    let stride = capture.width as usize * 4;
    let mut rgba = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
    for row in y0..y1 {
        let start = row as usize * stride + x0 as usize * 4;
        rgba.extend_from_slice(&capture.rgba[start..start + (x1 - x0) as usize * 4]);
    }
    Reply::Image {
        width: x1 - x0,
        height: y1 - y0,
        rgba,
    }
}

fn points(frames: Frames) -> Vec<Vec<Incoming>> {
    frames
        .into_iter()
        .map(|frame| frame.into_iter().map(Incoming::points).collect())
        .collect()
}

fn place(reads: bool, target: &str, view: &View<'_>) -> Result<Rect, String> {
    match reads {
        true => locate(target, view.rendered().lines()),
        false => coordinates(target).ok_or_else(|| {
            format!(
                "the screen tap takes X,Y or X,Y,WIDTH,HEIGHT in the platform window's pixels, not {target}; \
                 the app tap finds names and test ids: drive app ..."
            )
        }),
    }
}

fn aim(reads: bool, target: &str, view: &View<'_>) -> Result<Pos2, String> {
    match reads {
        true => point(target, view.rendered().lines(), view.pixels_per_point),
        false => {
            let center = place(reads, target, view)?.center();
            Ok(Pos2::new(
                center.x / view.pixels_per_point,
                center.y / view.pixels_per_point,
            ))
        }
    }
}
