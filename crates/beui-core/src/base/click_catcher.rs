use std::any::Any;

use crate::base::list::Direction;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{
    AutoscrollGesture, CursorIcon, DragGesture, PointerPress, ScrollGesture, SecondaryDrag,
    ZoomGesture,
};
use crate::painter::Painter;

use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, Rects, NodeOf};

pub struct ClickCatcherNode {
    pub child: Option<NodeId>,
    pub cursor: Option<CursorIcon>,
    pub scroll_axis: Option<Direction>,
    pub armed: bool,
    pub capture_presses: bool,
    pub repeat_drag: bool,
    pub touch_drags: bool,
    pub touch_drag_axis: Option<Direction>,
    pub claims_touch: bool,
    pub key_active: bool,
    pub hovered: bool,
    pub hover_pos: Option<Pos2>,
    pub active: bool,
    pub dragged: Option<Pos2>,
    pub pan_active: bool,
    pub middle_dragged: Option<Pos2>,
    pub middle_armed: bool,
    pub secondary_dragged: Option<Pos2>,
    pub on_click: ClickCallback,
    pub on_click_at: Callback<PointerPress>,
    pub on_hover_change: Callback<bool>,
    pub on_hover_move: Callback<PointerPress>,
    pub on_active_change: Callback<bool>,
    pub on_cancel: ClickCallback,
    pub on_middle_click: ClickCallback,
    pub on_press: Callback<PointerPress>,
    pub on_secondary_press: Callback<PointerPress>,
    pub on_secondary_drag: Callback<SecondaryDrag>,
    pub on_drag: Callback<PointerPress>,
    pub on_pan_drag: Callback<Vec2>,
    pub on_pan_active_change: Callback<bool>,
    pub on_scroll: Callback<ScrollGesture>,
    pub on_scroll_drag: Callback<DragGesture>,
    pub on_autoscroll: Callback<AutoscrollGesture>,
    pub on_zoom: Callback<ZoomGesture>,
    pub capture_at: Callback<Pos2, bool>,
    pub intercept_at: Callback<Pos2, bool>,
}

impl Default for ClickCatcherNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickCatcherNode {
    pub fn new() -> Self {
        Self {
            child: None,
            cursor: None,
            scroll_axis: None,
            armed: false,
            capture_presses: false,
            repeat_drag: false,
            touch_drags: false,
            touch_drag_axis: None,
            claims_touch: true,
            key_active: false,
            hovered: false,
            hover_pos: None,
            active: false,
            dragged: None,
            pan_active: false,
            middle_dragged: None,
            middle_armed: false,
            secondary_dragged: None,
            on_click: ClickCallback::empty(),
            on_click_at: Callback::empty(),
            on_hover_change: Callback::empty(),
            on_hover_move: Callback::empty(),
            on_active_change: Callback::empty(),
            on_cancel: ClickCallback::empty(),
            on_middle_click: ClickCallback::empty(),
            on_press: Callback::empty(),
            on_secondary_press: Callback::empty(),
            on_secondary_drag: Callback::empty(),
            on_drag: Callback::empty(),
            on_pan_drag: Callback::empty(),
            on_pan_active_change: Callback::empty(),
            on_scroll: Callback::empty(),
            on_scroll_drag: Callback::empty(),
            on_autoscroll: Callback::empty(),
            on_zoom: Callback::empty(),
            capture_at: Callback::empty(),
            intercept_at: Callback::empty(),
        }
    }

    pub fn takes_presses(&self) -> bool {
        self.capture_presses
            || !self.on_click.is_empty()
            || !self.on_click_at.is_empty()
            || !self.on_press.is_empty()
            || !self.on_secondary_press.is_empty()
    }

    pub fn claims_touches(&self) -> bool {
        self.claims_touch && self.takes_presses()
    }

    pub fn wants_gestures(&self) -> bool {
        !self.on_zoom.is_empty() || !self.on_pan_drag.is_empty()
    }

    pub fn wants_autoscroll(&self) -> bool {
        !self.on_autoscroll.is_empty()
    }

