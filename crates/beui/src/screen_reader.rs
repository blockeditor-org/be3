mod curtain;
mod speech;

use std::time::{Duration, Instant};

use accesskit::{Action, ActionRequest, Node as AccessNode, NodeId as AccessNodeId, TreeId};

use crate::context::Context;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, pos2};
use crate::input::{Event, Key, TouchId, TouchPhase};
use crate::painter::Painter;

use speech::Nodes;

const TRANSCRIPT: usize = 5;
const LIVE_INTERVAL: Duration = Duration::from_millis(400);
const SWIPE_TIME: Duration = Duration::from_millis(400);
const SWIPE_DISTANCE: f32 = 28.0;
const TAP_DISTANCE: f32 = 10.0;
const DOUBLE_TAP_TIME: Duration = Duration::from_millis(400);
const SCROLL_STEP: f32 = 48.0;
pub(crate) const DEFAULT_OPACITY: f32 = 0.94;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    First,
    Previous,
    Next,
    Last,
    PreviousControl,
    NextControl,
    Activate,
    Repeat,
    Decrement,
    Increment,
    ScrollUp,
    ScrollDown,
}

struct Item {
    access: AccessNodeId,
    rect: Rect,
    phrase: String,
    control: bool,
    adjustable: bool,
    scrollable: bool,
}

struct Finger {
    origin: Pos2,
    last: Pos2,
    started: Instant,
    moved: bool,
    cursor: Option<AccessNodeId>,
}

pub(crate) struct ScreenReader {
    enabled: bool,
    active: bool,
    opacity: f32,
    items: Vec<Item>,
    cursor: Option<AccessNodeId>,
    spoken: Vec<String>,
    reported: Option<(AccessNodeId, String)>,
    live: bool,
    last_spoke: Option<Instant>,
    fingers: Vec<(TouchId, Finger)>,
    finger_at: Option<Pos2>,
    multi: bool,
    scrolled: bool,
    scroll_travel: f32,
    last_tap: Option<Instant>,
    painted: Rect,
}

impl Default for ScreenReader {
    fn default() -> Self {
        Self {
            enabled: false,
            active: false,
            opacity: DEFAULT_OPACITY,
            items: Vec::new(),
            cursor: None,
            spoken: Vec::new(),
            reported: None,
            live: false,
            last_spoke: None,
            fingers: Vec::new(),
            finger_at: None,
            multi: false,
            scrolled: false,
            scroll_travel: 0.0,
            last_tap: None,
            painted: Rect::NOTHING,
        }
    }
}

impl ScreenReader {
    pub(crate) fn configure(&mut self, enabled: bool, opacity: f32) {
        self.enabled = enabled;
        self.opacity = opacity;
    }

    pub(crate) fn painting(&self) -> bool {
        self.enabled || self.painted.is_positive()
    }

    pub(crate) fn show(
        &mut self,
        target: &Document,
        ctx: &Context,
        content: Rect,
        commands: Vec<Command>,
        keyboard: bool,
    ) {
        if !self.enabled {
            self.stop();
            return;
        }
        self.items = collect(target);
        if !self.active {
            self.active = true;
            self.cursor = None;
            self.reported = None;
            self.spoken.clear();
            self.say("Screen reader simulation. The curtain hides the document.".to_owned());
        }
        if self.index().is_none() {
            self.cursor = self.items.first().map(|item| item.access);
        }
        let mut commands = commands;
        if keyboard {
            commands.extend(keyboard_commands(ctx));
        }
        self.touch(ctx, content, &mut commands);
        for command in commands {
            self.run(ctx, command);
        }
        self.report();
    }

    fn stop(&mut self) {
        self.active = false;
        self.items.clear();
        self.cursor = None;
        self.reported = None;
        self.spoken.clear();
        self.release();
    }

    fn release(&mut self) {
        self.fingers.clear();
        self.finger_at = None;
        self.multi = false;
        self.scrolled = false;
        self.scroll_travel = 0.0;
        self.last_tap = None;
    }

    fn index(&self) -> Option<usize> {
        let cursor = self.cursor?;
        self.items.iter().position(|item| item.access == cursor)
    }

