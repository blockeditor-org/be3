use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use beui_core::document::Document;
use beui_core::program::accessibility_dump::{Rendered, TestIds};
use beui_types::input::typing;
use beui_types::{Key, Modifiers, NodeId, PointerButton, TouchPhase};
use beui_types::{Pos2, Rect, Vec2};

use super::{FRAME_INTERVAL, Target, assert_fits, button_name};

const EAGER_FRAMES: usize = 8;
const SETTLE_DEADLINE: Duration = Duration::from_secs(30);

/// Counts the program's wakes, so `settle_until` can sleep until work on another thread
/// asks for a frame instead of polling.
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

/// What a test drives and reads a beui program through, whichever harness runs it: beui's
/// `Stepped` (and `be_sdk_test::DocumentTest` over it) or an editor's `be_sdk_test::BeuiTest`.
/// A harness supplies its document, the tree of its last frame, how a gesture reaches the
/// program, one frame (`run`), the clock and its wakes; gestures, reading by test id and
/// settling are written here once.
///
/// A gesture is one of `drive`'s commands, and paints the frames that take it before it
/// returns, so what it caused is in `shown`, `label` and the model at once. `advance` moves
/// the clock on without painting.
pub trait Gui {
    type DocumentRef<'a>: Deref<Target = Document>
    where
        Self: 'a;

    type DocumentMut<'a>: DerefMut<Target = Document>
    where
        Self: 'a;

    fn document(&self) -> Self::DocumentRef<'_>;

    fn document_mut(&mut self) -> Self::DocumentMut<'_>;

    /// The accessibility tree of the last frame painted, as `drive` reads it.
    fn tree(&self) -> Rc<Rendered>;

    /// The window's size in points.
    fn size(&self) -> Vec2;

    fn pixels_per_point(&self) -> f32;

    /// Gives the program the `drive` command `command` with its words, and paints the frames
    /// that take it. `targets` are the targets among the words, which a harness that plans
    /// the gesture itself locates.
    fn gesture(&mut self, command: &str, words: &[String], targets: &[Target]);

    /// Paints one frame, and whatever the host has to hand back after it.
    fn run(&mut self);

    /// Whether the last frame asked for no other at once.
    fn settled(&self) -> bool;

    /// Moves the clock on without painting.
    fn advance(&mut self, by: Duration);

    fn wakes(&self) -> &Wakes;

    /// The steps so far, as `drive` commands.
    fn steps(&self) -> &[String];

    fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size())
    }

    fn replay(&self) -> String {
        replay(self.steps())
    }

    fn test_ids(&self) -> Rc<TestIds> {
        Rc::clone(self.tree().test_ids())
    }

    /// Whether a node with this test id is on screen.
    fn shown(&self, test_id: &str) -> bool {
        self.test_ids().rect(test_id).is_some()
    }

    fn rect_of(&self, test_id: &str) -> Rect {
        self.test_ids()
            .rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}\n{}", self.replay()))
    }

    /// Every rect the test id names, for an id more than one node carries.
    fn rects_of(&self, test_id: &str) -> Vec<Rect> {
        self.test_ids()
            .iter()
            .find(|(named, _)| *named == test_id)
            .map(|(_, shown)| shown.iter().map(|at| at.rect).collect())
            .unwrap_or_default()
    }

    /// The middle of what the window shows of the node, where a click lands.
    fn point_of(&self, test_id: &str) -> Pos2 {
        let rect = self.rect_of(test_id);
        let shown = rect.intersect(self.rect());
        match shown.is_positive() {
            true => shown.center(),
            false => rect.center(),
        }
    }

    /// The text and icons inside the node, in tree order.
    fn label(&self, test_id: &str) -> String {
        let document = self.document();
        let node = found(&document, test_id);
        let mut collected = Vec::new();
        collect_text(&document, node, &mut collected);
        collected.join(" ")
    }

    fn accessible_label(&self, test_id: &str) -> Option<String> {
        let document = self.document();
        let node = found(&document, test_id);
        document
            .accessibility
            .get(&node)?
            .label()
            .map(str::to_owned)
    }

    fn accessible_description(&self, test_id: &str) -> Option<String> {
        let document = self.document();
        let node = found(&document, test_id);
        document
            .accessibility
            .get(&node)?
            .description()
            .map(str::to_owned)
    }

    /// Whether a node at `rect` needs no scrolling to be clicked.
    fn shows_whole(&self, rect: Rect) -> bool {
        self.rect().contains_rect(rect)
    }

    /// Scrolls the node into view, as a person would before clicking it, when a scroll hides
    /// or cuts it.
    fn reveal(&mut self, test_id: &str) {
        let cut = scroll_cuts(&self.document(), test_id);
        let shown = self.test_ids().rect(test_id);
        if !cut && shown.is_some_and(|rect| self.shows_whole(rect)) {
            return;
        }
        {
            let mut document = self.document_mut();
            let node = found(&document, test_id);
            document.reveal_node(node);
        }
        self.run();
    }

    /// Where a gesture at `target` lands, revealing a test id first.
    fn point(&mut self, target: Target) -> Pos2 {
        match target {
            Target::Id(id, offset) => {
                self.reveal(&id);
                match offset {
                    Some(offset) => self.rect_of(&id).min + offset,
                    None => self.point_of(&id),
                }
            }
            Target::Point(at) => at,
            Target::Text(text) => {
                let tree = self.tree();
                let shown = beui_types::target::shown_at(&text, tree.lines())
                    .unwrap_or_else(|error| panic!("{error}\n{}", self.replay()));
                let scale = self.pixels_per_point();
                Pos2::new(shown.center().x / scale, shown.center().y / scale)
            }
        }
    }

    fn hover(&mut self, target: impl Into<Target>) {
        act(self, "move", &[target.into()], &[]);
    }

    fn leave(&mut self) {
        act(self, "leave", &[], &[]);
    }

    fn motion(&mut self, by: Vec2) {
        let scale = self.pixels_per_point();
        let rest = [(by.x * scale).to_string(), (by.y * scale).to_string()];
        act(self, "motion", &[], &rest);
    }

    fn click(&mut self, target: impl Into<Target>) {
        act(self, "click", &[target.into()], &[]);
    }

    fn click_with(&mut self, target: impl Into<Target>, button: PointerButton) {
        let named = match button {
            PointerButton::Primary => Vec::new(),
            PointerButton::Secondary => vec!["right".to_owned()],
            PointerButton::Middle => vec!["middle".to_owned()],
            other => panic!("drive clicks the primary, right and middle buttons, not {other:?}"),
        };
        act(self, "click", &[target.into()], &named);
    }

    fn double_click(&mut self, target: impl Into<Target>) {
        act(self, "click", &[target.into()], &["double".to_owned()]);
    }

    fn down(&mut self, target: impl Into<Target>, button: PointerButton) {
        act(self, "down", &[target.into()], &[button_name(button)]);
    }

    fn up(&mut self, target: impl Into<Target>, button: PointerButton) {
        act(self, "up", &[target.into()], &[button_name(button)]);
    }

    fn drag(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        act(self, "drag", &[from.into(), to.into()], &[]);
    }

    /// A drag in `steps` moves of the pointer.
    fn drag_in(&mut self, from: impl Into<Target>, to: impl Into<Target>, steps: usize) {
        act(
            self,
            "drag",
            &[from.into(), to.into()],
            &[steps.to_string()],
        );
    }

    fn wheel(&mut self, target: impl Into<Target>, down: i32, right: i32) {
        let rest = [down.to_string(), right.to_string()];
        act(self, "wheel", &[target.into()], &rest);
    }

    fn zoom(&mut self, target: impl Into<Target>, factor: f32) {
        act(self, "zoom", &[target.into()], &[factor.to_string()]);
    }

    fn pinch(&mut self, target: impl Into<Target>, factor: f32) {
        act(self, "pinch", &[target.into()], &[factor.to_string()]);
    }

    fn scroll(&mut self, target: impl Into<Target>, by: Vec2) {
        let scale = self.pixels_per_point();
        let rest = [(by.y * scale).to_string(), (by.x * scale).to_string()];
        act(self, "scroll", &[target.into()], &rest);
    }

    fn tap(&mut self, target: impl Into<Target>) {
        act(self, "tap", &[target.into()], &[]);
    }

    fn swipe(&mut self, from: impl Into<Target>, to: impl Into<Target>) {
        act(self, "swipe", &[from.into(), to.into()], &[]);
    }

    fn fingers<T: Into<Target> + Clone>(&mut self, phase: TouchPhase, fingers: &[(u64, T)]) {
        let scale = self.pixels_per_point();
        let targets: Vec<Target> = fingers
            .iter()
            .map(|(_, target)| target.clone().into())
            .collect();
        let phase = match phase {
            TouchPhase::Start => "down",
            TouchPhase::Move => "move",
            TouchPhase::End => "up",
            TouchPhase::Cancel => "cancel",
        };
        let rest: Vec<String> = std::iter::once(phase.to_owned())
            .chain(
                fingers
                    .iter()
                    .zip(&targets)
                    .map(|((finger, _), target)| format!("{finger}={}", target.word(scale))),
            )
            .collect();
        self.gesture("touch", &rest, &targets);
    }

    fn type_text(&mut self, text: &str) {
        act(self, "type", &[], &[text.to_owned()]);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        let chord = typing::chord_name(key, modifiers)
            .unwrap_or_else(|| panic!("drive has no name for {modifiers:?}+{key:?}"));
        act(self, "key", &[], &[chord]);
    }

    fn key_down(&mut self, key: Key) {
        act(self, "keydown", &[], &[key_name(key)]);
    }

    fn key_up(&mut self, key: Key) {
        act(self, "keyup", &[], &[key_name(key)]);
    }

    fn back(&mut self) {
        act(self, "back", &[], &[]);
    }

    /// Lets `by` pass a frame at a time, painting each, as `drive pause` does: what waits
    /// on the clock (a submenu's hover delay, a timeout) is due by the end. `settle` does
    /// not move the clock past a delay a program asked for.
    fn pause(&mut self, by: Duration) {
        let frames = by.as_nanos().div_ceil(FRAME_INTERVAL.as_nanos());
        for _ in 0..frames {
            self.run();
        }
    }

    /// Paints until a frame stops asking to be painted again at once. Call it before a
    /// snapshot of something that animates in.
    fn settle(&mut self) {
        self.settle_until("its animations to finish", Self::settled);
    }

    /// Paints until `ready` holds, failing with `what` at a deadline. Between frames it waits
    /// for the program's wake or the delay it asked for, never a fixed time, so work on
    /// another thread is waited for rather than raced.
    fn settle_until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        let started = Instant::now();
        let mut eager = 0;
        while started.elapsed() < SETTLE_DEADLINE {
            let seen = self.wakes().count();
            self.run();
            if ready(self) {
                return;
            }
            let (changed, requested) = self.last_frame();
            // A frame that changed may ripple into the next one, so a few more are painted at
            // once. One asked for at once (an animation's) is painted anyway and takes none of
            // that allowance, so the last frame of a long animation still gets it.
            if requested == Duration::ZERO {
                eager = 0;
            }
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

    /// Whether the last frame changed what the window shows, and how soon it asked for
    /// another.
    fn last_frame(&self) -> (bool, Duration);
}

