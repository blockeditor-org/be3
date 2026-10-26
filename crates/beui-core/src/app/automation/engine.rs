use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::gestures::{Frames, Gestures};
use super::target::{describe, locate, node, point, scaled};
use super::window::{Resize, WindowSize};
use super::{Capture, Inbox, Reply, Request, changes};
use crate::app::FrameTimes;
use crate::app::accessibility_dump::{ACTIONS, Line};
use crate::clipboard::{PasteAnswer, PasteKind, Pasted};
use crate::context::{ActionGroup, Context};
use crate::file_picker::{FilePickAnswer, FilePickRequest, PickedFile};
use crate::file_saver::{FileSave, FileSaveAnswer, FileSaveRequest};
use crate::geometry::{Rect, Vec2, vec2};
use crate::input::Keyboard;
use crate::input::{BackEdge, DroppedFile, FileDrag, Input, Key, PointerButton, TouchPhase};
use crate::runner::{Incoming, Platform};
use crate::window::WindowState;

const CHANGES: usize = 60;
const FRAME: Duration = Duration::from_micros(16_667);
const CLIPBOARD_WAIT: Duration = Duration::from_secs(5);

pub const USAGE: &str = "Reading:
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  comps [--internals]        component tree, optionally including component internals
  hit TARGET                what a press at TARGET would reach: the Interactives along its path,
                            the components that made them and what each handles
  state                     window, cursor, pointer, fingers, held keys, input method, clipboard, dialogs
  frames                    how many frames ran since the last frames, and the longest one's wall time:
                            the window's, and those the app ran elsewhere, such as a plugin's
  actions [all]             the actions the command palette would list (all: every one)
  env                       NAME=VALUE lines a program needs to show its windows inside the app,
                            such as WAYLAND_DISPLAY; empty when the app hosts no programs
  shot [TARGET]             a screenshot, of TARGET only when given
  paint [TARGET]            what the window paints, plugins included, as a paint snapshot,
                            of TARGET only when given
  overflows                 what the window shows out of bounds, plugins included, one per line:
                            cut off where nothing scrolls to it, drawn outside its own box, or
                            with its outline cut while its box shows
Pointer:
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
Touch:
  tap TARGET [FINGER]
  swipe FROM TO [FINGER]
  pinch TARGET FACTOR       two fingers spreading (FACTOR > 1) or closing around TARGET
  touch down|move|up|cancel [FINGER=]TARGET...   fingers 0, 1, ... step by step, all in one
                            frame; cancel is the system taking a finger back, as for a gesture
  touch up / touch cancel   lift or cancel every finger
Pen:
  pen move TARGET [PRESSURE]   the pen hovers over TARGET, or moves there pressed down with PRESSURE
  pen down TARGET [PRESSURE]   the tip touches TARGET, PRESSURE from 0 to 1 (0.5)
  pen up [TARGET]           the tip lifts; the pen stays in range
  pen draw FROM TO [STEPS [PER_FRAME]]   the tip down at FROM, along in STEPS (8) with the
                            pressure rising and falling, PER_FRAME (1) samples in each frame, and
                            up at TO
  pen leave                 the pen goes out of range
  back                      the system back gesture, whole
  back [left|right] / back PROGRESS / back commit|cancel   step by step, PROGRESS from 0 to 1
Keyboard:
  type TEXT                 type TEXT key by key, as a US keyboard would (a newline presses Enter);
                            a character no key makes arrives as text alone
  key CHORD...              press and let go of each chord, such as ctrl+z or Enter
  keydown KEY... / keyup KEY...   hold keys down and let go; keydown on a held key repeats it
  ime compose TEXT / ime commit [TEXT] / ime cancel   an input method's composition
  act ID [PANE]             run the action ID as the command palette would, in PANE's plugin when given
Screen reader:
  a11y TARGET               the node a screen reader would act on, and the actions it offers
  a11y TARGET ACTION [VALUE]   do what a screen reader does: click, focus, increment, set VALUE, ...
Files:
  grab FILE...              drag FILEs in from outside, over where the pointer is; move carries them
  drop [TARGET]             drop the grabbed files; ungrab takes them away again
  upload FILE [TARGET]      answer the file dialog with FILE, clicking TARGET first to open it
  save FILE [TARGET]        answer the save dialog by writing what it saves to FILE, clicking
                            TARGET first to open it
  dismiss                   cancel the oldest open file or save dialog
