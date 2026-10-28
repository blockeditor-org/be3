use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use beui_core::app::accessibility_dump::TestIds;
use beui_core::context::{FrameOutput, WindowOutput};
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{Key, Modifiers, PointerButton, TouchPhase, typing};
use beui_core::node::NodeId;
use beui_layer_drive::gestures::{Frames, Gestures};

use super::{FRAME_INTERVAL, Target, button_name, quoted};

const EAGER_FRAMES: usize = 8;
const SETTLE_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Driving {
    gestures: Gestures,
    steps: Vec<String>,
}

impl Driving {
    pub fn gestures(&mut self) -> &mut Gestures {
        &mut self.gestures
    }

    pub(crate) fn note(&mut self, words: &[&str]) {
        self.steps.push(
            std::iter::once("drive")
                .chain(words.iter().copied())
                .map(quoted)
                .collect::<Vec<_>>()
                .join(" "),
        );
    }

    pub(crate) fn replay(&self) -> String {
        let mut replay = String::from("the steps so far, as drive commands:\n");
        for step in &self.steps {
            replay.push_str("  ");
            replay.push_str(step);
            replay.push('\n');
        }
        replay
    }
}

#[derive(Default)]
pub struct Wakes {
    count: Mutex<u64>,
    woken: Condvar,
}

impl Wakes {
    pub fn count(&self) -> u64 {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn wake(&self) {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.woken.notify_all();
    }

    pub fn wait_past(&self, seen: u64, deadline: Duration) {
        let count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        drop(
            self.woken
                .wait_timeout_while(count, deadline, |count| *count == seen)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

pub trait Gui {
    type DocumentRef<'a>: Deref<Target = Document>
    where
        Self: 'a;

    type DocumentMut<'a>: DerefMut<Target = Document>
    where
        Self: 'a;

    fn document(&self) -> Self::DocumentRef<'_>;

    fn document_mut(&mut self) -> Self::DocumentMut<'_>;

    fn output(&self) -> &FrameOutput;

    fn window_output(&self) -> &WindowOutput;

    fn size(&self) -> Vec2;

    fn pixels_per_point(&self) -> f32;

    fn perform(&mut self, frames: Frames);

    fn run(&mut self);

    fn settled(&self) -> bool;

    fn advance(&mut self, by: Duration);

    fn wakes(&self) -> &Wakes;

    fn driving(&self) -> &Driving;

    fn driving_mut(&mut self) -> &mut Driving;

    fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size())
    }

    fn steps(&self) -> &[String] {
        &self.driving().steps
    }

    fn replay(&self) -> String {
        self.driving().replay()
    }

    fn test_ids(&self) -> Rc<TestIds> {
        self.output().test_ids()
    }

    fn shown(&self, test_id: &str) -> bool {
        self.output().test_id_rect(test_id).is_some()
    }

    fn rect_of(&self, test_id: &str) -> Rect {
        self.output()
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}\n{}", self.replay()))
    }

    fn rects_of(&self, test_id: &str) -> Vec<Rect> {
        self.test_ids()
            .iter()
            .find(|(named, _)| *named == test_id)
            .map(|(_, shown)| shown.iter().map(|at| at.rect).collect())
            .unwrap_or_default()
    }

    fn point_of(&self, test_id: &str) -> Pos2 {
        let rect = self.rect_of(test_id);
        let shown = rect.intersect(self.rect());
        match shown.is_positive() {
            true => shown.center(),
            false => rect.center(),
        }
    }

    fn label(&self, test_id: &str) -> String {
        let document = self.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        let mut collected = Vec::new();
        collect_text(&document, node, &mut collected);
        collected.join(" ")
    }

    fn accessible_label(&self, test_id: &str) -> Option<String> {
        let document = self.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        document
            .accessibility
            .get(&node)?
            .label()
            .map(str::to_owned)
    }

    fn reveal(&mut self, test_id: &str) {
        if self.rect().contains_rect(self.rect_of(test_id)) {
            return;
        }
        {
            let mut document = self.document_mut();
            let node = document
                .find_test_id(test_id)
                .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
            document.reveal_node(node);
        }
        self.run();
    }

    fn hover(&mut self, target: impl Into<Target>) {
        let at = locate(self, "move", &[target.into()], &[]);
        let frames = self.driving_mut().gestures.hover(at[0]);
        self.perform(frames);
    }

    fn leave(&mut self) {
        self.driving_mut().note(&["leave"]);
        let frames = self.driving_mut().gestures.leave();
        self.perform(frames);
    }

    fn motion(&mut self, by: Vec2) {
        let scale = self.pixels_per_point();
        let (right, down) = ((by.x * scale).to_string(), (by.y * scale).to_string());
        self.driving_mut().note(&["motion", &right, &down]);
        let frames = self.driving_mut().gestures.motion(by);
        self.perform(frames);
    }

    fn click(&mut self, target: impl Into<Target>) {
        let at = locate(self, "click", &[target.into()], &[]);
        let frames = self
            .driving_mut()
            .gestures
            .click(at[0], PointerButton::Primary, 1);
        self.perform(frames);
    }

    fn click_with(&mut self, target: impl Into<Target>, button: PointerButton) {
        let named = match button {
            PointerButton::Primary => Vec::new(),
            PointerButton::Secondary => vec!["right".to_owned()],
            PointerButton::Middle => vec!["middle".to_owned()],
            other => panic!("drive clicks the primary, right and middle buttons, not {other:?}"),
        };
        let at = locate(self, "click", &[target.into()], &named);
        let frames = self.driving_mut().gestures.click(at[0], button, 1);
        self.perform(frames);
    }

    fn double_click(&mut self, target: impl Into<Target>) {
        let at = locate(self, "click", &[target.into()], &["double".to_owned()]);
        let frames = self
            .driving_mut()
            .gestures
            .click(at[0], PointerButton::Primary, 2);
        self.perform(frames);
    }

    fn down(&mut self, target: impl Into<Target>, button: PointerButton) {
        let at = locate(self, "down", &[target.into()], &[button_name(button)]);
        let frames = self.driving_mut().gestures.down(at[0], button);
        self.perform(frames);
    }

    fn up(&mut self, target: impl Into<Target>, button: PointerButton) {
        let at = locate(self, "up", &[target.into()], &[button_name(button)]);
        let frames = self.driving_mut().gestures.up(Some(at[0]), button);
        self.perform(frames);
    }

    fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        let at = locate(self, "drag", &[from.into(), to.into()], &[]);
        let frames = self
            .driving_mut()
            .gestures
            .drag(at[0], at[1], 2, PointerButton::Primary);
        self.perform(frames);
    }

