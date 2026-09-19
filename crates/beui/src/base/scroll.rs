use crate::color::Color32;
use crate::input::{Key, KeyPress};
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::base::child_list::{ChildHost, ChildList, SlotId};
use crate::base::list::Direction;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;
use crate::reactive::KeyedItems;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{
    Callback, Children, Prop, RenderFn, ScopeContext, create_effect, create_signal, owner_scope,
    settle, with_document,
};
use beui_macros::component;

const INERTIA_FRICTION: f32 = 4.5;
const MINIMUM_VELOCITY: f32 = 5.0;
const RUBBER_BAND_FACTOR: f32 = 0.55;
const SPRING_DAMPING: f32 = 24.0;
const SPRING_STIFFNESS: f32 = 180.0;
const MAX_ANIMATION_STEP: f32 = 0.05;
const STEP: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
}

impl ScrollPosition {
    pub const ZERO: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    };

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::ZERO
    }
}

type VirtualRows = KeyedItems<usize, NodeId>;

pub(crate) struct VirtualItems {
    pub(crate) count: usize,
    pub(crate) estimated: f32,
    pub(crate) first: usize,
    slot: SlotId,
    rows: Rc<VirtualRows>,
    owner: Option<ScopeContext>,
}

enum ScrollAnchor {
    Node { id: NodeId, start: f32 },
    VirtualItem { index: usize, start: f32 },
}

impl ChildHost for ScrollNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }

    fn children_changed(&mut self) {
        self.anchor = None;
    }
}

