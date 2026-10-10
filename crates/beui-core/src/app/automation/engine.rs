use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::target::{describe, locate, node, point, scaled};
use super::window::{Simulation, WindowSize};
use super::{Capture, Inbox, Reply, Request, changes, keys};
use crate::app::accessibility_dump::{ACTIONS, Line};
use crate::context::{ActionGroup, Context};
use crate::file_picker::PickedFile;
use crate::geometry::{Pos2, Rect, vec2};
use crate::input::{
    BackEdge, BackGesture, CursorIcon, DroppedFile, Event, ImeEvent, Key, Modifiers, PointerButton,
    TouchId, TouchPhase,
};

const WHEEL_LINE: f32 = 40.0;
const CHANGES: usize = 60;
const FRAME: Duration = Duration::from_micros(16_667);
const GESTURE_STEPS: usize = 4;
const PINCH_SPAN: f32 = 100.0;

pub const USAGE: &str = "Reading:
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  state                     window, cursor, pointer, fingers, held keys, input method, clipboard, dialogs
  actions [all]             the actions the command palette would list (all: every one)
  shot [TARGET]             a screenshot, of TARGET only when given
Pointer:
  click TARGET [right|middle|double]
  move TARGET               move the pointer onto TARGET (hover)
  down TARGET [right|middle] / up [TARGET] [right|middle]   press and let go, for any gesture
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
  back                      the system back gesture, whole
  back [left|right] / back PROGRESS / back commit|cancel   step by step, PROGRESS from 0 to 1
Keyboard:
  type TEXT                 type TEXT (a newline presses Enter)
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
Window (headless only, except focus, blur and leave):
  resize WIDTHxHEIGHT[@SCALE]   resize the window, and its scale when given
  resize screen WIDTHxHEIGHT    the screen a fullscreen window fills
  focus / blur              the window gains or loses focus
  clipboard [TEXT]          read the clipboard, or put TEXT on it
Waiting:
  settle                    wait until nothing is left to draw
  wait TEXT|#TEST_ID        wait until a line of the tree contains TEXT, or the test id is on screen
  gone TEXT|#TEST_ID        wait until it is not
  pause MILLISECONDS        let that much time pass, by the app's clock
  clock stop / clock run    stop the app's clock, so time passes only with pause, frame by frame
A TARGET is #TEST_ID, X,Y or X,Y,WIDTH,HEIGHT in the tree's pixels, or text found in one line of the tree.
Every command that gives input waits for the app to settle and answers with what changed in the tree,
at most 60 lines of it; before the command, --changes=N|all changes that, and --no-settle answers
after the frame that took the input instead of waiting.";

pub const PANE: char = '\u{1f}';

pub struct View<'a> {
    pub lines: &'a [Line],
    pub tree: &'a str,
    pub test_ids: &'a HashMap<String, Rect>,
    pub pixels_per_point: f32,
    pub now: Instant,
    pub cursor: CursorIcon,
    pub fullscreen: bool,
    pub wants_keyboard: bool,
    pub pointer_locked: bool,
    pub actions: &'a [ActionGroup],
    pub ime: Option<Rect>,
}

pub struct World<'a> {
    pub events: &'a mut Vec<Event>,
    pub simulation: Option<&'a mut Simulation>,
    pub context: &'a Context,
}