    fn drag_along(&mut self, points: &[Pos2]) {
        let (Some(&from), Some(&to)) = (points.first(), points.last()) else {
            return;
        };
        let scale = self.pixels_per_point();
        let word = |at: Pos2| Target::Point(at).word(scale);
        let driving = self.driving_mut();
        driving.note(&["down", &word(from), "left"]);
        for &at in &points[1..] {
            driving.note(&["move", &word(at)]);
        }
        driving.note(&["up", &word(to), "left"]);
        let gestures = &mut self.driving_mut().gestures;
        let mut frames = gestures.down(from, PointerButton::Primary);
        for &at in &points[1..] {
            frames.extend(gestures.hover(at));
        }
        frames.extend(gestures.up(Some(to), PointerButton::Primary));
        self.perform(frames);
    }

    fn wheel(&mut self, target: impl Into<Target>, down: i32, right: i32) {
        let at = locate(
            self,
            "wheel",
            &[target.into()],
            &[down.to_string(), right.to_string()],
        );
        let frames = self.driving_mut().gestures.wheel(at[0], down, right);
        self.perform(frames);
    }

    fn zoom(&mut self, target: impl Into<Target>, factor: f32) {
        let at = locate(self, "zoom", &[target.into()], &[factor.to_string()]);
        let frames = self.driving_mut().gestures.zoom(at[0], factor);
        self.perform(frames);
    }

    fn pinch(&mut self, target: impl Into<Target>, factor: f32) {
        let at = locate(self, "pinch", &[target.into()], &[factor.to_string()]);
        let frames = self.driving_mut().gestures.pinch(at[0], factor);
        self.perform(frames);
    }

    fn scroll(&mut self, target: impl Into<Target>, by: Vec2) {
        let scale = self.pixels_per_point();
        let rest = [(by.y * scale).to_string(), (by.x * scale).to_string()];
        let at = locate(self, "scroll", &[target.into()], &rest);
        let frames = self.driving_mut().gestures.scroll(at[0], by);
        self.perform(frames);
    }

    fn tap(&mut self, target: impl Into<Target>) {
        let at = locate(self, "tap", &[target.into()], &[]);
        let frames = self.driving_mut().gestures.tap(at[0], 1);
        self.perform(frames);
    }

    fn swipe(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        let at = locate(self, "swipe", &[from.into(), to.into()], &[]);
        let frames = self.driving_mut().gestures.swipe(at[0], at[1], 1);
        self.perform(frames);
    }