pub(crate) struct ScrollNode {
    pub(crate) direction: Direction,
    pub(crate) items: ChildList<NodeId>,
    pub(crate) virtual_items: Option<VirtualItems>,
    pub(crate) offset: f32,
    overscroll: f32,
    drag_offset: Option<f32>,
    velocity: f32,
    last_update: Instant,
    pub(crate) focused: bool,
    focus_color: Color32,
    pub(crate) position: Option<ScrollPosition>,
    anchor: Option<ScrollAnchor>,
    pub(crate) on_change: Callback<ScrollPosition>,
    pub(crate) reported: Option<ScrollPosition>,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            direction: Direction::Vertical,
            items: ChildList::default(),
            virtual_items: None,
            offset: 0.0,
            overscroll: 0.0,
            drag_offset: None,
            velocity: 0.0,
            last_update: Instant::now(),
            focused: false,
            focus_color: Color32::WHITE,
            position: None,
            anchor: None,
            on_change: Callback::empty(),
            reported: None,
        }
    }

    fn lengths(&self, doc: &mut Document, painter: &Painter, cross: f32) -> Vec<f32> {
        self.items
            .iter()
            .map(|&item| length(doc, painter, item, self.direction, cross))
            .collect()
    }

    fn nodes(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn content(&self, measured: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.count as f32 * items.estimated,
            None => measured,
        }
    }

    fn leading(&self, offset: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.first as f32 * items.estimated - offset,
            None => -offset,
        }
    }

    fn anchored_offset(&self, lengths: &[f32]) -> f32 {
        match &self.anchor {
            Some(ScrollAnchor::Node { id, start }) => self
                .items
                .iter()
                .position(|item| item == id)
                .map_or(self.offset, |index| {
                    lengths[..index].iter().sum::<f32>() - start
                }),
            Some(ScrollAnchor::VirtualItem { index, start }) => self
                .virtual_items
                .as_ref()
                .map_or(self.offset, |items| *index as f32 * items.estimated - start),
            None => self.offset,
        }
    }

    fn remember_anchor(&mut self, doc: &mut Document, painter: &Painter, cross: f32) {
        if let Some(items) = &self.virtual_items {
            self.anchor = (items.count > 0).then_some(ScrollAnchor::VirtualItem {
                index: items.first,
                start: items.first as f32 * items.estimated - self.offset,
            });
            return;
        }
        let mut start = -self.offset;
        self.anchor = None;
        for (&id, length) in self.items.iter().zip(self.lengths(doc, painter, cross)) {
            if start + length > 0.0 {
                self.anchor = Some(ScrollAnchor::Node { id, start });
                break;
            }
            start += length;
        }
    }

    fn revealing(
        &self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        item: NodeId,
        focused: NodeId,
    ) -> Option<f32> {
        let (_, cross) = self.direction.main_and_cross(rect.size());
        let lengths = self.lengths(doc, painter, cross);
        let index = self.items.iter().position(|id| *id == item)?;
        let start = self.direction.main(rect.min.to_vec2())
            + lengths[..index].iter().sum::<f32>()
            + self.leading(0.0)
            - self.offset;
        let placed = item_rect(self.direction, rect, start, lengths[index]);
        let mut rects = HashMap::new();
        crate::layout::layout(doc, painter, item, placed, &mut rects);
        let target = rects.get(&focused).copied().unwrap_or(placed);
        revealed_offset(self.direction, rect, target, self.offset)
    }

    fn position(&self, doc: &mut Document, painter: &Painter, rect: Rect) -> ScrollPosition {
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let lengths = match self.virtual_items {
            Some(_) => Vec::new(),
            None => self.lengths(doc, painter, cross),
        };
        ScrollPosition {
            offset: self.anchored_offset(&lengths),
            content: self.content(lengths.iter().sum()),
            viewport: main,
        }
    }

    fn realize(&mut self, doc: &mut Document, rect: Rect) {
        let Some(items) = self.virtual_items.take() else {
            return;
        };
        let VirtualItems {
            count,
            estimated,
            slot,
            rows,
            owner,
            ..
        } = items;
        let (main, _) = self.direction.main_and_cross(rect.size());
        let first = if estimated > 0.0 {
            ((self.offset / estimated) as usize).min(count.saturating_sub(1))
        } else {
            0
        };
        let leading = first as f32 * estimated - self.offset;
        let fit = match estimated > 0.0 {
            true => ((main - leading) / estimated).ceil().max(0.0) as usize,
            false => 0,
        };
        let last = first.saturating_add(fit).min(count);
        let range: Vec<usize> = (first..last).collect();
        let mapping = match owner.as_ref().filter(|owner| owner.is_alive()) {
            Some(owner) => settle(|| owner.run(|| rows.map(range))),
            None => settle(|| rows.map(range)),
        };
        mapping.commit(|realized, evicted| {
            self.items.fill(slot, realized);
            for item in evicted {
                doc.remove_node(item);
            }
        });
        self.virtual_items = Some(VirtualItems {
            count,
            estimated,
            first,
            slot,
            rows,
            owner,
        });
    }

    fn drag(&mut self, position: &mut ScrollPosition, delta: f32) {
        let raw = self.drag_offset.unwrap_or(position.offset) - delta;
        self.drag_offset = Some(raw);
        position.offset = raw.clamp(0.0, position.max_offset());
        self.overscroll = rubber_band(raw - position.offset, position.viewport);
        self.velocity = 0.0;
    }

    fn release(&mut self, velocity: f32) {
        self.drag_offset = None;
        self.velocity = if self.overscroll == 0.0 {
            velocity
        } else {
            velocity * 0.35
        };
        if self.velocity.abs() < MINIMUM_VELOCITY && self.overscroll == 0.0 {
            self.velocity = 0.0;
        }
    }

    fn animate(&mut self, position: &mut ScrollPosition, elapsed: f32) {
        let elapsed = elapsed.min(MAX_ANIMATION_STEP);
        if elapsed <= 0.0 {
            return;
        }
        if self.overscroll != 0.0 {
            let acceleration = -SPRING_STIFFNESS * self.overscroll - SPRING_DAMPING * self.velocity;
            self.velocity += acceleration * elapsed;
            self.overscroll += self.velocity * elapsed;
            if self.overscroll.abs() < 0.25 && self.velocity.abs() < MINIMUM_VELOCITY {
                self.overscroll = 0.0;
                self.velocity = 0.0;
            }
            return;
        }
        if self.velocity == 0.0 {
            return;
        }
        let raw = position.offset + self.velocity * elapsed;
        position.offset = raw.clamp(0.0, position.max_offset());
        self.velocity *= (-INERTIA_FRICTION * elapsed).exp();
        if raw != position.offset {
            self.overscroll = raw - position.offset;
        } else if self.velocity.abs() < MINIMUM_VELOCITY {
            self.velocity = 0.0;
        }
    }

    fn animating(&self) -> bool {
        self.overscroll != 0.0 || self.velocity != 0.0
    }
}

fn rubber_band(distance: f32, viewport: f32) -> f32 {
    if distance == 0.0 {
        return 0.0;
    }
    let dimension = viewport.max(1.0);
    let magnitude =
        dimension * (1.0 - 1.0 / (distance.abs() * RUBBER_BAND_FACTOR / dimension + 1.0));
    magnitude.copysign(distance)
}

fn revealed_offset(direction: Direction, viewport: Rect, item: Rect, offset: f32) -> Option<f32> {
    let length = direction.main(viewport.size());
    let start = direction.main(item.min - viewport.min) + offset;
    let end = direction.main(item.max - viewport.min) + offset;
    let revealed = if start < offset {
        start
    } else if end > offset + length {
        (end - length).min(start)
    } else {
        return None;
    };
    Some(revealed.max(0.0))
}