Controllers (a host that shows windows in 3D, such as space):
  xr HAND pose X Y Z [YAW PITCH]   HAND (left or right) at X, Y, Z metres, its laser turned YAW
                            degrees left and PITCH up from straight ahead (-Z)
  xr HAND point TARGET      aim HAND's laser, from where the hand is, at TARGET in the main window
  xr HAND trigger VALUE / xr HAND squeeze VALUE   pull the trigger or the grip, from 0 to 1
  xr HAND click TARGET      point at TARGET, then pull and let go of the trigger
  xr HAND lost              the controller is no longer tracked
Window (headless only, except focus, blur, leave and close):
  resize WIDTHxHEIGHT[@SCALE]   resize the window, and its scale when given
  resize screen WIDTHxHEIGHT    the screen a fullscreen window fills
  focus / blur              the window gains or loses focus; blur cancels the pointers and lets go of held keys
  clipboard [TEXT]          read the clipboard, or put TEXT on it
  close                     ask the window to close, as its close button does; an app that stays
                            open, to ask about unsaved changes, answers with what changed
Waiting:
  settle                    wait until nothing is left to draw
  wait TEXT|#TEST_ID        wait until a line of the tree contains TEXT, or the test id is on screen
  gone TEXT|#TEST_ID        wait until it is not
  pause MILLISECONDS        let that much time pass, by the app's clock
  clock stop / clock run    stop the app's clock, so time passes only with pause, frame by frame
  hurry off / hurry on      stop or start the app's shortcuts for a driven run, such as saving at once,
                            so it waits as it does for a person
A TARGET is #TEST_ID, #TEST_ID+X,Y (that far from its top left corner), X,Y or X,Y,WIDTH,HEIGHT
in the tree's pixels, or text found in one line of the tree.
Every command that gives input waits for the app to settle and answers with what changed in the tree,
at most 60 lines of it; before the command, --changes=N|all changes that, and --no-settle answers
after the frame that took the input instead of waiting.";

pub const PANE: char = '\u{1f}';

pub struct View<'a> {
    pub lines: &'a [Line],
    pub tree: &'a str,
    pub pixels_per_point: f32,
    pub now: Instant,
    pub size: Vec2,
    pub window: &'a WindowState,
    pub fullscreen: bool,
    pub actions: &'a [ActionGroup],
    pub environment: &'a dyn Fn() -> Vec<(String, String)>,
}

pub struct World<'a> {
    pub events: &'a mut Vec<Incoming>,
    pub platform: &'a mut dyn Platform,
    pub context: &'a Context,
}

#[derive(Clone, Copy)]
pub struct Settled {
    pub busy: bool,
    pub again: bool,
    pub drawing: bool,
}

enum Phase {
    Input {
        steps: VecDeque<Vec<Incoming>>,
        before: String,
        then: Option<Box<Phase>>,
    },
    Settling {
        before: Option<String>,
    },
    Waiting {
        text: String,
        present: bool,
    },
    Pausing {
        until: Instant,
    },
    Acting {
        id: String,
        before: String,
    },
    Hitting,
    Components,
    Uploading {
        file: PathBuf,
        before: String,
    },
    Saving {
        file: PathBuf,
        before: String,
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
    immediate: bool,
    changes: Option<usize>,
}

enum Started {
    Done(Reply),
    Phase(Phase),
}

pub struct Automation {
    inbox: Inbox,
    active: Option<Active>,
    gestures: Gestures,
    grabbed: Vec<DroppedFile>,
    clock_stopped: bool,
    unhurried: bool,
    closing: bool,
    focused: bool,
    picks: Vec<(u64, FilePickRequest, FilePickAnswer)>,
    saves: Vec<(u64, FileSaveRequest, FileSaveAnswer)>,
    dialogs: u64,
    frames: FrameTimes,
    other_frames: BTreeMap<String, FrameTimes>,
}

impl Automation {
    pub fn new(inbox: Inbox) -> Self {
        Self {
            inbox,
            active: None,
            gestures: Gestures::default(),
            grabbed: Vec::new(),
            clock_stopped: false,
            unhurried: false,
            closing: false,
            focused: true,
            picks: Vec::new(),
            saves: Vec::new(),
            dialogs: 0,
            frames: FrameTimes::default(),
            other_frames: BTreeMap::new(),
        }
    }

