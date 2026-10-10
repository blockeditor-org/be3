use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::target::{describe, locate, point, scaled};
use super::window::{Simulation, WindowSize};
use super::{Capture, Inbox, Reply, Request, changes, keys};
use crate::app::accessibility_dump::Line;
use crate::context::{ActionGroup, Context};
use crate::file_picker::PickedFile;
use crate::geometry::{Pos2, Rect, vec2};
use crate::input::{
    CursorIcon, DroppedFile, Event, Key, Modifiers, PointerButton, TouchId, TouchPhase,
};

pub const USAGE: &str = "Reading:
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  state                     the window, cursor, pointer, held keys, clipboard and file dialog
  actions [all]             the actions the command palette would list (all: every one)
  shot [TARGET]             a screenshot, of TARGET only when given
Pointer:
  click TARGET [right|middle|double]
  hover TARGET              move the pointer onto TARGET
  drag FROM TO              drag with the left button
  down TARGET [right|middle] / move TARGET / up [TARGET] [right|middle]
  scroll TARGET DY [DX]     scroll by DY pixels at TARGET, down when positive
  leave                     the pointer leaves the window
Touch:
  tap TARGET [FINGER]
  swipe FROM TO [FINGER]
  touch down|move|up TARGET [FINGER]
Keyboard:
  type TEXT                 type TEXT (a newline presses Enter)
  key CHORD...              press each chord in turn, such as ctrl+z or Enter
  hold KEY... / release KEY...   hold or let go of modifiers (shift, ctrl, alt, super)
  act ID                    run the action ID, as the command palette would
Window (headless only, except focus, blur and leave):
  resize WIDTHxHEIGHT[@SCALE]   resize the window, and its scale when given
  resize screen WIDTHxHEIGHT    the screen a fullscreen window fills
  focus / blur              the window gains or loses focus
  clipboard [TEXT]          read the clipboard, or put TEXT on it
  upload FILE [TARGET]      answer the open file dialog with FILE, clicking TARGET first to open it
  dismiss                   cancel the open file dialog
  drop TARGET FILE...       drag FILEs in from outside and drop them on TARGET
Waiting:
  settle                    wait until nothing is left to draw
  wait TEXT                 wait until a line of the tree contains TEXT
  gone TEXT                 wait until no line of the tree contains TEXT
  pause MILLISECONDS        let that much time pass, by the app's clock
A TARGET is #TEST_ID, X,Y in the tree's pixels, or text found in exactly one line of the tree.
Every command that gives input waits for the app to settle and answers with what changed in the tree.";

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
        until: Option<Instant>,
        length: Duration,
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
    fingers: Vec<u64>,
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
        }
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
            match self.start(&request.words, view, world) {
                Ok(Started::Done(reply)) => (request.reply)(reply),
                Ok(Started::Phase(phase)) => self.active = Some(Active { request, phase }),
                Err(error) => (request.reply)(Reply::Error(error)),
            }
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
        match &self.active.as_ref()?.phase {
            Phase::Input { steps, .. } if !steps.is_empty() => Some(Duration::ZERO),
            Phase::Pausing {
                until: Some(until), ..
            } => Some(until.saturating_duration_since(now)),
            Phase::Pausing { until: None, .. } => Some(Duration::ZERO),
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
            Phase::Input { steps, before, then } if steps.is_empty() => {
                active.phase = match then.take() {
                    Some(then) => *then,
                    None => settling(before),
                };
            }
            Phase::Acting { id, before } => match unran.iter().any(|unran| unran == id) {
                true => {
                    failed = Some(format!("no action is called {id}; `actions all` lists them"));
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
        let reply = match failed {
            Some(error) => Some(Reply::Error(error)),
            None => match &mut active.phase {
                Phase::Settling { before } if quiet => Some(Reply::Text(
                    before
                        .as_deref()
                        .map_or_else(String::new, |before| changes(before, view.tree)),
                )),
                Phase::Waiting { text, present }
                    if view
                        .lines
                        .iter()
                        .any(|line| line.text.contains(text.as_str()))
                        == *present =>
                {
                    Some(Reply::Text(String::new()))
                }
                Phase::Pausing { until, length } => match until {
                    None => {
                        *until = Some(view.now + *length);
                        None
                    }
                    Some(until) if view.now >= *until => Some(Reply::Text(String::new())),
                    Some(_) => None,
                },
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
                let rect = locate(argument(0)?, view.lines, view.test_ids, view.pixels_per_point)?;
                Ok(Started::Done(Reply::Text(format!("{}\n", describe(rect)))))
            }
            "state" => Ok(Started::Done(Reply::Text(self.state(view, world)))),
            "actions" => Ok(Started::Done(Reply::Text(actions(
                view.actions,
                arguments.first().map(String::as_str) == Some("all"),
            )))),
            "act" => {
                let id = argument(0)?.to_owned();
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
            "hover" | "move" => {
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
                self.pointer = to;
                let middle = from + (to - from) * 0.5;
                input(vec![
                    vec![
                        Event::PointerMoved(from),
                        self.button(from, PointerButton::Primary, true),
                    ],
                    vec![Event::PointerMoved(middle)],
                    vec![
                        Event::PointerMoved(to),
                        self.button(to, PointerButton::Primary, false),
                    ],
                ])
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
            "leave" => {
                self.buttons.clear();
                input(vec![vec![Event::PointerGone]])
            }
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
                let at = at(argument(1)?)?;
                let finger = finger(arguments.get(2))?;
                match phase {
                    TouchPhase::Start => self.fingers.push(finger),
                    TouchPhase::End => self.fingers.retain(|held| *held != finger),
                    _ => {}
                }
                input(vec![vec![touch(finger, phase, at)]])
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
            "hold" | "release" => {
                let pressed = command == "hold";
                let mut events = Vec::new();
                for name in arguments {
                    let key = keys::modifier(name)
                        .ok_or_else(|| format!("{name} is not shift, ctrl, alt or super"))?;
                    keys::set_modifier(&mut self.held, key, pressed);
                    keys::modifier_event(key, pressed, self.held, &mut events);
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
            "drop" => {
                let at = at(argument(0)?)?;
                let files: Vec<DroppedFile> = arguments[1..]
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
                if files.is_empty() {
                    return Err("drop needs a TARGET and at least one FILE".to_owned());
                }
                self.pointer = at;
                input(vec![
                    vec![Event::PointerMoved(at), Event::FileHovered],
                    files.into_iter().map(Event::FileDropped).collect(),
                ])
            }
            "settle" => Ok(Started::Phase(Phase::Settling { before: None })),
            "wait" | "gone" => Ok(Started::Phase(Phase::Waiting {
                text: arguments.join(" "),
                present: command == "wait",
            })),
            "pause" => {
                let milliseconds: u64 = argument(0)?
                    .parse()
                    .map_err(|_| "pause takes milliseconds".to_owned())?;
                Ok(Started::Phase(Phase::Pausing {
                    until: None,
                    length: Duration::from_millis(milliseconds),
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
            .chain(self.fingers.iter().map(|finger| format!("finger {finger}")))
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
        if !modifiers.is_empty() {
            text.push_str(&format!("holding {}\n", modifiers.join("+")));
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
