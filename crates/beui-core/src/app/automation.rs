use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker as TaskWaker};

use crate::app::Waker;
use crate::app::accessibility_dump::Line;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::input::{Event, Key, Modifiers, PointerButton};

pub mod keys;
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub mod socket;

pub enum Reply {
    Text(String),
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Error(String),
}

pub struct Request {
    pub words: Vec<String>,
    pub reply: Box<dyn FnOnce(Reply) + Send>,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Answer {
    reply: Option<Reply>,
    answered: bool,
    waker: Option<TaskWaker>,
}

pub struct Answered {
    answer: Arc<Mutex<Answer>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct Canceller {
    answer: Arc<Mutex<Answer>>,
    cancelled: Arc<AtomicBool>,
}

impl Answered {
    pub fn canceller(&self) -> Canceller {
        Canceller {
            answer: self.answer.clone(),
            cancelled: self.cancelled.clone(),
        }
    }
}

impl Canceller {
    pub fn cancel(&self, why: String) {
        self.cancelled.store(true, Ordering::SeqCst);
        answer(&self.answer, Reply::Error(why));
    }
}

fn answer(answer: &Mutex<Answer>, reply: Reply) {
    let waker = {
        let mut answer = answer.lock().unwrap_or_else(|poison| poison.into_inner());
        if answer.reply.is_some() || answer.answered {
            return;
        }
        answer.answered = true;
        answer.reply = Some(reply);
        answer.waker.take()
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

impl Future for Answered {
    type Output = Reply;

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Reply> {
        let mut answer = self
            .answer
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match answer.reply.take() {
            Some(reply) => Poll::Ready(reply),
            None => {
                answer.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub fn ask(inbox: &Inbox, words: Vec<String>) -> Answered {
    let answer = Arc::new(Mutex::new(Answer::default()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let answering = answer.clone();
    inbox.send(Request {
        words,
        reply: Box::new(move |reply| self::answer(&answering, reply)),
        cancelled: cancelled.clone(),
    });
    Answered { answer, cancelled }
}

#[derive(Clone, Default)]
pub struct Inbox {
    inner: Arc<Mutex<InboxState>>,
}

#[derive(Default)]
struct InboxState {
    requests: VecDeque<Request>,
    waker: Option<Waker>,
}

impl Inbox {
    pub fn send(&self, request: Request) {
        let waker = {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.requests.push_back(request);
            state.waker.clone()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        let pending = {
            let mut state = self
                .inner
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            state.waker = Some(waker.clone());
            !state.requests.is_empty()
        };
        if pending {
            waker.wake();
        }
    }

    fn take(&self) -> Option<Request> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .requests
            .pop_front()
    }
}

pub struct View<'a> {
    pub lines: &'a [Line],
    pub tree: &'a str,
    pub test_ids: &'a HashMap<String, Rect>,
    pub pixels_per_point: f32,
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
    },
    Settling {
        before: Option<String>,
    },
    Waiting {
        text: String,
        present: bool,
    },
    Capturing {
        crop: Option<Rect>,
    },
}

struct Active {
    request: Request,
    phase: Phase,
}

pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct Automation {
    inbox: Inbox,
    active: Option<Active>,
    held: Modifiers,
    pointer: Pos2,
}

pub const USAGE: &str = "commands:
  tree                      the accessibility tree, one node per line
  ids                       every test id on screen, with where it is
  find TARGET               where TARGET is
  click TARGET [right|middle|double]
  hover TARGET              move the pointer onto TARGET
  drag FROM TO              drag with the left button
  scroll TARGET DY [DX]     scroll by DY pixels at TARGET, down when positive
  type TEXT                 type TEXT (a newline presses Enter)
  key CHORD...              press each chord in turn, such as ctrl+z or Enter
  hold KEY... / release KEY...   hold or let go of modifiers (shift, ctrl, alt, super)
  settle                    wait until nothing is left to draw
  wait TEXT                 wait until a line of the tree contains TEXT
  gone TEXT                 wait until no line of the tree contains TEXT
  shot [TARGET]             a screenshot, of TARGET only when given
A TARGET is #TEST_ID, X,Y in the tree's pixels, or text found in exactly one line of the tree.
Every command that gives input waits for the app to settle and answers with what changed in the tree.";

impl Automation {
    pub fn new(inbox: Inbox) -> Self {
        Self {
            inbox,
            active: None,
            held: Modifiers::NONE,
            pointer: Pos2::ZERO,
        }
    }

    pub fn inbox(&self) -> &Inbox {
        &self.inbox
    }

    pub fn begin(&mut self, view: &View<'_>, events: &mut Vec<Event>) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request.cancelled.load(Ordering::SeqCst))
        {
            self.active = None;
        }
        while self.active.is_none() {
            let Some(request) = self.inbox.take() else {
                break;
            };
            if request.cancelled.load(Ordering::SeqCst) {
                continue;
            }
            match self.start(&request.words, view) {
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
            events.extend(step);
        }
    }

    pub fn wants_frame(&self) -> bool {
        matches!(
            &self.active,
            Some(Active {
                phase: Phase::Input { steps, .. },
                ..
            }) if !steps.is_empty()
        )
    }

    pub fn end(&mut self, view: &View<'_>, settled: Settled) {
        let quiet = !settled.again && !settled.busy;
        let Some(active) = &mut self.active else {
            return;
        };
        if let Phase::Input { steps, before } = &mut active.phase
            && steps.is_empty()
        {
            active.phase = Phase::Settling {
                before: Some(std::mem::take(before)),
            };
        }
        let reply = match &active.phase {
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
            _ => None,
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

    fn start(&mut self, words: &[String], view: &View<'_>) -> Result<Started, String> {
        let (command, arguments) = words.split_first().ok_or_else(|| USAGE.to_owned())?;
        let argument = |index: usize| -> Result<&str, String> {
            arguments
                .get(index)
                .map(String::as_str)
                .ok_or_else(|| format!("{command} needs more arguments\n{USAGE}"))
        };
        let input = |steps: Vec<Vec<Event>>| {
            Ok(Started::Phase(Phase::Input {
                steps: steps.into(),
                before: view.tree.to_owned(),
            }))
        };
        match command.as_str() {
            "help" => Ok(Started::Done(Reply::Text(USAGE.to_owned()))),
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
                let rect = locate(argument(0)?, view)?;
                Ok(Started::Done(Reply::Text(format!("{}\n", describe(rect)))))
            }
            "click" => {
                let at = point(argument(0)?, view)?;
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
                let mut steps = Vec::new();
                for _ in 0..count {
                    steps.push(vec![
                        Event::PointerMoved(at),
                        self.button(at, button, true),
                        self.button(at, button, false),
                    ]);
                }
                input(steps)
            }
            "hover" => {
                let at = point(argument(0)?, view)?;
                self.pointer = at;
                input(vec![vec![Event::PointerMoved(at)]])
            }
            "drag" => {
                let from = point(argument(0)?, view)?;
                let to = point(argument(1)?, view)?;
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
                let at = point(argument(0)?, view)?;
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
            "settle" => Ok(Started::Phase(Phase::Settling { before: None })),
            "wait" | "gone" => Ok(Started::Phase(Phase::Waiting {
                text: arguments.join(" "),
                present: command == "wait",
            })),
            "shot" => {
                let crop = match arguments.first() {
                    Some(target) => Some(locate(target, view)?),
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
}

enum Started {
    Done(Reply),
    Phase(Phase),
}

fn scaled(rect: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * scale, rect.min.y * scale),
        pos2(rect.max.x * scale, rect.max.y * scale),
    )
}

fn describe(rect: Rect) -> String {
    format!(
        "at {},{} size {}x{}",
        rect.min.x.round(),
        rect.min.y.round(),
        rect.width().round(),
        rect.height().round()
    )
}

fn locate(target: &str, view: &View<'_>) -> Result<Rect, String> {
    if let Some(id) = target.strip_prefix('#') {
        return view
            .test_ids
            .get(id)
            .map(|rect| scaled(*rect, view.pixels_per_point))
            .ok_or_else(|| format!("no node on screen has the test id {id}; `ids` lists them"));
    }
    if let Some((x, y)) = target.split_once(',')
        && let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>())
    {
        return Ok(Rect::from_min_size(pos2(x, y), Vec2::ZERO));
    }
    let found: Vec<usize> = (0..view.lines.len())
        .filter(|index| view.lines[*index].text.contains(target))
        .collect();
    let line = match pick(view.lines, &found) {
        Some(index) => &view.lines[index],
        None if found.is_empty() => {
            return Err(format!(
                "no line of the tree contains {target:?}; `tree` prints it"
            ));
        }
        None => {
            let mut message = format!("{} lines of the tree contain {target:?}:\n", found.len());
            for index in &found {
                message.push_str("  ");
                message.push_str(&view.lines[*index].text);
                message.push('\n');
            }
            message.push_str("name one of them more closely, or use #TEST_ID or X,Y");
            return Err(message);
        }
    };
    let rect = line
        .bounds
        .ok_or_else(|| format!("{:?} has no place on screen", line.text))?;
    if !line.visible {
        return Err(format!("{:?} is scrolled out of view", line.text));
    }
    Ok(rect)
}

fn pick(lines: &[Line], found: &[usize]) -> Option<usize> {
    let within = |candidates: &[usize]| {
        let first = *candidates.first()?;
        let depth = lines[first].depth;
        let end = lines[first + 1..]
            .iter()
            .position(|line| line.depth <= depth)
            .map_or(lines.len(), |offset| first + 1 + offset);
        candidates.iter().all(|index| *index < end).then_some(first)
    };
    within(found).or_else(|| {
        let shown: Vec<usize> = found
            .iter()
            .copied()
            .filter(|index| lines[*index].visible)
            .collect();
        within(&shown)
    })
}

fn point(target: &str, view: &View<'_>) -> Result<Pos2, String> {
    let rect = locate(target, view)?;
    let center = rect.center();
    Ok(pos2(
        center.x / view.pixels_per_point,
        center.y / view.pixels_per_point,
    ))
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

const DIFF_LIMIT: usize = 4000;

pub fn changes(before: &str, after: &str) -> String {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old = &old[prefix..old.len() - suffix];
    let new = &new[prefix..new.len() - suffix];
    if old.is_empty() && new.is_empty() {
        return "no change to the tree\n".to_owned();
    }
    let mut text = String::new();
    if old.len().saturating_mul(new.len()) > DIFF_LIMIT * DIFF_LIMIT / 4 {
        for line in old {
            text.push_str("- ");
            text.push_str(line);
            text.push('\n');
        }
        for line in new {
            text.push_str("+ ");
            text.push_str(line);
            text.push('\n');
        }
        return text;
    }
    let width = new.len() + 1;
    let mut common = vec![0u32; (old.len() + 1) * width];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i * width + j] = if old[i] == new[j] {
                common[(i + 1) * width + j + 1] + 1
            } else {
                common[(i + 1) * width + j].max(common[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            i += 1;
            j += 1;
        } else if i < old.len()
            && (j == new.len() || common[(i + 1) * width + j] >= common[i * width + j + 1])
        {
            text.push_str("- ");
            text.push_str(old[i]);
            text.push('\n');
            i += 1;
        } else {
            text.push_str("+ ");
            text.push_str(new[j]);
            text.push('\n');
            j += 1;
        }
    }
    text
}