    pub fn claims_middle(&self) -> bool {
        self.wants_autoscroll() || !self.on_pan_drag.is_empty() || !self.on_middle_click.is_empty()
    }

    pub fn wants_wheel(&self, wheel: Vec2) -> bool {
        !self.on_scroll.is_empty() && self.along(wheel)
    }

    pub fn catches_drag(&self, direction: Direction) -> bool {
        self.touch_drags
            || self.touch_drag_axis == Some(direction)
            || (!self.on_scroll_drag.is_empty()
                && self.scroll_axis.is_none_or(|axis| axis == direction))
    }

    fn along(&self, wheel: Vec2) -> bool {
        match self.scroll_axis {
            Some(axis) => axis.main(wheel) != 0.0,
            None => wheel != Vec2::ZERO,
        }
    }

    pub fn is_active(&self) -> bool {
        self.armed || self.key_active
    }

    fn report_active(&mut self) {
        let active = self.is_active();
        if active != self.active {
            self.active = active;
            self.on_active_change.call(active);
        }
    }

    fn pan_drag(&mut self, input: &InteractInput, id: NodeId, contains_pointer: bool) {
        if input.middle_pressed_this_frame && contains_pointer {
            self.middle_dragged = input.pointer_pos;
        }
        if !input.middle_down {
            self.middle_dragged = None;
        }
        let fingers = input.zoom_target == Some(id);
        let active = self.middle_dragged.is_some() || fingers;
        if active != self.pan_active {
            self.pan_active = active;
            self.on_pan_active_change.call(active);
        }
        if let Some(previous) = self.middle_dragged
            && let Some(pos) = input.pointer_pos
            && pos != previous
        {
            self.middle_dragged = Some(pos);
            self.on_pan_drag.call(pos - previous);
        }
        if fingers && input.touch_pan != Vec2::ZERO {
            self.on_pan_drag.call(input.touch_pan);
        }
    }

    fn middle_click(&mut self, input: &InteractInput, contains_pointer: bool) {
        if self.on_middle_click.is_empty() {
            return;
        }
        if input.middle_pressed_this_frame {
            self.middle_armed = contains_pointer;
        }
        if input.middle_released_this_frame {
            if self.middle_armed && contains_pointer {
                self.on_middle_click.call();
            }
            self.middle_armed = false;
        }
    }

    fn secondary_drag(&mut self, input: &InteractInput, rect: Rect) {
        if self.on_secondary_drag.is_empty() {
            return;
        }
        let Some(drag) = input.secondary_drag else {
            if let Some(pos) = self.secondary_dragged.take() {
                self.on_secondary_drag.call(SecondaryDrag {
                    from: pos,
                    pos,
                    started: false,
                    ended: false,
                    cancelled: true,
                    modifiers: input.modifiers,
                });
            }
            return;
        };
        if drag.started && input.over(rect, drag.from) {
            self.secondary_dragged = Some(drag.from);
            self.on_secondary_drag.call(drag);
        } else if self.secondary_dragged.is_some()
            && (drag.ended || drag.cancelled || self.secondary_dragged != Some(drag.pos))
        {
            self.secondary_dragged = Some(drag.pos);
            self.on_secondary_drag.call(drag);
        }
        if drag.ended || drag.cancelled {
            self.secondary_dragged = None;
        }
    }

    fn press(&self, input: &InteractInput, rect: Rect, pos: Pos2) -> PointerPress {
        PointerPress {
            pos,
            fraction: fraction(rect, pos),
            clicks: input.clicks,
            modifiers: input.modifiers,
            touch: input.touch_started
                || input.touch_active
                || input.touch_ended
                || input.touch_cancelled,
        }
    }
}

