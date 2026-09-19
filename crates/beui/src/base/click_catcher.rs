use std::any::Any;

use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{CursorIcon, PointerPress, ScrollGesture, ZoomGesture};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::reactive::{Callback, Child, ClickCallback, Prop, create_effect, with_document};

use beui_macros::component;

pub(crate) struct ClickCatcherNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) cursor: CursorIcon,
    pub(crate) armed: bool,
    pub(crate) capture_presses: bool,
    pub(crate) repeat_drag: bool,
    pub(crate) key_active: bool,
    pub(crate) hovered: bool,
    pub(crate) hover_pos: Option<Pos2>,
    pub(crate) active: bool,
    pub(crate) dragged: Option<Pos2>,
    pub(crate) pan_active: bool,
    pub(crate) middle_dragged: Option<Pos2>,
    pub(crate) on_click: ClickCallback,
    pub(crate) on_click_at: Callback<PointerPress>,
    pub(crate) on_hover_change: Callback<bool>,
    pub(crate) on_hover_move: Callback<PointerPress>,
    pub(crate) on_active_change: Callback<bool>,
    pub(crate) on_press: Callback<PointerPress>,
    pub(crate) on_secondary_press: Callback<PointerPress>,
    pub(crate) on_drag: Callback<PointerPress>,
    pub(crate) on_pan_drag: Callback<Vec2>,
    pub(crate) on_pan_active_change: Callback<bool>,
    pub(crate) on_scroll: Callback<ScrollGesture>,
    pub(crate) on_zoom: Callback<ZoomGesture>,
    pub(crate) capture_at: Callback<Pos2, bool>,
}

impl ClickCatcherNode {
    pub(crate) fn new(cursor: CursorIcon) -> Self {
        Self {
            child: None,
            cursor,
            armed: false,
            capture_presses: false,
            repeat_drag: false,
            key_active: false,
            hovered: false,
            hover_pos: None,
            active: false,
            dragged: None,
            pan_active: false,
            middle_dragged: None,
            on_click: ClickCallback::empty(),
            on_click_at: Callback::empty(),
            on_hover_change: Callback::empty(),
            on_hover_move: Callback::empty(),
            on_active_change: Callback::empty(),
            on_press: Callback::empty(),
            on_secondary_press: Callback::empty(),
            on_drag: Callback::empty(),
            on_pan_drag: Callback::empty(),
            on_pan_active_change: Callback::empty(),
            on_scroll: Callback::empty(),
            on_zoom: Callback::empty(),
            capture_at: Callback::empty(),
        }
    }

    pub(crate) fn wants_gestures(&self) -> bool {
        !self.on_scroll.is_empty() || !self.on_zoom.is_empty()
    }

    pub(crate) fn is_active(&self) -> bool {
        self.armed || self.key_active
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

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        false
    }