impl Element for ScrollNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (_, cross) = self.direction.main_and_cross(available);
        let content = self
            .nodes()
            .into_iter()
            .map(|item| {
                let size = crate::layout::measure(
                    doc,
                    painter,
                    item,
                    self.direction.axes(f32::INFINITY, cross),
                );
                self.direction.main_and_cross(size).1
            })
            .fold(0.0_f32, f32::max);
        self.direction.axes(0.0, content)
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let virtualised = self.virtual_items.is_some();
        let mut lengths = match virtualised {
            true => Vec::new(),
            false => self.lengths(doc, painter, cross),
        };
        self.offset = self.anchored_offset(&lengths).max(0.0);
        if virtualised {
            self.realize(doc, rect);
            lengths = self.lengths(doc, painter, cross);
        }
        let content = self.content(lengths.iter().sum());
        let position = ScrollPosition {
            offset: self.offset.clamp(0.0, (content - main).max(0.0)),
            content,
            viewport: main,
        };
        self.offset = position.offset;
        let grid = doc.pixel_grid();
        let offset = grid.snap(position.offset + self.overscroll);
        let start = self.direction.main(rect.min.to_vec2());
        let mut cursor = start + grid.snap(self.leading(offset));
        let clipped = painter.with_clip_rect(rect);
        for (&item, length) in self.items.iter().zip(&lengths) {
            if cursor >= start + main {
                break;
            }
            if cursor + length > start {
                crate::layout::layout(
                    doc,
                    &clipped,
                    item,
                    item_rect(self.direction, rect, cursor, *length),
                    out,
                );
            }
            cursor += length;
        }
        if self.anchor.is_none() {
            self.remember_anchor(doc, painter, cross);
        }
        self.position = Some(position);
        if !self.on_change.is_empty() && self.reported != Some(position) {
            self.reported = Some(position);
            let on_change = &self.on_change;
            settle(|| on_change.call(position));
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(rect);
        for item in self.items.iter() {
            if rects.contains_key(item) {
                crate::paint::paint(doc, &clipped, rects, *item);
            }
        }
        if self.focused {
            painter.rect_stroke(rect.shrink(1.0), 0.0, 2.0, self.focus_color);
        }
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        let accepts_focus = (input.pressed_this_frame && !input.touch_started)
            || (input.touch_ended && !input.touch_dragged && !input.touch_cancelled);
        if accepts_focus && input.pointer_pos.is_some_and(|pos| rect.contains(pos)) {
            *focus_target = Some(id);
        }
        let mut position = self.position(doc, painter, rect);
        let anchored_offset = position.offset;
        let previous_overscroll = self.overscroll;
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        let touch_target = input.touch_scroll_target == Some(id);
        if !touch_target {
            self.drag_offset = None;
            self.animate(&mut position, elapsed);
        }
        let wheel = self.direction.main(input.scroll);
        if input.wheel_target == Some(id) && wheel != 0.0 {
            self.velocity = 0.0;
            self.overscroll = 0.0;
            position.offset -= wheel;
        }
        if touch_target {
            if input.touch_started {
                self.drag_offset = Some(position.offset);
                self.velocity = 0.0;
            }
            let dragged = self.direction.main(input.touch_scroll_delta);
            if dragged != 0.0 {
                self.drag(&mut position, dragged);
            }
            if input.touch_ended {
                self.release(-self.direction.main(input.touch_velocity));
            } else if input.touch_cancelled {
                self.release(0.0);
            }
        }
        position.offset = position.offset.clamp(0.0, position.max_offset());

        if self.offset != position.offset || self.overscroll != previous_overscroll {
            doc.arena.invalidate_node(id);
            self.offset = position.offset;
        }
        if position.offset != anchored_offset {
            self.anchor = None;
        }
        if self.animating() {
            painter.ctx().request_repaint();
        }

        self.nodes()
    }

    fn children(&self) -> Vec<NodeId> {
        self.nodes()
    }

    fn kind(&self) -> &'static str {
        "scroll"
    }

    fn detail(&self) -> Option<String> {
        let horizontal = (self.direction == Direction::Horizontal).then_some("horizontal");
        let realized = self.virtual_items.as_ref().map(|items| {
            format!(
                "{}..{} of {}",
                items.first,
                items.first + self.items.len(),
                items.count
            )
        });
        let detail = [horizontal.map(str::to_string), realized]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!detail.is_empty()).then_some(detail)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn length(
    doc: &mut Document,
    painter: &Painter,
    item: NodeId,
    direction: Direction,
    cross: f32,
) -> f32 {
    direction.main(crate::layout::measure(
        doc,
        painter,
        item,
        direction.axes(f32::INFINITY, cross),
    ))
}

