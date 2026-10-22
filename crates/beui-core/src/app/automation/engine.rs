use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::target::{describe, locate, node, point, scaled};
use super::window::{Resize, WindowSize};
use super::{Capture, Inbox, Reply, Request, changes, keys};
use crate::app::accessibility_dump::{ACTIONS, Line};
use crate::clipboard::{PasteAnswer, PasteKind, Pasted};
use crate::context::{ActionGroup, Context};
use crate::file_picker::{FilePickAnswer, FilePickRequest, PickedFile};
use crate::geometry::{Pos2, Rect, Vec2, vec2};
use crate::input::Keyboard;
use crate::input::{
    BackEdge, BackGesture, DroppedFile, Event, ImeEvent, Key, Modifiers, Pen, PenPhase,
    PointerButton, TouchId, TouchPhase,
};
use crate::runner::Platform;
use crate::window::WindowState;

use crate::runner::SCROLL_LINE as WHEEL_LINE;
const CHANGES: usize = 60;
const FRAME: Duration = Duration::from_micros(16_667);
const CLIPBOARD_WAIT: Duration = Duration::from_secs(5);
const GESTURE_STEPS: usize = 4;
const PINCH_SPAN: f32 = 100.0;

pub const USAGE: &str = "Reading:
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  comps [--internals]        component tree, optionally including component internals
  hit TARGET                what a press at TARGET would reach: the Interactives along its path,
                            the components that made them and what each handles
  state                     window, cursor, pointer, fingers, held keys, input method, clipboard, dialogs
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
Touch:
  tap TARGET [FINGER]
  swipe FROM TO [FINGER]
  pinch TARGET FACTOR       two fingers spreading (FACTOR > 1) or closing around TARGET
  touch down|move|up [FINGER=]TARGET...   fingers 0, 1, ... step by step, all in one frame
  touch up                  lift every finger
Pen:
  pen move TARGET [PRESSURE]   the pen hovers over TARGET, or moves there pressed down with PRESSURE
  pen down TARGET [PRESSURE]   the tip touches TARGET, PRESSURE from 0 to 1 (0.5)
  pen up [TARGET]           the tip lifts; the pen stays in range
  pen draw FROM TO [STEPS]  the tip down at FROM, along in STEPS (8) with the pressure rising and
                            falling, and up at TO
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
  dismiss                   cancel the open file dialog
Window (headless only, except focus, blur, leave and close):
  resize WIDTHxHEIGHT[@SCALE]   resize the window, and its scale when given
  resize screen WIDTHxHEIGHT    the screen a fullscreen window fills
  focus / blur              the window gains or loses focus
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
A TARGET is #TEST_ID, X,Y or X,Y,WIDTH,HEIGHT in the tree's pixels, or text found in one line of the tree.
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
    pub events: &'a mut Vec<Event>,
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
        steps: VecDeque<Vec<Event>>,
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
    held: Modifiers,
    pointer: Pos2,
    buttons: Vec<PointerButton>,
    fingers: Vec<(u64, Pos2)>,
    pen: Option<(Pos2, bool)>,
    keys: Vec<Key>,
    grabbed: Vec<DroppedFile>,
    composing: Option<String>,
    ime: bool,
    back: Option<f32>,
    clock_stopped: bool,
    unhurried: bool,
    closing: bool,
    focused: bool,
    picks: Vec<(FilePickRequest, FilePickAnswer)>,
}