/// A test's own fixture that derefs to a harness, made a `Gui` by `impl Wraps for Fixture {}`,
/// so `fixture.click(fixture.point_of(..))` borrows the fixture, not the harness inside it
/// (two-phase borrows do not reach through `DerefMut`).
pub trait Wraps: DerefMut {}

impl<T> Gui for T
where
    T: Wraps,
    T::Target: Gui,
{
    type DocumentRef<'a>
        = <T::Target as Gui>::DocumentRef<'a>
    where
        Self: 'a;

    type DocumentMut<'a>
        = <T::Target as Gui>::DocumentMut<'a>
    where
        Self: 'a;

    fn document(&self) -> Self::DocumentRef<'_> {
        (**self).document()
    }

    fn document_mut(&mut self) -> Self::DocumentMut<'_> {
        (**self).document_mut()
    }

    fn tree(&self) -> Rc<Rendered> {
        (**self).tree()
    }

    fn size(&self) -> Vec2 {
        (**self).size()
    }

    fn pixels_per_point(&self) -> f32 {
        (**self).pixels_per_point()
    }

    fn gesture(&mut self, command: &str, words: &[String], targets: &[Target]) {
        (**self).gesture(command, words, targets);
    }

    fn run(&mut self) {
        (**self).run();
    }

    fn settled(&self) -> bool {
        (**self).settled()
    }

    fn advance(&mut self, by: Duration) {
        (**self).advance(by);
    }

    fn wakes(&self) -> &Wakes {
        (**self).wakes()
    }

    fn steps(&self) -> &[String] {
        (**self).steps()
    }

    fn last_frame(&self) -> (bool, Duration) {
        (**self).last_frame()
    }

    fn shows_whole(&self, rect: Rect) -> bool {
        (**self).shows_whole(rect)
    }

    fn reveal(&mut self, test_id: &str) {
        (**self).reveal(test_id);
    }

    fn point(&mut self, target: Target) -> Pos2 {
        (**self).point(target)
    }

    fn settle(&mut self) {
        (**self).settle();
    }
}