    fn current(&self) -> Option<&Item> {
        self.items.get(self.index()?)
    }

    fn say(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.last_spoke = Some(Instant::now());
        if self.spoken.last() == Some(&text) {
            return;
        }
        self.spoken.push(text);
        if self.spoken.len() > TRANSCRIPT {
            self.spoken.remove(0);
        }
    }

    fn report(&mut self) {
        let Some(item) = self.current() else {
            self.reported = None;
            return;
        };
        let (access, phrase) = (item.access, item.phrase.clone());
        let moved = self.reported.as_ref().is_none_or(|(id, _)| *id != access);
        let changed = self
            .reported
            .as_ref()
            .is_none_or(|(_, text)| *text != phrase);
        if !moved && !changed {
            return;
        }
        if !moved
            && self.live
            && self
                .last_spoke
                .is_some_and(|at| at.elapsed() < LIVE_INTERVAL)
        {
            return;
        }
        self.live = !moved;
        self.reported = Some((access, phrase.clone()));
        self.say(phrase);
    }

    fn run(&mut self, ctx: &Context, command: Command) {
        self.live = false;
        match command {
            Command::First => self.go(ctx, 0),
            Command::Last => self.go(ctx, self.items.len().saturating_sub(1)),
            Command::Next => self.step(ctx, true, false),
            Command::Previous => self.step(ctx, false, false),
            Command::NextControl => self.step(ctx, true, true),
            Command::PreviousControl => self.step(ctx, false, true),
            Command::Activate => self.act(ctx, Action::Click),
            Command::Increment => self.adjust(ctx, Action::Increment),
            Command::Decrement => self.adjust(ctx, Action::Decrement),
            Command::ScrollUp => self.scroll(ctx, Action::ScrollUp),
            Command::ScrollDown => self.scroll(ctx, Action::ScrollDown),
            Command::Repeat => {
                if let Some(phrase) = self.current().map(|item| item.phrase.clone()) {
                    self.spoken.pop_if(|last| *last == phrase);
                    self.say(phrase);
                }
            }
        }
    }

    fn step(&mut self, ctx: &Context, forward: bool, controls: bool) {
        if self.items.is_empty() {
            return;
        }
        let Some(current) = self.index() else {
            return self.go(ctx, 0);
        };
        let candidates: Vec<usize> = if forward {
            (current + 1..self.items.len()).collect()
        } else {
            (0..current).rev().collect()
        };
        match candidates
            .into_iter()
            .find(|index| !controls || self.items[*index].control)
        {
            Some(index) => self.go(ctx, index),
            None if forward => self.say("End of the document".to_owned()),
            None => self.say("Start of the document".to_owned()),
        }
    }

