use std::any::Any;

use crate::base::focus::Focus;
use crate::base::list::Direction;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{
    AutoscrollGesture, CursorIcon, DragGesture, Modifiers, PointerPress, ScrollGesture,
    SecondaryDrag, ZoomGesture,
};
use crate::painter::Painter;

use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};

pub struct InteractiveNode {
    pub child: Option<NodeId>,
    pub focus: Option<Focus>,
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
    pub on_forward: Callback<crate::interact::forward::ForwardedInput>,
    pub forward_at: Callback<Pos2, bool>,
    pub claim_modifiers: Option<Modifiers>,
    pub claim_at: Callback<(Pos2, Modifiers), bool>,
    pub on_press_outside: Callback<Pos2>,
}

impl Default for InteractiveNode {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractiveNode {
    fn declines(&self, input: &InteractInput, rect: Rect) -> bool {
        !self.on_forward.is_empty()
            && !self.forward_at.is_empty()
            && input
                .pointer_pos
                .is_some_and(|pos| !self.forward_at.call(pos - rect.min.to_vec2()))
    }

    pub fn new() -> Self {
        Self {
            child: None,
            focus: None,
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
            on_forward: Callback::empty(),
            forward_at: Callback::empty(),
            claim_modifiers: None,
            claim_at: Callback::empty(),
            on_press_outside: Callback::empty(),
        }
    }

    pub fn claims(&self, pos: Pos2, rect: Rect, held: Modifiers) -> bool {
        let whole = self.claim_modifiers.is_some_and(|claimed| {
            claimed.any() && held.holds(claimed) && rect.contains_half_open(pos)
        });
        whole || (!self.claim_at.is_empty() && self.claim_at.call((pos, held)))
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

impl Element for InteractiveNode {
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
        focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        if let Some(focus) = self.focus.as_ref()
            && input.pointer_over(rect)
            && ((input.pressed_this_frame && !input.touch_started)
                || (input.touch_ended && !input.touch_dragged && !input.touch_cancelled))
            && !self.declines(input, rect)
        {
            *focus_target = match focus.press_focus {
                true => Some(id),
                false => doc.focused_node(),
            };
        }
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
            && let Some(pos) = input.press_pos.or(input.pointer_pos)
            && (input.over(rect, pos) || captured)
        {
            self.armed = true;
            let press = self.press(input, rect, pos);
            self.on_press.call(press);
            self.report_active();
        }
        let secondary_yielded = doc.secondary_claim.is_some_and(|claimant| claimant != id);
        if contains_pointer
            && input.secondary_pressed_this_frame
            && !secondary_yielded
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
        if input.released_this_frame
            && self.armed
            && !input.touch_cancelled
            && (!input.touch_scrolling || holds_drag)
            && let Some(pos) = input.pointer_pos
            && self.dragged.or(input.press_pos) != Some(pos)
        {
            self.dragged = Some(pos);
            let press = self.press(input, rect, pos);
            self.on_drag.call(press);
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
        if !secondary_yielded {
            self.secondary_drag(input, rect);
        }
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
        "interactive"
    }

    fn detail(&self) -> Option<String> {
        self.focus.as_ref().map(|_| "focusable".to_owned())
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        let handlers: Vec<&str> = [
            (self.on_click.is_empty(), "click"),
            (self.on_click_at.is_empty(), "click at"),
            (self.on_press.is_empty(), "press"),
            (self.on_secondary_press.is_empty(), "secondary press"),
            (self.on_secondary_drag.is_empty(), "secondary drag"),
            (self.on_middle_click.is_empty(), "middle click"),
            (self.on_cancel.is_empty(), "cancel"),
            (self.on_hover_change.is_empty(), "hover"),
            (self.on_hover_move.is_empty(), "hover move"),
            (self.on_active_change.is_empty(), "active"),
            (self.on_drag.is_empty(), "drag"),
            (self.on_pan_drag.is_empty(), "pan"),
            (self.on_scroll.is_empty(), "scroll"),
            (self.on_scroll_drag.is_empty(), "scroll drag"),
            (self.on_autoscroll.is_empty(), "autoscroll"),
            (self.on_zoom.is_empty(), "zoom"),
            (self.on_forward.is_empty(), "forward"),
        ]
        .into_iter()
        .filter_map(|(empty, name)| (!empty).then_some(name))
        .collect();
        let states: Vec<&str> = [
            (self.hovered, "hovered"),
            (self.active, "active"),
            (self.armed, "armed"),
            (self.key_active, "key active"),
            (self.dragged.is_some(), "dragged"),
            (self.pan_active, "panning"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        let focus = self.focus.as_ref().map_or_else(
            || "none".to_owned(),
            |focus| {
                let flags: Vec<&str> = [
                    (focus.tab_stop, "tab stop"),
                    (focus.press_focus, "press"),
                    (focus.ime, "ime"),
                    (focus.keyboard_on_focus, "keyboard"),
                ]
                .into_iter()
                .filter_map(|(on, name)| on.then_some(name))
                .collect();
                match flags.is_empty() {
                    true => "focusable".to_owned(),
                    false => flags.join(", "),
                }
            },
        );
        let list = |names: Vec<&str>| match names.is_empty() {
            true => "none".to_owned(),
            false => names.join(", "),
        };
        vec![
            ("focus", focus),
            (
                "cursor",
                self.cursor
                    .map_or_else(|| "default".to_owned(), |cursor| format!("{cursor:?}")),
            ),
            (
                "scroll axis",
                self.scroll_axis
                    .map_or_else(|| "none".to_owned(), |axis| format!("{axis:?}")),
            ),
            ("handlers", list(handlers)),
            ("state", list(states)),
        ]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_interactive(&mut self, focusable: bool) -> NodeOf<InteractiveNode> {
        let mut node = InteractiveNode::new();
        node.focus = focusable.then(Focus::new);
        self.arena.insert(node)
    }

    pub fn capture_pointer(&mut self, captor: NodeId) {
        self.pointer_capture = Some(captor);
        let scrolls = |direction| {
            self.arena.contains(captor)
                && self
                    .arena
                    .get(captor)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()
                    .is_some_and(|catcher| catcher.catches_drag(direction))
        };
        let (vertical, horizontal) = (scrolls(Direction::Vertical), scrolls(Direction::Horizontal));
        self.touch_scroll_vertical = vertical.then_some(captor);
        self.touch_scroll_horizontal = horizontal.then_some(captor);
    }

    pub fn set_interactive_child(&mut self, interactive: NodeOf<InteractiveNode>, child: NodeId) {
        if self.arena.get_as::<InteractiveNode>(interactive).child == Some(child) {
            return;
        }
        self.arena.get_mut_as::<InteractiveNode>(interactive).child = Some(child);
    }

    pub fn set_interactive_cursor(
        &mut self,
        id: NodeOf<InteractiveNode>,
        cursor: Option<CursorIcon>,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).cursor != cursor {
            self.arena.touch_mut_as::<InteractiveNode>(id).cursor = cursor;
        }
    }

    pub fn set_interactive_capture_presses(
        &mut self,
        id: NodeOf<InteractiveNode>,
        capture_presses: bool,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).capture_presses != capture_presses {
            self.arena
                .touch_mut_as::<InteractiveNode>(id)
                .capture_presses = capture_presses;
        }
    }

    pub fn set_interactive_scroll_axis(
        &mut self,
        id: NodeOf<InteractiveNode>,
        axis: Option<Direction>,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).scroll_axis != axis {
            self.arena.touch_mut_as::<InteractiveNode>(id).scroll_axis = axis;
        }
    }

    pub fn set_interactive_touch_drags(&mut self, id: NodeOf<InteractiveNode>, touch_drags: bool) {
        if self.arena.get_as::<InteractiveNode>(id).touch_drags != touch_drags {
            self.arena.get_mut_as::<InteractiveNode>(id).touch_drags = touch_drags;
        }
    }

    pub fn set_interactive_touch_drag_axis(
        &mut self,
        id: NodeOf<InteractiveNode>,
        axis: Option<Direction>,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).touch_drag_axis != axis {
            self.arena.get_mut_as::<InteractiveNode>(id).touch_drag_axis = axis;
        }
    }

    pub fn set_interactive_claims_touch(
        &mut self,
        id: NodeOf<InteractiveNode>,
        claims_touch: bool,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).claims_touch != claims_touch {
            self.arena.get_mut_as::<InteractiveNode>(id).claims_touch = claims_touch;
        }
    }