    fn captures(&mut self, _doc: &mut Document, pos: Pos2, rect: Rect) -> bool {
        (self.capture_presses && rect.contains(pos)) || self.capture_at.call(pos)
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
        let contains_pointer = input.pointer_pos.is_some_and(|pos| rect.contains(pos));
        if input.pressed_this_frame
            && !yielded
            && (contains_pointer || captured)
            && let Some(pos) = input.pointer_pos
        {
            self.armed = true;
            let press = self.press(input, rect, pos);
            self.on_press.call(press);
        }
        if contains_pointer
            && input.secondary_pressed_this_frame
            && let Some(pos) = input.pointer_pos
        {
            let press = self.press(input, rect, pos);
            self.on_secondary_press.call(press);
        }
        if input.touch_cancelled || (input.touch_scrolling && !captured) {
            self.armed = false;
            self.dragged = None;
        }
        if input.released_this_frame {
            if self.armed
                && (contains_pointer || captured)
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
        if hovered || self.is_active() {
            painter.ctx().set_cursor_icon(self.cursor);
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
        let active = self.is_active();
        if active != self.active {
            self.active = active;
            self.on_active_change.call(active);
        }
        if self.armed
            && input.pointer_down
            && (!input.touch_scrolling || captured)
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
        self.pan_drag(input, id, contains_pointer);
        if input.wheel_target == Some(id)
            && input.scroll != Vec2::ZERO
            && let Some(pos) = input.pointer_pos
        {
            self.on_scroll.call(ScrollGesture {
                delta: input.scroll,
                pos,
                modifiers: input.modifiers,
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
    pub(crate) fn create_click_catcher(&mut self, cursor: CursorIcon) -> NodeId {
        self.arena.insert(ClickCatcherNode::new(cursor))
    }

    pub(crate) fn capture_pointer(&mut self, captor: NodeId) {
        self.pointer_capture = Some(captor);
        self.touch_scroll_vertical = None;
        self.touch_scroll_horizontal = None;
    }

    pub(crate) fn set_click_catcher_child(&mut self, click_catcher: NodeId, child: NodeId) {
        if self.arena.get_as::<ClickCatcherNode>(click_catcher).child == Some(child) {
            return;
        }
        self.arena
            .get_mut_as::<ClickCatcherNode>(click_catcher)
            .child = Some(child);
    }

    pub(crate) fn set_click_catcher_cursor(&mut self, id: NodeId, cursor: CursorIcon) {
        if self.arena.get_as::<ClickCatcherNode>(id).cursor != cursor {
            self.arena.get_mut_as::<ClickCatcherNode>(id).cursor = cursor;
        }
    }

    pub(crate) fn set_click_catcher_capture_presses(&mut self, id: NodeId, capture_presses: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).capture_presses != capture_presses {
            self.arena
                .get_mut_as::<ClickCatcherNode>(id)
                .capture_presses = capture_presses;
        }
    }

    pub(crate) fn set_click_catcher_repeat_drag(&mut self, id: NodeId, repeat_drag: bool) {
        if self.arena.get_as::<ClickCatcherNode>(id).repeat_drag != repeat_drag {
            self.arena.get_mut_as::<ClickCatcherNode>(id).repeat_drag = repeat_drag;
        }
    }

    pub(crate) fn set_click_catcher_key_active(&mut self, id: NodeId, key_active: bool) {
        if !self.contains(id) {
            return;
        }
        let click_catcher = self.arena.get_mut_as::<ClickCatcherNode>(id);
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

#[component]
pub fn ClickCatcher(
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    #[prop(default = false)] key_active: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = false)] repeat_drag: Prop<bool>,
    on_click: ClickCallback,
    on_click_at: Callback<PointerPress>,
    on_hover_change: Callback<bool>,
    on_hover_move: Callback<PointerPress>,
    on_active_change: Callback<bool>,
    on_press: Callback<PointerPress>,
    on_secondary_press: Callback<PointerPress>,
    on_drag: Callback<PointerPress>,
    on_pan_drag: Callback<Vec2>,
    on_pan_active_change: Callback<bool>,
    on_scroll: Callback<ScrollGesture>,
    on_zoom: Callback<ZoomGesture>,
    capture_at: Callback<Pos2, bool>,
    children: Option<Child>,
) -> NodeId {
    let click_catcher = with_document(|document| {
        let click_catcher = document.create_click_catcher(CursorIcon::Default);
        let node = document.arena.get_mut_as::<ClickCatcherNode>(click_catcher);
        node.on_click = on_click;
        node.on_click_at = on_click_at;
        node.on_hover_change = on_hover_change;
        node.on_hover_move = on_hover_move;
        node.on_active_change = on_active_change;
        node.on_press = on_press;
        node.on_secondary_press = on_secondary_press;
        node.on_drag = on_drag;
        node.on_pan_drag = on_pan_drag;
        node.on_pan_active_change = on_pan_active_change;
        node.on_scroll = on_scroll;
        node.on_zoom = on_zoom;
        node.capture_at = capture_at;
        if let Some(child) = children {
            document.set_click_catcher_child(click_catcher, child);
        }
        click_catcher
    });
    create_effect(move || {
        with_document(|document| document.set_click_catcher_cursor(click_catcher, cursor.get()))
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_capture_presses(click_catcher, capture_presses.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_repeat_drag(click_catcher, repeat_drag.get())
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_click_catcher_key_active(click_catcher, key_active.get())
        })
    });
    click_catcher
}