fn item_rect(direction: Direction, rect: Rect, start: f32, length: f32) -> Rect {
    match direction {
        Direction::Horizontal => {
            Rect::from_min_size(pos2(start, rect.top()), vec2(length, rect.height()))
        }
        Direction::Vertical => {
            Rect::from_min_size(pos2(rect.left(), start), vec2(rect.width(), length))
        }
    }
}

impl Document {
    pub(crate) fn create_scroll(&mut self) -> NodeId {
        self.arena.insert(ScrollNode::new())
    }

    pub(crate) fn set_scroll_direction(&mut self, scroll: NodeId, direction: Direction) {
        if self.arena.get_as::<ScrollNode>(scroll).direction == direction {
            return;
        }
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        node.direction = direction;
        node.anchor = None;
    }

    pub(crate) fn set_scroll_virtual_items(
        &mut self,
        scroll: NodeId,
        count: usize,
        estimated_height: f32,
        build: impl Fn(usize) -> NodeId + 'static,
    ) {
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        if matches!(node.anchor, Some(ScrollAnchor::Node { .. })) {
            node.anchor = None;
        }
        let items = node.items.take_all();
        for item in items {
            self.remove_node(item);
        }
        let owner = owner_scope();
        let slot = self.arena.get_mut_as::<ScrollNode>(scroll).items.open();
        self.arena.get_mut_as::<ScrollNode>(scroll).virtual_items = Some(VirtualItems {
            count,
            estimated: estimated_height.max(0.0),
            first: 0,
            slot,
            rows: Rc::new(KeyedItems::new(build)),
            owner,
        });
    }

    pub fn scroll_offset(&self, scroll: NodeId) -> f32 {
        self.arena.get_as::<ScrollNode>(scroll).offset
    }

    pub(crate) fn set_scroll_offset(&mut self, scroll: NodeId, offset: f32) {
        let node = self.arena.get_as::<ScrollNode>(scroll);
        if node.offset == offset
            && node.anchor.is_none()
            && node.overscroll == 0.0
            && node.velocity == 0.0
        {
            return;
        }
        let node = self.arena.get_mut_as::<ScrollNode>(scroll);
        node.offset = offset;
        node.anchor = None;
        node.overscroll = 0.0;
        node.drag_offset = None;
        node.velocity = 0.0;
        node.last_update = Instant::now();
    }

    #[cfg(test)]
    pub(crate) fn scroll_overscroll(&self, scroll: NodeId) -> f32 {
        self.arena.get_as::<ScrollNode>(scroll).overscroll
    }

    #[cfg(test)]
    pub(crate) fn scroll_is_animating(&self, scroll: NodeId) -> bool {
        self.arena.get_as::<ScrollNode>(scroll).animating()
    }

    pub(crate) fn set_scroll_on_change(
        &mut self,
        scroll: NodeId,
        handler: impl FnMut(ScrollPosition) + 'static,
    ) {
        self.arena
            .get_mut_as::<ScrollNode>(scroll)
            .on_change
            .set(handler);
    }
}

impl Document {
    fn reveal_scroll_index(&mut self, scroll: NodeId, index: usize) {
        let Some(&item) = self.arena.get_as::<ScrollNode>(scroll).items.get(index) else {
            return;
        };
        self.reveal_scroll_item(scroll, item);
    }

    fn reveal_scroll_item(&mut self, scroll: NodeId, item: NodeId) {
        let (Some(viewport), Some(item)) = (self.node_rect(scroll), self.node_rect(item)) else {
            return;
        };
        let direction = self.arena.get_as::<ScrollNode>(scroll).direction;
        let offset = self.scroll_offset(scroll);
        let Some(revealed) = revealed_offset(direction, viewport, item, offset) else {
            return;
        };
        self.set_scroll_offset(scroll, revealed);
    }

    pub(crate) fn set_scroll_focus_color(&mut self, scroll: NodeId, color: Color32) {
        self.arena.get_mut_as::<ScrollNode>(scroll).focus_color = color;
    }