fn fraction(rect: Rect, pos: Pos2) -> Vec2 {
    let axis = |offset: f32, length: f32| {
        if length > 0.0 {
            (offset / length).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    Vec2::new(
        axis(pos.x - rect.left(), rect.width()),
        axis(pos.y - rect.top(), rect.height()),
    )
}

impl Element for ClickCatcherNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        false
    }

    fn engaged(&self) -> bool {
        self.armed
            || self.key_active
            || self.active
            || self.hovered
            || self.hover_pos.is_some()
            || self.dragged.is_some()
            || self.pan_active
            || self.middle_dragged.is_some()
            || self.middle_armed
            || self.secondary_dragged.is_some()
    }

    fn disengage(&mut self, _doc: &mut Document) {
        self.hover_pos = None;
        if self.hovered {
            self.hovered = false;
            self.on_hover_change.call(false);
        }
    }

    fn captures(&mut self, _doc: &mut Document, pos: Pos2, rect: Rect) -> bool {
        (self.capture_presses && rect.contains_half_open(pos)) || self.capture_at.call(pos)
    }

    fn intercepts(&mut self, _doc: &mut Document, pos: Pos2, _rect: Rect) -> bool {
        self.intercept_at.call(pos)
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        if !input.pointer_down && !input.released_this_frame {
            self.armed = false;
            self.dragged = None;
        }
        let captured = doc.pointer_capture == Some(id);
        let yielded = doc.pointer_capture.is_some_and(|captor| captor != id);
        if yielded {
            self.armed = false;
            self.dragged = None;
        }
        let contains_pointer = input.pointer_over(rect);
        if input.pressed_this_frame
            && !yielded
            && (contains_pointer || captured)
            && let Some(pos) = input.pointer_pos
        {
            self.armed = true;
            let press = self.press(input, rect, pos);
            self.on_press.call(press);
            self.report_active();
        }
        if contains_pointer
            && input.secondary_pressed_this_frame
            && let Some(pos) = input.pointer_pos
        {
            let press = self.press(input, rect, pos);
            self.on_secondary_press.call(press);
        }
        let holds_drag = captured
            || self.touch_drags
            || (self.touch_drag_axis.is_some() && input.touch_scroll_target == Some(id));
        if input.touch_cancelled || (input.touch_scrolling && !holds_drag) {
            if self.armed {
                self.on_cancel.call();
            }
            self.armed = false;
            self.dragged = None;
        }
        if input.released_this_frame {
            if self.armed
                && (contains_pointer || captured || input.touch_ended)
                && !input.touch_dragged
                && !input.touch_cancelled
            {
                self.on_click.call();
                if let Some(pos) = input.pointer_pos {
                    let press = self.press(input, rect, pos);
                    self.on_click_at.call(press);
                }
            }
            self.armed = false;
            self.dragged = None;
        }
        let hovered = contains_pointer && !input.touch_active && !input.touch_ended;
        if (hovered || self.is_active())
            && let Some(cursor) = self.cursor
        {
            painter.ctx().set_cursor_icon(cursor);
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            self.on_hover_change.call(hovered);
        }
        let at = hovered.then_some(input.pointer_pos).flatten();
        if at != self.hover_pos {
            self.hover_pos = at;
            if let Some(pos) = at {
                let press = self.press(input, rect, pos);
                self.on_hover_move.call(press);
            }
        }
        self.report_active();
        if self.armed
            && input.pointer_down
            && (!input.touch_scrolling || holds_drag)
            && let Some(pos) = input.pointer_pos
            && (self.repeat_drag || self.dragged != Some(pos))
        {
            self.dragged = Some(pos);
            if self.repeat_drag {
                painter.ctx().request_repaint();
            }
            let press = self.press(input, rect, pos);
            self.on_drag.call(press);
        }
        self.middle_click(input, contains_pointer);
        self.pan_drag(input, id, contains_pointer);
        self.secondary_drag(input, rect);
        if input.wheel_target == Some(id)
            && (input.scroll != Vec2::ZERO || input.scroll_fling != Vec2::ZERO)
            && let Some(pos) = input.pointer_pos
        {
            self.on_scroll.call(ScrollGesture {
                delta: input.scroll,
                fling: input.scroll_fling,
                pos,
                modifiers: input.modifiers,
            });
        }
        if input.touch_scroll_target == Some(id) {
            self.on_scroll_drag.call(DragGesture {
                started: input.touch_started,
                ended: input.touch_ended,
                cancelled: input.touch_cancelled,
                delta: input.touch_scroll_delta,
                velocity: input.touch_velocity,
            });
        }
        if input.zoom_target == Some(id)
            && input.zoom != 1.0
            && let Some(pos) = input.zoom_pos
        {
            self.on_zoom.call(ZoomGesture {
                factor: input.zoom,
                pos,
            });
        }

        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "click-catcher"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_click_catcher(&mut self) -> NodeOf<ClickCatcherNode> {
        self.arena.insert(ClickCatcherNode::new())
    }

    pub fn capture_pointer(&mut self, captor: NodeId) {
        self.pointer_capture = Some(captor);
        let scrolls = |direction| {
            self.arena.contains(captor)
                && self
                    .arena
                    .get(captor)
                    .as_any()
                    .downcast_ref::<ClickCatcherNode>()
                    .is_some_and(|catcher| catcher.catches_drag(direction))
        };
        let (vertical, horizontal) = (scrolls(Direction::Vertical), scrolls(Direction::Horizontal));
        self.touch_scroll_vertical = vertical.then_some(captor);
        self.touch_scroll_horizontal = horizontal.then_some(captor);
    }

    pub fn set_click_catcher_child(&mut self, click_catcher: NodeOf<ClickCatcherNode>, child: NodeId) {
        if self.arena.get_as::<ClickCatcherNode>(click_catcher).child == Some(child) {
            return;
        }
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .child = Some(child);
    }

    pub fn set_click_catcher_cursor(&mut self, id: NodeOf<ClickCatcherNode>, cursor: Option<CursorIcon>) {
        if self.arena.get_as::<ClickCatcherNode>(id).cursor != cursor {
            self.arena.touch_mut_as::<ClickCatcherNode>(id).cursor = cursor;
        }
    }

    pub fn set_click_catcher_capture_presses(&mut self, id: NodeOf<ClickCatcherNode>, capture_presses: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).capture_presses != capture_presses {
            self.arena
                .touch_mut_as::<ClickCatcherNode>(id)
                .capture_presses = capture_presses;
        }
    }

    pub fn set_click_catcher_scroll_axis(&mut self, id: NodeOf<ClickCatcherNode>, axis: Option<Direction>) {
        if self.arena.get_as::<ClickCatcherNode>(id).scroll_axis != axis {
            self.arena.touch_mut_as::<ClickCatcherNode>(id).scroll_axis = axis;
        }
    }

    pub fn set_click_catcher_touch_drags(&mut self, id: NodeOf<ClickCatcherNode>, touch_drags: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).touch_drags != touch_drags {
            self.arena.get_mut_as::<ClickCatcherNode>(id).touch_drags = touch_drags;
        }
    }

    pub fn set_click_catcher_touch_drag_axis(&mut self, id: NodeOf<ClickCatcherNode>, axis: Option<Direction>) {
        if self.arena.get_as::<ClickCatcherNode>(id).touch_drag_axis != axis {
            self.arena
                .get_mut_as::<ClickCatcherNode>(id)
                .touch_drag_axis = axis;
        }
    }

    pub fn set_click_catcher_claims_touch(&mut self, id: NodeOf<ClickCatcherNode>, claims_touch: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).claims_touch != claims_touch {
            self.arena.get_mut_as::<ClickCatcherNode>(id).claims_touch = claims_touch;
        }
    }

    pub fn set_click_catcher_repeat_drag(&mut self, id: NodeOf<ClickCatcherNode>, repeat_drag: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).repeat_drag != repeat_drag {
            self.arena.touch_mut_as::<ClickCatcherNode>(id).repeat_drag = repeat_drag;
        }
    }

    pub fn set_click_catcher_key_active(&mut self, id: NodeOf<ClickCatcherNode>, key_active: bool) {
        if !self.contains(id) {
            return;
        }
        let click_catcher = self.arena.touch_mut_as::<ClickCatcherNode>(id);
        click_catcher.key_active = key_active;
        let active = click_catcher.is_active();
        if active == click_catcher.active {
            return;
        }
        click_catcher.active = active;
        let on_active_change = click_catcher.on_active_change.clone();
        on_active_change.call(active);
    }
}