#[derive(Clone, Copy)]
pub struct Settled {
    pub busy: bool,
    pub again: bool,
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
    Uploading {
        file: PathBuf,
        before: String,
    },
    Capturing {
        crop: Option<Rect>,
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
    keys: Vec<Key>,
    grabbed: Vec<DroppedFile>,
    composing: Option<String>,
    ime: bool,
    back: Option<f32>,
    clock_stopped: bool,
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
            keys: Vec::new(),
            grabbed: Vec::new(),
            composing: None,
            ime: false,
            back: None,
            clock_stopped: false,
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
            if let Some(simulation) = world.simulation.as_deref_mut() {
                for event in &step {
                    if let Event::Focus(focused) = event {
                        simulation.focused = *focused;
                    }
                }
            }
            world.events.extend(step);
        }
    }

    pub fn wants_frame(&self, now: Instant) -> Option<Duration> {
        let stopped = self.clock_stopped;
        match &self.active.as_ref()?.phase {
            Phase::Input { steps, .. } if !steps.is_empty() => Some(Duration::ZERO),
            Phase::Pausing { .. } if stopped => Some(Duration::ZERO),
            Phase::Pausing { until } => Some(until.saturating_duration_since(now)),
            _ => None,
        }
    }

    pub fn end(
        &mut self,
        view: &View<'_>,
        settled: Settled,
        world: &mut World<'_>,
        unran: &[String],
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
            let picked = world.simulation.as_deref_mut().and_then(|simulation| {
                (!simulation.picks.is_empty()).then(|| simulation.picks.remove(0))
            });
            match picked {
                Some(pick) => match std::fs::read(&*file) {
                    Ok(data) => {
                        let name = file
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        world
                            .context
                            .file_picked(pick.id, Ok(Some(PickedFile { name, data })));
                        active.phase = settling(before);
                        return;
                    }
                    Err(error) => {
                        world.context.file_picked(pick.id, Ok(None));
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
                _ => None,
            },
        };
        if let Some(reply) = reply {
            self.finish(reply);
        }
    }

    pub fn wants_capture(&self) -> bool {
        matches!(
            self.active,
            Some(Active {
                phase: Phase::Capturing { .. },
                ..
            })
        )
    }

    pub fn captured(&mut self, capture: Option<Capture>) {
        let Some(Active {
            phase: Phase::Capturing { crop },
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
        let at = |target: &str| point(target, view.lines, view.test_ids, view.pixels_per_point);
        let input = |steps: Vec<Vec<Event>>| {
            Ok(Started::Phase(Phase::Input {
                steps: steps.into(),
                before: view.tree.to_owned(),
                then: None,
            }))
        };
        let simulated = |what: &str| {
            format!("{what} needs a simulated window: start the app headless (BEUI_HEADLESS)")
        };
        match command.as_str() {
            "help" => Ok(Started::Done(Reply::Text(format!("{USAGE}\n")))),
            "tree" => Ok(Started::Done(Reply::Text(view.tree.to_owned()))),
            "ids" => {
                let mut ids: Vec<_> = view.test_ids.iter().collect();
                ids.sort_by(|a, b| a.0.cmp(b.0));
                let text = ids
                    .into_iter()
                    .map(|(id, rect)| {
                        format!("#{id} {}\n", describe(scaled(*rect, view.pixels_per_point)))
                    })
                    .collect();
                Ok(Started::Done(Reply::Text(text)))
            }
            "find" => {
                let rect = locate(
                    argument(0)?,
                    view.lines,
                    view.test_ids,
                    view.pixels_per_point,
                )?;
                Ok(Started::Done(Reply::Text(format!("{}\n", describe(rect)))))
            }
            "state" => Ok(Started::Done(Reply::Text(self.state(view, world)))),
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
                let text = arguments.join(" ");
                let mut events = Vec::new();
                for (index, part) in text.split('\n').enumerate() {
                    if index > 0 {
                        keys::press(Key::Enter, self.held, &mut events);
                    }
                    if !part.is_empty() {
                        events.push(Event::Text(part.to_owned()));
                    }
                }
                input(vec![events])
            }
            "key" => {
                if arguments.is_empty() {
                    return Err(format!("key needs a chord, such as ctrl+z\n{USAGE}"));
                }
                let mut events = Vec::new();
                for chord in arguments {
                    keys::chord(chord, self.held, &mut events)?;
                    let pasting = chord.eq_ignore_ascii_case("ctrl+v")
                        || (self.held.ctrl && chord.eq_ignore_ascii_case("v"));
                    if pasting
                        && let Some(text) = world
                            .simulation
                            .as_deref()
                            .and_then(|simulation| simulation.clipboard.clone())
                    {
                        events.push(Event::Text(text));
                    }
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
                    other => {
                        return Err(format!("ime takes compose, commit or cancel, not {other}"));
                    }
                }
                input(vec![events])
            }
            "focus" | "blur" => input(vec![vec![Event::Focus(command == "focus")]]),
            "resize" => {
                let simulation = world
                    .simulation
                    .as_deref_mut()
                    .ok_or_else(|| simulated("resize"))?;
                match argument(0)? {
                    "screen" => {
                        let size = WindowSize::parse(argument(1)?)
                            .ok_or("resize screen takes WIDTHxHEIGHT")?;
                        simulation.screen = size.size;
                    }
                    size => {
                        simulation.window = WindowSize::parse(size)
                            .ok_or("resize takes WIDTHxHEIGHT or WIDTHxHEIGHT@SCALE")?;
                    }
                }
                input(vec![Vec::new()])
            }
            "clipboard" => {
                let simulation = world
                    .simulation
                    .as_deref_mut()
                    .ok_or_else(|| simulated("clipboard"))?;
                if arguments.is_empty() {
                    return Ok(Started::Done(Reply::Text(
                        simulation
                            .clipboard
                            .clone()
                            .map(|text| format!("{text}\n"))
                            .unwrap_or_default(),
                    )));
                }
                simulation.clipboard = Some(arguments.join(" "));
                Ok(Started::Done(Reply::Text(String::new())))
            }
            "upload" => {
                let simulation = world
                    .simulation
                    .as_deref_mut()
                    .ok_or_else(|| simulated("upload"))?;
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
                    None if simulation.picks.is_empty() => Err(
                        "no file dialog is open; give upload a TARGET that opens one".to_owned(),
                    ),
                    None => Ok(Started::Phase(Phase::Uploading { file, before })),
                }
            }
            "dismiss" => {
                let simulation = world
                    .simulation
                    .as_deref_mut()
                    .ok_or_else(|| simulated("dismiss"))?;
                if simulation.picks.is_empty() {
                    return Err("no file dialog is open".to_owned());
                }
                let pick = simulation.picks.remove(0);
                world.context.file_picked(pick.id, Ok(None));
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
            "a11y" => {
                let line = node(
                    argument(0)?,
                    view.lines,
                    view.test_ids,
                    view.pixels_per_point,
                )?;
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
                let crop = match arguments.first() {
                    Some(target) => Some(locate(
                        target,
                        view.lines,
                        view.test_ids,
                        view.pixels_per_point,
                    )?),
                    None => None,
                };
                Ok(Started::Phase(Phase::Capturing { crop }))
            }
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

    fn state(&self, view: &View<'_>, world: &World<'_>) -> String {
        let yes = |value: bool| if value { "yes" } else { "no" };
        let mut text = String::new();
        let scale = view.pixels_per_point;
        match world.simulation.as_deref() {
            Some(simulation) => {
                let shown = simulation.shown();
                text.push_str(&format!(
                    "window {}x{} at scale {} (headless; windowed {}x{}, screen {}x{}), fullscreen {}, focused {}\n",
                    shown.x,
                    shown.y,
                    simulation.window.scale,
                    simulation.window.size.x,
                    simulation.window.size.y,
                    simulation.screen.x,
                    simulation.screen.y,
                    yes(simulation.fullscreen),
                    yes(simulation.focused),
                ));
            }
            None => text.push_str(&format!(
                "window at scale {scale}, fullscreen {}\n",
                yes(view.fullscreen)
            )),
        }
        text.push_str(&format!("cursor {:?}\n", view.cursor));
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
        match (&self.composing, view.ime) {
            (Some(composing), _) => text.push_str(&format!("composing {composing:?}\n")),
            (None, Some(cursor)) => text.push_str(&format!(
                "the app asks for an input method, its caret {}\n",
                describe(scaled(cursor, scale))
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
        if let Some(progress) = self.back {
            text.push_str(&format!("back gesture at {progress}\n"));
        }
        text.push_str(&format!(
            "wants the keyboard {}, pointer locked {}\n",
            yes(view.wants_keyboard),
            yes(view.pointer_locked)
        ));
        if let Some(simulation) = world.simulation.as_deref() {
            match &simulation.clipboard {
                Some(clipboard) => text.push_str(&format!("clipboard {clipboard:?}\n")),
                None => text.push_str("clipboard empty\n"),
            }
            for pick in &simulation.picks {
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
        }
        text
    }
}

fn button_named(name: &str) -> Option<PointerButton> {
    match name {
        "left" => Some(PointerButton::Primary),
        "right" => Some(PointerButton::Secondary),
        "middle" => Some(PointerButton::Middle),
        _ => None,
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
        Some(id) => view.test_ids.contains_key(id),
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