    fn fingers<T: Into<Target> + Clone>(&mut self, phase: TouchPhase, fingers: &[(u64, T)]) {
        let scale = self.pixels_per_point();
        let targets: Vec<Target> = fingers
            .iter()
            .map(|(_, target)| target.clone().into())
            .collect();
        let named: Vec<String> = fingers
            .iter()
            .zip(&targets)
            .map(|((finger, _), target)| format!("{finger}={}", target.word(scale)))
            .collect();
        let phase_name = match phase {
            TouchPhase::Start => "down",
            TouchPhase::Move => "move",
            TouchPhase::End => "up",
            TouchPhase::Cancel => "cancel",
        };
        let mut words = vec!["touch", phase_name];
        words.extend(named.iter().map(String::as_str));
        self.driving_mut().note(&words);
        let at: Vec<(u64, Option<Pos2>)> = fingers
            .iter()
            .zip(targets)
            .map(|((finger, _), target)| (*finger, Some(point(self, target))))
            .collect();
        let frames = self
            .driving_mut()
            .gestures
            .touch(phase, &at)
            .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()));
        self.perform(frames);
    }

    fn type_text(&mut self, text: &str) {
        self.driving_mut().note(&["type", text]);
        let frames = self.driving_mut().gestures.type_text(text);
        self.perform(frames);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let chord =
            typing::chord_name(key, modifiers).unwrap_or_else(|| format!("{modifiers:?}+{key:?}"));
        self.driving_mut().note(&["key", &chord]);
        let frames = self.driving_mut().gestures.chord(key, modifiers);
        self.perform(frames);
    }

    fn key_down(&mut self, key: Key) {
        held_key(self, key, true);
    }

    fn key_up(&mut self, key: Key) {
        held_key(self, key, false);
    }

    fn hold_modifiers(&mut self, modifiers: Modifiers) {
        let frames = self.driving_mut().gestures.hold(modifiers);
        self.perform(frames);
    }

    fn back(&mut self) {
        self.driving_mut().note(&["back"]);
        let frames = self.driving_mut().gestures.back();
        self.perform(frames);
    }

    fn settle(&mut self) {
        self.settle_until("its animations to finish", Self::settled);
    }

    fn settle_until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        let started = Instant::now();
        let mut eager = 0;
        while started.elapsed() < SETTLE_DEADLINE {
            let seen = self.wakes().count();
            self.run();
            if ready(self) {
                return;
            }
            let changed = self.window_output().changed;
            let requested = self.output().repaint_after;
            if changed && eager < EAGER_FRAMES {
                eager += 1;
                continue;
            }
            eager = 0;
            if requested < Duration::MAX {
                self.advance(requested.saturating_sub(FRAME_INTERVAL));
                continue;
            }
            let left = SETTLE_DEADLINE.saturating_sub(started.elapsed());
            self.wakes().wait_past(seen, left);
        }
        panic!(
            "still waiting for {what} after {SETTLE_DEADLINE:?}\n{}",
            self.replay()
        );
    }
}

fn locate<G: Gui + ?Sized>(
    gui: &mut G,
    command: &str,
    targets: &[Target],
    rest: &[String],
) -> Vec<Pos2> {
    let scale = gui.pixels_per_point();
    let words: Vec<String> = targets
        .iter()
        .map(|target| target.word(scale))
        .chain(rest.iter().cloned())
        .collect();
    let mut step = vec![command];
    step.extend(words.iter().map(String::as_str));
    gui.driving_mut().note(&step);
    targets
        .iter()
        .map(|target| point(gui, target.clone()))
        .collect()
}

fn point<G: Gui + ?Sized>(gui: &mut G, target: Target) -> Pos2 {
    match target {
        Target::Id(id, offset) => {
            gui.reveal(&id);
            match offset {
                Some(offset) => gui.rect_of(&id).min + offset,
                None => gui.point_of(&id),
            }
        }
        Target::Point(at) => at,
    }
}

fn held_key<G: Gui + ?Sized>(gui: &mut G, key: Key, pressed: bool) {
    let command = if pressed { "keydown" } else { "keyup" };
    let name = key.name().unwrap_or_else(|| format!("{key:?}"));
    gui.driving_mut().note(&[command, &name]);
    let frames = gui
        .driving_mut()
        .gestures
        .key(key, pressed)
        .unwrap_or_else(|error| panic!("{error}\n{}", gui.replay()));
    gui.perform(frames);
}

fn collect_text(document: &Document, node: NodeId, collected: &mut Vec<String>) {
    if document.node_kind(node) == "text" {
        let detail = document.node_detail(node).unwrap_or_default();
        let text = detail.trim_matches('"');
        collected.extend(
            document
                .text_icons(node)
                .into_iter()
                .map(|glyph| glyph.as_str().to_owned()),
        );
        if !text.is_empty() {
            collected.push(text.to_owned());
        }
    }
    for child in document.children(node) {
        collect_text(document, child, collected);
    }
}