    fn go(&mut self, ctx: &Context, index: usize) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        if self.cursor == Some(item.access) {
            return;
        }
        self.cursor = Some(item.access);
        self.live = false;
        if item.control {
            request(ctx, Action::Focus, item.access);
        }
        ctx.request_repaint();
    }

    fn act(&mut self, ctx: &Context, action: Action) {
        let Some(item) = self.current() else {
            return;
        };
        if !item.control {
            return self.say("No action here".to_owned());
        }
        request(ctx, action, item.access);
    }

    fn adjust(&mut self, ctx: &Context, action: Action) {
        let Some(item) = self.current() else {
            return;
        };
        if !item.adjustable {
            return self.say("Nothing to adjust here".to_owned());
        }
        request(ctx, action, item.access);
    }

    fn scroll(&mut self, ctx: &Context, action: Action) {
        let Some(index) = self.index() else {
            return;
        };
        let rect = self.items[index].rect;
        let target = (0..=index)
            .rev()
            .find(|candidate| {
                let item = &self.items[*candidate];
                item.scrollable && (*candidate == index || item.rect.intersects(rect))
            })
            .map(|candidate| self.items[candidate].access);
        match target {
            Some(access) => request(ctx, action, access),
            None => self.say("Nothing to scroll here".to_owned()),
        }
    }

    fn touch(&mut self, ctx: &Context, content: Rect, commands: &mut Vec<Command>) {
        for event in ctx.input(|input| input.events.clone()) {
            let Event::Touch { id, phase, pos, .. } = event else {
                continue;
            };
            match phase {
                TouchPhase::Start if content.contains(pos) => self.begin(ctx, id, pos),
                TouchPhase::Start => {}
                TouchPhase::Move => self.advance(ctx, id, pos, commands),
                TouchPhase::End => self.finish(ctx, id, pos, false, commands),
                TouchPhase::Cancel => self.finish(ctx, id, pos, true, commands),
            }
        }
    }

    fn begin(&mut self, ctx: &Context, id: TouchId, pos: Pos2) {
        self.fingers.push((
            id,
            Finger {
                origin: pos,
                last: pos,
                started: Instant::now(),
                moved: false,
                cursor: self.cursor,
            },
        ));
        if self.fingers.len() > 1 {
            self.multi = true;
            self.finger_at = None;
            self.scroll_travel = 0.0;
            self.cursor = self.fingers[0].1.cursor;
            return;
        }
        self.finger_at = Some(pos);
        self.explore(ctx, pos);
    }

    fn advance(&mut self, ctx: &Context, id: TouchId, pos: Pos2, commands: &mut Vec<Command>) {
        let Some(index) = self.fingers.iter().position(|(other, _)| *other == id) else {
            return;
        };
        let leader = index == 0;
        let delta = {
            let finger = &mut self.fingers[index].1;
            let delta = pos - finger.last;
            finger.last = pos;
            if finger.origin.distance(pos) >= TAP_DISTANCE {
                finger.moved = true;
            }
            delta
        };
        if self.multi {
            if leader {
                self.scroll_travel += delta.y;
                while self.scroll_travel.abs() >= SCROLL_STEP {
                    let direction = self.scroll_travel.signum();
                    self.scroll_travel -= direction * SCROLL_STEP;
                    self.scrolled = true;
                    commands.push(match direction > 0.0 {
                        true => Command::ScrollUp,
                        false => Command::ScrollDown,
                    });
                }
            }
            return;
        }
        self.finger_at = Some(pos);
        self.explore(ctx, pos);
    }

    fn finish(
        &mut self,
        ctx: &Context,
        id: TouchId,
        pos: Pos2,
        cancelled: bool,
        commands: &mut Vec<Command>,
    ) {
        self.advance(ctx, id, pos, commands);
        let Some(index) = self.fingers.iter().position(|(other, _)| *other == id) else {
            return;
        };
        let (_, finger) = self.fingers.remove(index);
        self.finger_at = None;
        let quick = finger.started.elapsed() <= SWIPE_TIME;
        if cancelled {
            self.settle();
            return;
        }
        if self.multi {
            if self.fingers.is_empty() && !self.scrolled && !finger.moved && quick {
                commands.push(Command::Repeat);
            }
            self.settle();
            return;
        }
        let travel = pos - finger.origin;
        if quick && finger.origin.distance(pos) >= SWIPE_DISTANCE {
            self.cursor = finger.cursor;
            commands.push(match travel.x.abs() >= travel.y.abs() {
                true if travel.x > 0.0 => Command::Next,
                true => Command::Previous,
                false if travel.y < 0.0 => Command::Increment,
                false => Command::Decrement,
            });
        } else if quick && !finger.moved {
            match self.last_tap.filter(|at| at.elapsed() <= DOUBLE_TAP_TIME) {
                Some(_) => {
                    self.last_tap = None;
                    commands.push(Command::Activate);
                }
                None => self.last_tap = Some(Instant::now()),
            }
        }
        self.settle();
    }

    fn settle(&mut self) {
        if !self.fingers.is_empty() {
            return;
        }
        self.multi = false;
        self.scrolled = false;
        self.scroll_travel = 0.0;
    }

    fn explore(&mut self, ctx: &Context, pos: Pos2) {
        let Some(index) = self.items.iter().rposition(|item| item.rect.contains(pos)) else {
            return;
        };
        self.go(ctx, index);
    }

    pub(crate) fn paint(&mut self, painter: &Painter, scale: f32) -> Rect {
        let content = painter.clip_rect();
        if !self.enabled {
            let damage = self.painted;
            self.painted = Rect::NOTHING;
            return damage;
        }
        let view = curtain::View {
            content,
            opacity: self.opacity,
            spoken: &self.spoken,
            status: self.status(),
            focus: self.current().map(|item| item.rect),
            finger: self.finger_at,
        };
        let painted = curtain::paint(painter, scale, &view);
        let damage = painted.union(self.painted);
        self.painted = painted;
        damage
    }

    #[cfg(test)]
    pub(crate) fn transcript(&self) -> Vec<String> {
        self.spoken.clone()
    }

    #[cfg(test)]
    pub(crate) fn reading(&self) -> Option<String> {
        self.current().map(|item| item.phrase.clone())
    }

    #[cfg(test)]
    pub(crate) fn item_center(&self, index: usize) -> Pos2 {
        self.items
            .get(index)
            .map(|item| item.rect.center())
            .expect("the screen reader has no item at that index")
    }

    #[cfg(test)]
    pub(crate) fn items(&self) -> Vec<String> {
        self.items.iter().map(|item| item.phrase.clone()).collect()
    }

    fn status(&self) -> String {
        let total = self.items.len();
        match self.index() {
            Some(index) => format!("item {} of {}", index + 1, total),
            None => "nothing to read".to_owned(),
        }
    }
}