    pub fn framed(&mut self, took: Duration, others: Vec<(String, FrameTimes)>) {
        self.frames.add(took);
        for (name, times) in others {
            self.other_frames.entry(name).or_default().merge(times);
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

    pub fn clock_stopped(&self) -> bool {
        self.clock_stopped
    }

    pub fn inbox(&self) -> &Inbox {
        &self.inbox
    }

    pub fn begin(&mut self, view: &View<'_>, world: &mut World<'_>) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request.cancelled.load(Ordering::SeqCst))
            && let Some(active) = self.active.take()
            && let Phase::Input { steps, .. } = active.phase
        {
            world.events.extend(steps.into_iter().flatten());
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
                None => self.start(&request.words[skipped..], view, world),
            };
            match started {
                Ok(Started::Done(reply)) => (request.reply)(reply),
                Ok(Started::Phase(phase)) => {
                    self.active = Some(Active {
                        request,
                        phase,
                        immediate,
                        changes,
                    });
                }
                Err(error) => (request.reply)(Reply::Error(error)),
            }
        }
        if let Some(Active {
            phase: Phase::Pausing { until },
            ..
        }) = &self.active
            && self.clock_stopped
        {
            let left = until.saturating_duration_since(view.now);
            world.context.advance_clock(left.min(FRAME));
        }
        if let Some(Active {
            phase: Phase::Input { steps, .. },
            ..
        }) = &mut self.active
            && let Some(step) = steps.pop_front()
        {
            for event in &step {
                if let Incoming::Input {
                    input: Input::Focus(focused),
                    ..
                } = event
                {
                    self.focused = *focused;
                }
            }
            world.events.extend(step);
        }
    }