impl Automation {
    pub fn new(inbox: Inbox) -> Self {
        Self {
            inbox,
            active: None,
            held: Modifiers::NONE,
            pointer: Pos2::ZERO,
            buttons: Vec::new(),
            fingers: Vec::new(),
            pen: None,
            keys: Vec::new(),
            grabbed: Vec::new(),
            composing: None,
            ime: false,
            back: None,
            clock_stopped: false,
            unhurried: false,
            closing: false,
            focused: true,
            picks: Vec::new(),
        }
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
                if let Event::Focus(focused) = event {
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
        self.picks.push((request, answer));
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
                Some((_, answer)) => match std::fs::read(&*file) {
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
                "this renderer cannot take screenshots; run the app headless".to_owned(),
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
        let input = |steps: Vec<Vec<Event>>| {
            Ok(Started::Phase(Phase::Input {
                steps: steps.into(),
                before: view.tree.to_owned(),
                then: None,
            }))
        };
        match command.as_str() {
            "help" => Ok(Started::Done(Reply::Text(format!("{USAGE}\n")))),
            "tree" => Ok(Started::Done(Reply::Text(view.tree.to_owned()))),
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
                self.pointer = at;
                let steps = (0..count)
                    .map(|_| {
                        vec![
                            Event::PointerMoved(at),
                            self.button(at, button, true),
                            self.button(at, button, false),
                        ]
                    })
                    .collect();
                input(steps)
            }
            "move" => {
                let at = at(argument(0)?)?;
                self.pointer = at;
                input(vec![vec![Event::PointerMoved(at)]])
            }
            "down" | "up" => {
                let pressed = command == "down";
                let (target, button) = match (arguments.first(), arguments.get(1)) {
                    (Some(first), _) if button_named(first).is_some() => {
                        (None, button_named(first))
                    }
                    (target, button) => (target, button.and_then(|name| button_named(name))),
                };
                let button = button.unwrap_or(PointerButton::Primary);
                if pressed && target.is_none() {
                    return Err("down needs a TARGET".to_owned());
                }
                let at = match target {
                    Some(target) => at(target)?,
                    None => self.pointer,
                };
                self.pointer = at;
                if pressed {
                    self.buttons.push(button);
                } else {
                    self.buttons.retain(|held| *held != button);
                }
                input(vec![vec![
                    Event::PointerMoved(at),
                    self.button(at, button, pressed),
                ]])
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
                self.pointer = to;
                let mut frames = vec![vec![
                    Event::PointerMoved(from),
                    self.button(from, PointerButton::Primary, true),
                ]];
                for step in 1..steps.max(1) {
                    frames.push(vec![Event::PointerMoved(
                        from + (to - from) * (step as f32 / steps as f32),
                    )]);
                }
                frames.push(vec![
                    Event::PointerMoved(to),
                    self.button(to, PointerButton::Primary, false),
                ]);
                input(frames)
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
                let (down, right) = (ticks(1)?, ticks(2)?);
                self.pointer = at;
                let count = down.unsigned_abs().max(right.unsigned_abs());
                let frames = (0..count)
                    .map(|tick| {
                        let along = |ticks: i32| match tick < ticks.unsigned_abs() {
                            true => -(ticks.signum() as f32) * WHEEL_LINE,
                            false => 0.0,
                        };
                        vec![
                            Event::PointerMoved(at),
                            Event::Scroll(vec2(along(right), along(down))),
                        ]
                    })
                    .collect();
                input(frames)
            }
            "zoom" => {
                let at = at(argument(0)?)?;
                let factor = factor(argument(1)?)?;
                self.pointer = at;
                let step = factor.powf(1.0 / GESTURE_STEPS as f32);
                input(
                    (0..GESTURE_STEPS)
                        .map(|_| vec![Event::PointerMoved(at), Event::Zoom(step)])
                        .collect(),
                )
            }
            "pinch" => {
                let center = at(argument(0)?)?;
                let factor = factor(argument(1)?)?;
                let start = PINCH_SPAN / 2.0;
                let end = start * factor;
                let fingers = |half: f32, phase: TouchPhase| {
                    vec![
                        touch(0, phase, center - vec2(half, 0.0)),
                        touch(1, phase, center + vec2(half, 0.0)),
                    ]
                };
                let mut frames = vec![fingers(start, TouchPhase::Start)];
                for step in 1..=GESTURE_STEPS {
                    let half = start + (end - start) * (step as f32 / GESTURE_STEPS as f32);
                    frames.push(fingers(half, TouchPhase::Move));
                }
                frames.push(fingers(end, TouchPhase::End));
                input(frames)
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
                let delta = vec2(-number(2)?, -number(1)?);
                self.pointer = at;
                input(vec![vec![
                    Event::PointerMoved(at),
                    Event::Scroll(delta),
                    Event::ScrollEnded,
                ]])
            }
            "leave" => input(vec![vec![Event::PointerGone]]),
            "tap" => {
                let at = at(argument(0)?)?;
                let finger = finger(arguments.get(1))?;
                input(vec![vec![
                    touch(finger, TouchPhase::Start, at),
                    touch(finger, TouchPhase::End, at),
                ]])
            }
            "swipe" => {
                let from = at(argument(0)?)?;
                let to = at(argument(1)?)?;
                let finger = finger(arguments.get(2))?;
                let steps = (0..=4)
                    .map(|step| {
                        let at = from + (to - from) * (step as f32 / 4.0);
                        let phase = match step {
                            0 => TouchPhase::Start,
                            _ => TouchPhase::Move,
                        };
                        vec![touch(finger, phase, at)]
                    })
                    .chain(std::iter::once(vec![touch(finger, TouchPhase::End, to)]))
                    .collect();
                input(steps)
            }
            "pen" => {
                let pressure = |index: usize, default: f32| {
                    arguments.get(index).map_or(Ok(default), |text| {
                        text.parse::<f32>()
                            .ok()
                            .filter(|pressure| (0.0..=1.0).contains(pressure))
                            .ok_or_else(|| format!("{text} is not a pressure from 0 to 1"))
                    })
                };
                let held = self.pen;
                let mut frames = Vec::new();
                match argument(0)? {
                    "move" => {
                        let to = at(argument(1)?)?;
                        let down = held.is_some_and(|(_, down)| down);
                        let pressure = pressure(2, if down { 0.5 } else { 0.0 })?;
                        frames.push(vec![pen(PenPhase::Move, to, pressure)]);
                        self.pen = Some((to, down));
                    }
                    "down" => {
                        let to = at(argument(1)?)?;
                        if held.is_some_and(|(_, down)| down) {
                            return Err("the pen is already down".to_owned());
                        }
                        let pressure = pressure(2, 0.5)?;
                        frames.push(vec![
                            pen(PenPhase::Move, to, 0.0),
                            pen(PenPhase::Down(PointerButton::Primary), to, pressure),
                        ]);
                        self.pen = Some((to, true));
                    }
                    "up" => {
                        let Some((last, true)) = held else {
                            return Err("the pen is not down".to_owned());
                        };
                        let to = match arguments.get(1) {
                            Some(target) => at(target)?,
                            None => last,
                        };
                        frames.push(vec![
                            pen(PenPhase::Move, to, 0.5),
                            pen(PenPhase::Up(PointerButton::Primary), to, 0.0),
                        ]);
                        self.pen = Some((to, false));
                    }
                    "draw" => {
                        let from = at(argument(1)?)?;
                        let to = at(argument(2)?)?;
                        let steps: usize = match arguments.get(3) {
                            Some(steps) => steps
                                .parse()
                                .map_err(|_| format!("{steps} is not a number of steps"))?,
                            None => 8,
                        };
                        if held.is_some_and(|(_, down)| down) {
                            return Err("the pen is already down".to_owned());
                        }
                        let steps = steps.max(1);
                        let along = |step: usize| {
                            let fraction = step as f32 / steps as f32;
                            let pressure = 0.2 + 0.8 * (fraction * std::f32::consts::PI).sin();
                            (from + (to - from) * fraction, pressure)
                        };
                        frames.push(vec![
                            pen(PenPhase::Move, from, 0.0),
                            pen(PenPhase::Down(PointerButton::Primary), from, 0.2),
                        ]);
                        for step in 1..=steps {
                            let (at, pressure) = along(step);
                            frames.push(vec![pen(PenPhase::Move, at, pressure)]);
                        }
                        frames.push(vec![pen(PenPhase::Up(PointerButton::Primary), to, 0.0)]);
                        self.pen = Some((to, false));
                    }
                    "leave" => {
                        let Some((last, _)) = held else {
                            return Err("the pen is not in range".to_owned());
                        };
                        let mut frame = Vec::new();
                        if held.is_some_and(|(_, down)| down) {
                            frame.push(pen(PenPhase::Up(PointerButton::Primary), last, 0.0));
                        }
                        frame.push(pen(PenPhase::Gone, last, 0.0));
                        frames.push(frame);
                        self.pen = None;
                    }
                    other => {
                        return Err(format!(
                            "pen takes move, down, up, draw or leave, not {other}"
                        ));
                    }
                }
                input(frames)
            }
            "touch" => {
                let phase = match argument(0)? {
                    "down" => TouchPhase::Start,
                    "move" => TouchPhase::Move,
                    "up" => TouchPhase::End,
                    other => return Err(format!("touch takes down, move or up, not {other}")),
                };
                let mut events = Vec::new();
                if arguments.len() == 1 {
                    if phase != TouchPhase::End {
                        return Err("touch down and touch move need a TARGET".to_owned());
                    }
                    for (finger, pos) in std::mem::take(&mut self.fingers) {
                        events.push(touch(finger, phase, pos));
                    }
                }
                for spec in &arguments[1..] {
                    let (finger, target) = finger_target(spec);
                    let held = self.fingers.iter().position(|(held, _)| *held == finger);
                    let pos = match (target, held) {
                        (Some(target), _) => at(target)?,
                        (None, Some(index)) if phase == TouchPhase::End => self.fingers[index].1,
                        (None, _) => return Err(format!("{spec} names no TARGET")),
                    };
                    match (phase, held) {
                        (TouchPhase::Start, Some(_)) => {
                            return Err(format!("finger {finger} is already down"));
                        }
                        (TouchPhase::Start, None) => self.fingers.push((finger, pos)),
                        (_, None) => return Err(format!("finger {finger} is not down")),
                        (TouchPhase::End, Some(index)) => {
                            self.fingers.remove(index);
                        }
                        (_, Some(index)) => self.fingers[index].1 = pos,
                    }
                    events.push(touch(finger, phase, pos));
                }
                input(vec![events])
            }
            "back" => {
                let mut events = Vec::new();
                let start = |edge: BackEdge, events: &mut Vec<Event>, back: &mut Option<f32>| {
                    if back.is_none() {
                        *back = Some(0.0);
                        events.push(Event::Back(BackGesture::Started { edge }));
                    }
                };
                match arguments.first().map(String::as_str) {
                    None => {
                        start(BackEdge::Left, &mut events, &mut self.back);
                        let mut frames = vec![events];
                        for progress in [0.5, 1.0] {
                            frames.push(vec![Event::Back(BackGesture::Progressed(progress))]);
                        }
                        frames.push(vec![Event::Back(BackGesture::Invoked)]);
                        self.back = None;
                        return input(frames);
                    }
                    Some(edge @ ("left" | "right")) => {
                        if self.back.is_some() {
                            return Err("a back gesture is already under way".to_owned());
                        }
                        let edge = match edge {
                            "left" => BackEdge::Left,
                            _ => BackEdge::Right,
                        };
                        start(edge, &mut events, &mut self.back);
                    }
                    Some("commit") => {
                        if self.back.take().is_none() {
                            return Err("no back gesture is under way".to_owned());
                        }
                        events.push(Event::Back(BackGesture::Invoked));
                    }
                    Some("cancel") => {
                        if self.back.take().is_none() {
                            return Err("no back gesture is under way".to_owned());
                        }
                        events.push(Event::Back(BackGesture::Cancelled));
                    }
                    Some(progress) => {
                        let progress: f32 = progress
                            .parse()
                            .ok()
                            .filter(|progress: &f32| (0.0..=1.0).contains(progress))
                            .ok_or(
                                "back takes left, right, a progress from 0 to 1, commit or cancel",
                            )?;
                        start(BackEdge::Left, &mut events, &mut self.back);
                        self.back = Some(progress);
                        events.push(Event::Back(BackGesture::Progressed(progress)));
                    }
                }
                input(vec![events])
            }
            "type" => {
                let mut events = Vec::new();
                keys::typed(&arguments.join(" "), self.held, &mut events);
                input(vec![events])
            }
            "key" => {
                if arguments.is_empty() {
                    return Err(format!("key needs a chord, such as ctrl+z\n{USAGE}"));
                }
                let mut events = Vec::new();
                for chord in arguments {
                    keys::chord(chord, self.held, &mut events)?;
                }
                input(vec![events])
            }
            "keydown" | "keyup" => {
                let pressed = command == "keydown";
                if arguments.is_empty() {
                    return Err(format!("{command} needs a KEY"));
                }
                let mut events = Vec::new();
                for name in arguments {
                    let key = keys::named(name)
                        .ok_or_else(|| format!("{name:?} is not a key beui knows"))?;
                    if key.is_modifier() {
                        keys::set_modifier(&mut self.held, key, pressed);
                        keys::modifier_event(key, pressed, self.held, &mut events);
                        continue;
                    }
                    let held = self.keys.contains(&key);
                    match (pressed, held) {
                        (true, false) => self.keys.push(key),
                        (false, false) => return Err(format!("{name} is not held down")),
                        (false, true) => self.keys.retain(|down| *down != key),
                        (true, true) => {}
                    }
                    keys::key_event(key, pressed, pressed && held, self.held, &mut events);
                }
                input(vec![events])
            }
            "ime" => {
                let mut events = Vec::new();
                if !["compose", "commit", "cancel"].contains(&argument(0)?) {
                    return Err(format!(
                        "ime takes compose, commit or cancel, not {}",
                        arguments[0]
                    ));
                }
                if !self.ime {
                    self.ime = true;
                    events.push(Event::Ime(ImeEvent::Enabled));
                }
                match argument(0)? {
                    "compose" => {
                        let text = arguments[1..].join(" ");
                        self.composing = Some(text.clone());
                        events.push(Event::Ime(ImeEvent::SetComposingText(text)));
                    }
                    "commit" => {
                        let composed = self.composing.take().unwrap_or_default();
                        let text = match arguments.len() {
                            1 => composed,
                            _ => arguments[1..].join(" "),
                        };
                        events.push(Event::Ime(ImeEvent::SetComposingText(String::new())));
                        events.push(Event::Ime(ImeEvent::CommitText(text)));
                    }
                    "cancel" => {
                        self.composing = None;
                        events.push(Event::Ime(ImeEvent::SetComposingText(String::new())));
                    }
                    _ => unreachable!("the sub-command was checked above"),
                }
                input(vec![events])
            }
            "focus" | "blur" => input(vec![vec![Event::Focus(command == "focus")]]),
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
                        self.pointer = at;
                        Ok(Started::Phase(Phase::Input {
                            steps: vec![vec![
                                Event::PointerMoved(at),
                                self.button(at, PointerButton::Primary, true),
                                self.button(at, PointerButton::Primary, false),
                            ]]
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
            "dismiss" => {
                if self.picks.is_empty() {
                    return Err("no file dialog is open".to_owned());
                }
                let (_, answer) = self.picks.remove(0);
                answer.send(Ok(None));
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
                input(vec![vec![
                    Event::PointerMoved(self.pointer),
                    Event::FileHovered,
                ]])
            }
            "drop" => {
                if self.grabbed.is_empty() {
                    return Err("nothing is grabbed: grab FILE first".to_owned());
                }
                if let Some(target) = arguments.first() {
                    self.pointer = at(target)?;
                }
                let files = std::mem::take(&mut self.grabbed);
                input(vec![
                    vec![Event::PointerMoved(self.pointer)],
                    files.into_iter().map(Event::FileDropped).collect(),
                ])
            }
            "ungrab" => {
                if std::mem::take(&mut self.grabbed).is_empty() {
                    return Err("nothing is grabbed".to_owned());
                }
                input(vec![vec![Event::FileHoverCancelled]])
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

    fn button(&self, pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: self.held,
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
        text.push_str(&format!("cursor {:?}\n", view.window.cursor));
        let held: Vec<String> = self
            .buttons
            .iter()
            .map(|button| format!("{button:?}"))
            .chain(self.fingers.iter().map(|(finger, pos)| {
                format!(
                    "finger {finger} at {},{}",
                    (pos.x * scale).round(),
                    (pos.y * scale).round()
                )
            }))
            .chain(self.pen.map(|(pos, down)| {
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
            (self.pointer.x * scale).round(),
            (self.pointer.y * scale).round(),
            match held.is_empty() {
                true => String::new(),
                false => format!(", holding {}", held.join(", ")),
            }
        ));
        let modifiers = [
            ("shift", self.held.shift),
            ("ctrl", self.held.ctrl),
            ("alt", self.held.alt),
            ("super", self.held.logo),
        ]
        .into_iter()
        .filter(|(_, held)| *held)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
        let keys: Vec<String> = modifiers
            .iter()
            .map(|name| (*name).to_owned())
            .chain(self.keys.iter().map(|key| format!("{key:?}")))
            .collect();
        if !keys.is_empty() {
            text.push_str(&format!("keys down {}\n", keys.join(", ")));
        }
        match (&self.composing, view.window.ime.as_ref()) {
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
        if let Some(progress) = self.back {
            text.push_str(&format!("back gesture at {progress}\n"));
        }
        text.push_str(&format!(
            "wants the keyboard {}, pointer locked {}\n",
            yes(view.window.wants_keyboard),
            yes(view.window.pointer_locked)
        ));
        for (pick, _) in &self.picks {
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

fn finger(argument: Option<&String>) -> Result<u64, String> {
    argument.map_or(Ok(0), |finger| {
        finger
            .parse()
            .map_err(|_| format!("{finger} is not a finger number"))
    })
}

fn pen(phase: PenPhase, pos: Pos2, pressure: f32) -> Event {
    Event::Pen {
        phase,
        pos,
        pressure,
        pen: Pen::default(),
    }
}

fn touch(finger: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: TouchId { device: 0, finger },
        phase,
        pos,
        force: None,
    }
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