    pub fn set_interactive_claim_modifiers(
        &mut self,
        id: NodeOf<InteractiveNode>,
        claimed: Option<Modifiers>,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).claim_modifiers == claimed {
            return;
        }
        self.arena.get_mut_as::<InteractiveNode>(id).claim_modifiers = claimed;
        match claimed {
            Some(_) => self.press_claimants.insert(id.id()),
            None => self.press_claimants.remove(&id.id()),
        };
    }

    pub fn set_interactive_on_press_outside(
        &mut self,
        id: NodeOf<InteractiveNode>,
        on_press_outside: Callback<Pos2>,
    ) {
        match on_press_outside.is_empty() {
            true => self.outside_watchers.remove(&id.id()),
            false => self.outside_watchers.insert(id.id()),
        };
        self.arena.get_mut_as::<InteractiveNode>(id).on_press_outside = on_press_outside;
    }

    pub(crate) fn pressed_outside(&mut self, rects: &Rects, pos: Pos2) {
        let outside: Vec<Callback<Pos2>> = self
            .outside_watchers
            .iter()
            .filter(|id| self.arena.contains(**id))
            .filter(|id| !rects.visible(*id).is_some_and(|rect| rect.contains_half_open(pos)))
            .filter_map(|id| {
                self.arena
                    .get(*id)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()
                    .map(|node| node.on_press_outside.clone())
            })
            .collect();
        for callback in outside {
            callback.call(pos);
            ::reactive::settle(|| {});
        }
    }

    pub fn press_claims(&self) -> Vec<(Modifiers, Rect)> {
        self.press_claimants
            .iter()
            .filter(|id| self.arena.contains(**id))
            .filter_map(|id| {
                let claimed = self
                    .arena
                    .get(*id)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()?
                    .claim_modifiers?;
                let rect = self.rects.visible(id)?;
                (claimed.any() && rect.is_positive()).then_some((claimed, rect))
            })
            .collect()
    }

    pub fn set_interactive_repeat_drag(&mut self, id: NodeOf<InteractiveNode>, repeat_drag: bool) {
        if self.arena.get_as::<InteractiveNode>(id).repeat_drag != repeat_drag {
            self.arena.touch_mut_as::<InteractiveNode>(id).repeat_drag = repeat_drag;
        }
    }

    pub fn set_interactive_key_active(&mut self, id: NodeOf<InteractiveNode>, key_active: bool) {
        if !self.contains(id) {
            return;
        }
        let interactive = self.arena.touch_mut_as::<InteractiveNode>(id);
        interactive.key_active = key_active;
        let active = interactive.is_active();
        if active == interactive.active {
            return;
        }
        interactive.active = active;
        let on_active_change = interactive.on_active_change.clone();
        on_active_change.call(active);
    }
}