    pub(crate) fn key_scroll(&mut self, scroll: NodeId, press: KeyPress) -> bool {
        if press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let node = self.arena.get_as::<ScrollNode>(scroll);
        let (Some(position), direction) = (node.position, node.direction) else {
            return false;
        };
        let (forwards, backwards) = match direction {
            Direction::Horizontal => (Key::ArrowRight, Key::ArrowLeft),
            Direction::Vertical => (Key::ArrowDown, Key::ArrowUp),
        };
        let offset = match press.key {
            key if key == forwards => position.offset + STEP,
            key if key == backwards => position.offset - STEP,
            Key::PageDown | Key::Space if !press.modifiers.shift => {
                position.offset + position.viewport
            }
            Key::PageUp | Key::Space => position.offset - position.viewport,
            Key::Home => 0.0,
            Key::End => position.max_offset(),
            _ => return false,
        };
        if press.pressed {
            let offset = offset.clamp(0.0, position.max_offset());
            self.set_scroll_offset(scroll, offset);
            self.arena.get_mut_as::<ScrollNode>(scroll).position =
                Some(ScrollPosition { offset, ..position });
        }
        true
    }

    pub(crate) fn key_scroll_ancestor(&mut self, press: KeyPress) -> bool {
        if !matches!(
            press.key,
            Key::ArrowUp
                | Key::ArrowDown
                | Key::ArrowLeft
                | Key::ArrowRight
                | Key::Home
                | Key::End
                | Key::PageUp
                | Key::PageDown
        ) {
            return false;
        }
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return false;
        };
        if self
            .arena
            .get(focused)
            .as_any()
            .downcast_ref::<crate::base::focusable::FocusableNode>()
            .is_some_and(|node| !node.on_step.is_empty())
        {
            return false;
        }
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return false;
        }
        for id in path.into_iter().rev().skip(1) {
            if self.arena.get(id).as_any().is::<ScrollNode>() {
                return self.key_scroll(id, press);
            }
        }
        false
    }

    pub(crate) fn reveal_node(&mut self, node: NodeId) {
        let Some(root) = self.root else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, node, &mut path) {
            return;
        }
        for id in path.into_iter().rev().skip(1) {
            if self.arena.get(id).as_any().is::<ScrollNode>() {
                self.reveal_scroll_item(id, node);
                return;
            }
        }
    }

    pub(crate) fn reveal_focus(&mut self, painter: &Painter) {
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return;
        }
        for pair in path.windows(2).rev() {
            let (scroll, item) = (pair[0], pair[1]);
            if !self.arena.get(scroll).as_any().is::<ScrollNode>() {
                continue;
            }
            let Some(rect) = self.node_rect(scroll) else {
                continue;
            };
            let element = self.arena.take(scroll);
            let offset = element
                .as_any()
                .downcast_ref::<ScrollNode>()
                .and_then(|node| node.revealing(self, painter, rect, item, focused));
            self.arena.put_back(scroll, element);
            if let Some(offset) = offset {
                self.set_scroll_offset(scroll, offset);
            }
        }
    }

    fn focus_path(&self, id: NodeId, focused: NodeId, path: &mut Vec<NodeId>) -> bool {
        path.push(id);
        if id == focused {
            return true;
        }
        for child in self.children(id) {
            if self.focus_path(child, focused, path) {
                return true;
            }
        }
        path.pop();
        false
    }
}

#[component]
pub fn VirtualList(
    count: Prop<usize>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(children)] item: Option<RenderFn<usize>>,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    on_change: Callback<ScrollPosition>,
) -> NodeId {
    let scroll = create_scroll(direction, focus_color, on_change);
    let item = item.expect("virtual_list requires an `item` builder");
    let (count_read, set_count) = create_signal(0);
    let (height_read, set_height) = create_signal(0.0);
    create_effect(move || set_count.set(count.get()));
    create_effect(move || set_height.set(item_size.get()));
    create_effect(move || {
        let (count, height) = (count_read.get(), height_read.get());
        let item = item.clone();
        with_document(|document| {
            document.set_scroll_virtual_items(scroll, count, height, move |index| item.call(index));
        });
    });
    scroll
}

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let scroll = create_scroll(direction, focus_color, on_change);
    children.mount(scroll);
    create_effect(move || {
        with_document(|document| document.set_scroll_offset(scroll, offset.get()))
    });
    create_effect(move || {
        let index = reveal.get();
        let Some(index) = index else {
            return;
        };
        with_document(|document| document.reveal_scroll_index(scroll, index));
    });
    scroll
}

fn create_scroll(
    direction: Prop<Direction>,
    focus_color: Prop<Color32>,
    on_change: Callback<ScrollPosition>,
) -> NodeId {
    let scroll = with_document(|document| {
        let scroll = document.create_scroll();
        document.set_scroll_on_change(scroll, move |position| on_change.call(position));
        scroll
    });
    create_effect(move || {
        with_document(|document| document.set_scroll_direction(scroll, direction.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_scroll_focus_color(scroll, focus_color.get()))
    });
    scroll
}