    pub fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.closing)
    }

    pub fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        self.dialogs += 1;
        self.picks.push((self.dialogs, request, answer));
    }

    pub fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        self.dialogs += 1;
        self.saves.push((self.dialogs, request, answer));
    }

    pub fn closed(&mut self, accepted: bool) {
        if accepted {
            self.finish(Reply::Text("The window closed.\n".to_owned()));
        }
    }

    pub fn wants_frame(&self, now: Instant) -> Option<Duration> {
        let stopped = self.clock_stopped;
        match &self.active.as_ref()?.phase {
            Phase::Input { steps, .. } if !steps.is_empty() => Some(Duration::ZERO),
            Phase::Pausing { .. } if stopped => Some(Duration::ZERO),
            Phase::Pausing { until } => Some(until.saturating_duration_since(now)),
            Phase::Reading { .. } => Some(FRAME),
            _ => None,
        }
    }

    pub fn end(
        &mut self,
        view: &View<'_>,
        settled: Settled,
        unran: &[String],
        hits: Option<&[String]>,
        comps: Option<&[String]>,
    ) {
        let quiet = !settled.again && !settled.busy;
        let Some(active) = &mut self.active else {
            return;
        };
        let settling = |before: &mut String| Phase::Settling {
            before: Some(std::mem::take(before)),
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
                    match locate(target, view.lines) {
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
                    match locate(target, view.lines) {
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
            Phase::Acting { id, before } => match unran.iter().any(|unran| unran == id) {
                true => {
                    let named = id.replace(PANE, " in ");
                    failed = Some(format!(
                        "no action is called {named}; `actions all` lists them"
                    ));
                }
                false => active.phase = settling(before),
            },
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
                Phase::Settling { before } if quiet || (immediate && before.is_some()) => Some(
                    Reply::Text(before.as_deref().map_or_else(String::new, |before| {
                        capped(changes(before, view.tree), limit)
                    })),
                ),
                Phase::Waiting { text, present } if shown(view, text) == *present => {
                    Some(Reply::Text(String::new()))
                }
                Phase::Pausing { until } if view.now >= *until => Some(Reply::Text(String::new())),
                Phase::Components => comps.map(|trees| Reply::Text(trees.join("\n"))),
                Phase::Hitting => hits.map(|hits| {
                    Reply::Text(match hits.is_empty() {
                        true => "nothing in the window is there\n".to_owned(),
                        false => hits.join("\n"),
                    })
                }),
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

    pub fn wants_sight(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Capturing { .. },
                ..
            })
        )
    }

    pub fn wants_capture(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Capturing { drawn: true, .. },
                ..
            })
        )
    }

    pub fn take_painting_request(&mut self) -> bool {
        match &mut self.active {
            Some(Active {
                phase: Phase::Painting { asked, .. },
                ..
            }) if !*asked => {
                *asked = true;
                true
            }
            _ => false,
        }
    }

    pub fn wants_painting(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Painting {
                    ready: true,
                    overflows: false,
                    ..
                },
                ..
            })
        )
    }

    pub fn wants_overflows(&self) -> bool {
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

    pub fn overflowed(&mut self, found: Vec<String>) {
        if self.wants_overflows() {
            self.finish(Reply::Text(
                found.into_iter().map(|line| line + "\n").collect(),
            ));
        }
    }

    pub fn painted(&mut self, painting: Result<crate::painting::Painting, String>) {
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

    pub fn captured(&mut self, capture: Option<Capture>) {
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
        world: &mut World<'_>,
    ) -> Result<Started, String> {
        let (command, arguments) = words.split_first().ok_or_else(|| USAGE.to_owned())?;
        let argument = |index: usize| -> Result<&str, String> {
            arguments
                .get(index)
                .map(String::as_str)
                .ok_or_else(|| format!("{command} needs more arguments\n{USAGE}"))
        };
        let at = |target: &str| point(target, view.lines, view.pixels_per_point);
        let incoming = |steps: Vec<Vec<Incoming>>| {
            Ok(Started::Phase(Phase::Input {
                steps: steps.into(),
                before: view.tree.to_owned(),
                then: None,
            }))
        };
        let input = |frames: Frames| incoming(points(frames));
        match command.as_str() {
            "help" => Ok(Started::Done(Reply::Text(format!("{USAGE}\n")))),
            "tree" => Ok(Started::Done(Reply::Text(view.tree.to_owned()))),
            "frames" => Ok(Started::Done(Reply::Text(self.take_frames()))),
            "ids" => {
                let mut ids: Vec<(&str, &Line)> = view
                    .lines
                    .iter()
                    .flat_map(|line| line.test_ids.iter().map(move |id| (id.as_str(), line)))
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
                let rect = locate(argument(0)?, view.lines)?;
                Ok(Started::Done(Reply::Text(format!("{}\n", describe(rect)))))
            }
            "comps" => {
                if arguments.len() > 1 || arguments.first().is_some_and(|arg| arg != "--internals")
                {
                    return Err("usage: comps [--internals]".to_owned());
                }
                world.context.request_comps(!arguments.is_empty());
                Ok(Started::Phase(Phase::Components))
            }
            "hit" => {
                world.context.request_hit(at(argument(0)?)?);
                Ok(Started::Phase(Phase::Hitting))
            }
            "state" => Ok(Started::Done(Reply::Text(self.state(view)))),
            "env" => Ok(Started::Done(Reply::Text(
                (view.environment)()
                    .into_iter()
                    .map(|(name, value)| format!("{name}={value}\n"))
                    .collect(),
            ))),
            "actions" => Ok(Started::Done(Reply::Text(actions(
                view.actions,
                arguments.first().map(String::as_str) == Some("all"),
            )))),
            "act" => {
                let id = match arguments.get(1) {
                    Some(pane) => format!("{pane}{PANE}{}", argument(0)?),
                    None => argument(0)?.to_owned(),
                };
                world.context.request_action(id.clone());
                Ok(Started::Phase(Phase::Acting {
                    id,
                    before: view.tree.to_owned(),
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
                input(self.gestures.click(at, button, count))
            }
            "move" => input(self.gestures.hover(at(argument(0)?)?)),
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
                    (true, Some(at)) => input(self.gestures.down(at, button)),
                    (true, None) => Err("down needs a TARGET".to_owned()),
                    (false, at) => input(self.gestures.up(at, button)),
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
                input(self.gestures.drag(from, to, steps))
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
                input(self.gestures.wheel(at, ticks(1)?, ticks(2)?))
            }
            "zoom" => {
                let at = at(argument(0)?)?;
                input(self.gestures.zoom(at, factor(argument(1)?)?))
            }
            "pinch" => {
                let center = at(argument(0)?)?;
                input(self.gestures.pinch(center, factor(argument(1)?)?))
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
                input(self.gestures.scroll(at, vec2(number(2)?, number(1)?)))
            }
            "leave" => input(self.gestures.leave()),
            "motion" => {
                let pixels = |index: usize| -> Result<f32, String> {
                    let text = argument(index)?;
                    text.parse::<f32>()
                        .map(|value| value / view.pixels_per_point)
                        .map_err(|_| format!("{text} is not a number of pixels"))
                };
                input(self.gestures.motion(vec2(pixels(0)?, pixels(1)?)))
            }
            "tap" => {
                let at = at(argument(0)?)?;
                input(self.gestures.tap(at, finger(arguments.get(1))?))
            }
            "swipe" => {
                let from = at(argument(0)?)?;
                let to = at(argument(1)?)?;
                input(self.gestures.swipe(from, to, finger(arguments.get(2))?))
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
                let frames = match argument(0)? {
                    "move" => self.gestures.pen_move(at(argument(1)?)?, pressure(2)?),
                    "down" => self.gestures.pen_down(at(argument(1)?)?, pressure(2)?)?,
                    "up" => {
                        let to = arguments.get(1).map(|target| at(target)).transpose()?;
                        self.gestures.pen_up(to)?
                    }
                    "draw" => {
                        let from = at(argument(1)?)?;
                        let to = at(argument(2)?)?;
                        let count =
                            |index: usize, default: usize, what: &str| match arguments.get(index) {
                                Some(text) => text
                                    .parse::<usize>()
                                    .map_err(|_| format!("{text} is not a number of {what}")),
                                None => Ok(default),
                            };
                        let steps = count(3, 8, "steps")?;
                        let per_frame = count(4, 1, "samples a frame")?;
                        if per_frame == 0 {
                            return Err(
                                "pen draw puts at least one sample in each frame".to_owned()
                            );
                        }
                        self.gestures.pen_draw(from, to, steps, per_frame)?
                    }
                    "leave" => self.gestures.pen_leave()?,
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
                input(self.gestures.touch(phase, &fingers)?)
            }
            "back" => {
                let frames = match arguments.first().map(String::as_str) {
                    None => self.gestures.back(),
                    Some("left") => self.gestures.back_from(BackEdge::Left)?,
                    Some("right") => self.gestures.back_from(BackEdge::Right)?,
                    Some("commit") => self.gestures.back_end(true)?,
                    Some("cancel") => self.gestures.back_end(false)?,
                    Some(progress) => {
                        let progress: f32 = progress
                            .parse()
                            .ok()
                            .filter(|progress: &f32| (0.0..=1.0).contains(progress))
                            .ok_or(
                                "back takes left, right, a progress from 0 to 1, commit or cancel",
                            )?;
                        self.gestures.back_to(progress)
                    }
                };
                input(frames)
            }
            "type" => input(self.gestures.type_text(&arguments.join(" "))),
            "key" => {
                if arguments.is_empty() {
                    return Err(format!("key needs a chord, such as ctrl+z\n{USAGE}"));
                }
                input(self.gestures.chords(arguments)?)
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
                    events.extend(self.gestures.key(key, pressed)?.into_iter().flatten());
                }
                input(vec![events])
            }
            "ime" => {
                let frames = match argument(0)? {
                    "compose" => self.gestures.compose(arguments[1..].join(" ")),
                    "commit" => self
                        .gestures
                        .commit((arguments.len() > 1).then(|| arguments[1..].join(" "))),
                    "cancel" => self.gestures.cancel_composing(),
                    other => {
                        return Err(format!("ime takes compose, commit or cancel, not {other}"));
                    }
                };
                input(frames)
            }
            "focus" | "blur" => input(self.gestures.focus(command == "focus")),
            "xr" => incoming(controller(arguments, at)?),
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
                world.platform.resize(resize)?;
                input(vec![Vec::new()])
            }
            "clipboard" => {
                if !arguments.is_empty() {
                    world.platform.copy(arguments.join(" "));
                    return Ok(Started::Done(Reply::Text(String::new())));
                }
                let (sender, pasted) = std::sync::mpsc::channel();
                world
                    .platform
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
                let before = view.tree.to_owned();
                match arguments.get(1) {
                    Some(target) => {
                        let at = at(target)?;
                        Ok(Started::Phase(Phase::Input {
                            steps: points(self.gestures.click(at, PointerButton::Primary, 1))
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
                let before = view.tree.to_owned();
                match arguments.get(1) {
                    Some(target) => {
                        let at = at(target)?;
                        Ok(Started::Phase(Phase::Input {
                            steps: points(self.gestures.click(at, PointerButton::Primary, 1))
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
                    Incoming::points(Input::PointerMoved(self.gestures.pointer)),
                    FileDrag::Hovered.into(),
                ]])
            }
            "drop" => {
                if self.grabbed.is_empty() {
                    return Err("nothing is grabbed: grab FILE first".to_owned());
                }
                if let Some(target) = arguments.first() {
                    self.gestures.pointer = at(target)?;
                }
                let files = std::mem::take(&mut self.grabbed);
                incoming(vec![
                    vec![Incoming::points(Input::PointerMoved(self.gestures.pointer))],
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
            "settle" => Ok(Started::Phase(Phase::Settling { before: None })),
            "close" => {
                self.closing = true;
                Ok(Started::Phase(Phase::Settling {
                    before: Some(view.tree.to_owned()),
                }))
            }
            "a11y" => {
                let line = node(argument(0)?, view.lines)?;
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
                world
                    .context
                    .accessibility_action(accesskit::ActionRequest {
                        action,
                        target_tree: accesskit::TreeId::ROOT,
                        target_node: line.id,
                        data,
                    });
                input(vec![Vec::new()])
            }
            "clock" => {
                match argument(0)? {
                    "stop" => world.context.stop_clock(),
                    "run" => world.context.run_clock(),
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
                world.context.set_hurried(!self.unhurried);
                Ok(Started::Phase(Phase::Settling { before: None }))
            }
            "wait" | "gone" => Ok(Started::Phase(Phase::Waiting {
                text: arguments.join(" "),
                present: command == "wait",
            })),
            "pause" => {
                let milliseconds: u64 = argument(0)?
                    .parse()
                    .map_err(|_| "pause takes milliseconds".to_owned())?;
                Ok(Started::Phase(Phase::Pausing {
                    until: view.now + Duration::from_millis(milliseconds),
                }))
            }
            "shot" => {
                let target = arguments.first().cloned();
                let crop = match &target {
                    Some(target) => Some(locate(target, view.lines)?),
                    None => None,
                };
                Ok(Started::Phase(Phase::Capturing {
                    target,
                    crop,
                    drawn: false,
                }))
            }
            "paint" => {
                let target = arguments.first().cloned();
                let crop = match &target {
                    Some(target) => Some(in_points(
                        locate(target, view.lines)?,
                        view.pixels_per_point,
                    )),
                    None => None,
                };
                Ok(Started::Phase(Phase::Painting {
                    asked: false,
                    ready: false,
                    target,
                    crop,
                    overflows: false,
                }))
            }
            "overflows" => Ok(Started::Phase(Phase::Painting {
                asked: false,
                ready: false,
                target: None,
                crop: None,
                overflows: true,
            })),
            other => Err(format!("unknown command {other}\n{USAGE}")),
        }
    }

    fn state(&self, view: &View<'_>) -> String {
        let yes = |value: bool| if value { "yes" } else { "no" };
        let mut text = String::new();
        let scale = view.pixels_per_point;
        text.push_str(&format!(
            "window {}x{} at scale {scale}, fullscreen {}, focused {}\n",
            view.size.x,
            view.size.y,
            yes(view.fullscreen),
            yes(self.focused),
        ));
        text.push_str(&format!("cursor {:?}\n", view.window.requests.cursor));
        let held: Vec<String> = self
            .gestures
            .buttons
            .iter()
            .map(|button| format!("{button:?}"))
            .chain(self.gestures.fingers.iter().map(|(finger, pos)| {
                format!(
                    "finger {finger} at {},{}",
                    (pos.x * scale).round(),
                    (pos.y * scale).round()
                )
            }))
            .chain(self.gestures.pen.map(|(pos, down)| {
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
            (self.gestures.pointer.x * scale).round(),
            (self.gestures.pointer.y * scale).round(),
            match held.is_empty() {
                true => String::new(),
                false => format!(", holding {}", held.join(", ")),
            }
        ));
        let modifiers = [
            ("shift", self.gestures.held.shift),
            ("ctrl", self.gestures.held.ctrl),
            ("alt", self.gestures.held.alt),
            ("super", self.gestures.held.logo),
        ]
        .into_iter()
        .filter(|(_, held)| *held)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
        let keys: Vec<String> = modifiers
            .iter()
            .map(|name| (*name).to_owned())
            .chain(self.gestures.keys.iter().map(|key| format!("{key:?}")))
            .collect();
        if !keys.is_empty() {
            text.push_str(&format!("keys down {}\n", keys.join(", ")));
        }
        match (&self.gestures.composing, view.window.requests.ime.as_ref()) {
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
        if !self.grabbed.is_empty() {
            let names: Vec<&str> = self.grabbed.iter().map(|file| file.name.as_str()).collect();
            text.push_str(&format!("dragging the files {}\n", names.join(", ")));
        }
        if self.clock_stopped {
            text.push_str("clock stopped: time passes only with pause\n");
        }
        if self.unhurried {
            text.push_str("hurry off: the app waits as it does for a person\n");
        }
        if let Some(progress) = self.gestures.back {
            text.push_str(&format!("back gesture at {progress}\n"));
        }
        text.push_str(&format!(
            "wants the keyboard {}, pointer locked {}\n",
            yes(view.window.requests.wants_keyboard),
            yes(view.window.requests.pointer_locked)
        ));
        for (_, pick, _) in &self.picks {
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
        for (_, save, _) in &self.saves {
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
        "... and {} more changed lines; --changes=all prints them, and `drive tree` the whole tree\n",
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

fn shown(view: &View<'_>, text: &str) -> bool {
    match text.strip_prefix('#') {
        Some(id) => view
            .lines
            .iter()
            .any(|line| line.test_ids.iter().any(|named| named == id)),
        None => view.lines.iter().any(|line| line.text.contains(text)),
    }
}

fn controller(
    arguments: &[String],
    at: impl Fn(&str) -> Result<crate::geometry::Pos2, String>,
) -> Result<Vec<Vec<Incoming>>, String> {
    use crate::runner::{Controller, Hand, HandPose};
    let usage = "xr takes HAND pose X Y Z [YAW PITCH], HAND point TARGET, HAND trigger VALUE, \
                 HAND squeeze VALUE, HAND click TARGET or HAND lost, where HAND is left or right";
    let word = |index: usize| arguments.get(index).map(String::as_str).ok_or(usage);
    let number = |index: usize, default: Option<f32>| -> Result<f32, String> {
        match (arguments.get(index), default) {
            (Some(text), _) => text
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| format!("{text} is not a number")),
            (None, Some(default)) => Ok(default),
            (None, None) => Err(usage.to_owned()),
        }
    };
    let hand = Hand::named(word(0)?).ok_or(usage)?;
    let value = |index: usize| {
        number(index, None).and_then(|value| match (0.0..=1.0).contains(&value) {
            true => Ok(value),
            false => Err(format!("{value} is not from 0 to 1")),
        })
    };
    let once = |controller: Controller| vec![vec![Incoming::Controller(controller)]];
    Ok(match word(1)? {
        "pose" => {
            let position = [number(2, None)?, number(3, None)?, number(4, None)?];
            let (yaw, pitch) = (
                number(5, Some(0.0))?.to_radians() / 2.0,
                number(6, Some(0.0))?.to_radians() / 2.0,
            );
            let orientation = [
                yaw.cos() * pitch.sin(),
                yaw.sin() * pitch.cos(),
                -yaw.sin() * pitch.sin(),
                yaw.cos() * pitch.cos(),
            ];
            let pose = HandPose {
                position,
                orientation,
            };
            once(Controller::Pose {
                hand,
                aim: pose,
                grip: pose,
            })
        }
        "point" => once(Controller::Toward {
            hand,
            window: crate::window::WindowId::MAIN,
            at: at(word(2)?)?,
        }),
        "trigger" => once(Controller::Trigger {
            hand,
            value: value(2)?,
        }),
        "squeeze" => once(Controller::Squeeze {
            hand,
            value: value(2)?,
        }),
        "click" => {
            let toward = Controller::Toward {
                hand,
                window: crate::window::WindowId::MAIN,
                at: at(word(2)?)?,
            };
            [
                toward,
                Controller::Trigger { hand, value: 1.0 },
                Controller::Trigger { hand, value: 0.0 },
            ]
            .map(|controller| vec![Incoming::Controller(controller)])
            .into()
        }
        "lost" => once(Controller::Lost { hand }),
        _ => return Err(usage.to_owned()),
    })
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
    for group in groups {
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