/// A recording of painted frames, and what they showed out of bounds.
#[derive(Default)]
pub struct Recording {
    frames: Option<beui_paint_snapshot::Snapshot>,
    out_of_bounds: Vec<String>,
}

/// Paintings of what a harness shows (guides/testing_a_gui.md, 4).
pub trait Snapshots {
    /// The window as the last frame painted it.
    fn capture(&mut self) -> beui_paint_snapshot::Snapshot;

    /// What the last frame shows out of bounds.
    fn out_of_bounds(&self) -> Vec<String>;

    fn recording(&mut self) -> &mut Recording;

    /// Keeps the current frame for the next `snapshot`.
    fn record(&mut self) {
        let frame = self.capture();
        let found = self.out_of_bounds();
        let recording = self.recording();
        recording.out_of_bounds.extend(found);
        match &mut recording.frames {
            Some(frames) => frames.append(frame),
            None => recording.frames = Some(frame),
        }
    }

    /// Compares the recorded frames, or the current frame when none were recorded, with the
    /// accepted painting, and fails on what they showed out of bounds.
    fn snapshot(&mut self, name: &str) {
        let painting = match self.recording().frames.take() {
            Some(frames) => frames,
            None => {
                let found = self.out_of_bounds();
                self.recording().out_of_bounds.extend(found);
                self.capture()
            }
        };
        beui_paint_snapshot::assert_accepted(name, &painting, beui_paint_snapshot::HOW_TO_ACCEPT);
        let found = std::mem::take(&mut self.recording().out_of_bounds);
        assert_fits(name, found);
    }
}

/// The steps so far, as `drive` commands to replay them with.
pub(super) fn replay(steps: &[String]) -> String {
    let mut replay = String::from("the steps so far, as drive commands:\n");
    for step in steps {
        replay.push_str("  ");
        replay.push_str(step);
        replay.push('\n');
    }
    replay
}

/// Whether a scroll cuts any node with this test id, so it must be revealed even where the
/// window shows its rect.
fn scroll_cuts(document: &Document, test_id: &str) -> bool {
    document
        .find_test_ids(test_id)
        .into_iter()
        .any(|node| document.is_cut(node))
}

/// Gives the gesture its targets' words before the rest of its words.
fn act<G: Gui + ?Sized>(gui: &mut G, command: &str, targets: &[Target], rest: &[String]) {
    let scale = gui.pixels_per_point();
    let words: Vec<String> = targets
        .iter()
        .map(|target| target.word(scale))
        .chain(rest.iter().cloned())
        .collect();
    gui.gesture(command, &words, targets);
}

fn found(document: &Document, test_id: &str) -> NodeId {
    document
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("no element with test id {test_id:?}"))
}

fn key_name(key: Key) -> String {
    key.name()
        .unwrap_or_else(|| panic!("drive has no name for {key:?}"))
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