fn request(ctx: &Context, action: Action, target: AccessNodeId) {
    ctx.accessibility_action(ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: target,
        data: None,
    });
}

fn keyboard_commands(ctx: &Context) -> Vec<Command> {
    ctx.input(|input| input.events.iter().filter_map(shortcut).collect())
}

fn shortcut(event: &Event) -> Option<Command> {
    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    if modifiers.ctrl || modifiers.alt {
        return None;
    }
    Some(match key {
        Key::ArrowRight | Key::ArrowDown => Command::Next,
        Key::ArrowLeft | Key::ArrowUp => Command::Previous,
        Key::Tab if modifiers.shift => Command::PreviousControl,
        Key::Tab => Command::NextControl,
        Key::Enter | Key::Space => Command::Activate,
        Key::Home => Command::First,
        Key::End => Command::Last,
        Key::R => Command::Repeat,
        Key::Plus => Command::Increment,
        Key::Minus => Command::Decrement,
        Key::PageUp => Command::ScrollUp,
        Key::PageDown => Command::ScrollDown,
        _ => return None,
    })
}

fn collect(target: &Document) -> Vec<Item> {
    let Some(fragment) = target.accessibility_fragment() else {
        return Vec::new();
    };
    let nodes: Nodes = fragment.nodes.into_iter().collect();
    let mut items = Vec::new();
    gather(target, &nodes, fragment.root, false, &mut items);
    items
}

fn gather(
    target: &Document,
    nodes: &Nodes,
    access: AccessNodeId,
    inside: bool,
    items: &mut Vec<Item>,
) {
    let Some(node) = nodes.get(&access) else {
        return;
    };
    if node.is_hidden() {
        return;
    }
    let control = speech::control(node);
    if let Some(item) = item(target, nodes, access, node, control, inside) {
        items.push(item);
    }
    for child in node.children() {
        gather(target, nodes, *child, inside || control, items);
    }
}

fn item(
    target: &Document,
    nodes: &Nodes,
    access: AccessNodeId,
    node: &AccessNode,
    control: bool,
    inside: bool,
) -> Option<Item> {
    let scrollable = speech::scrollable(node);
    let named = speech::own_name(node).is_some();
    if !control && !scrollable && (inside || !named) {
        return None;
    }
    target.local_node_id(access)?;
    Some(Item {
        access,
        rect: node.bounds().map(access_rect).unwrap_or(Rect::NOTHING),
        phrase: speech::phrase(nodes, node, control),
        control,
        adjustable: speech::adjustable(node),
        scrollable,
    })
}

fn access_rect(rect: accesskit::Rect) -> Rect {
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
}
