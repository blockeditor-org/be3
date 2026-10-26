use std::any::Any;

use crate::base::focus::Focus;
use crate::base::list::Direction;
use crate::base::overlay::OverlayNode;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{
    AutoscrollGesture, CursorIcon, DragGesture, Modifiers, PointerPress, ScrollGesture,
    SecondaryDrag, ZoomGesture,
};
use crate::painter::Painter;

use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::node::{Element, NodeId, NodeOf, Rects};

pub struct InteractiveNode {
    pub child: Option<NodeId>,
    pub focus: Option<Focus>,
    pub cursor: Option<CursorIcon>,
    pub locks_pointer: bool,
    pub scroll_axis: Option<Direction>,
    pub capture_presses: bool,
    pub press_focus: bool,
    pub repeat_drag: bool,
    pub touch_drags: bool,
    pub touch_drag_axis: Option<Direction>,
    pub claims_touch: bool,
    pub drawing: bool,
    pub every_sample: bool,
    pub key_active: bool,
    pub armed: bool,
    pub hovered: bool,
    pub hover_pos: Option<Pos2>,
    pub active: bool,
    pub pan_active: bool,
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
    pub gestures_at: Callback<Pos2, bool>,
    pub hit_at: Callback<Pos2, bool>,
    pub drawing_at: Callback<Pos2, bool>,
    pub on_forward: Callback<crate::interact::forward::ForwardedInput>,
    pub inhibits_shortcuts: bool,
    pub clipboard_keys: bool,
    pub forwards_wheel: bool,
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
    pub fn new() -> Self {
        Self {
            child: None,
            focus: None,
            cursor: None,
            locks_pointer: false,
            scroll_axis: None,
            capture_presses: false,
            press_focus: true,
            repeat_drag: false,
            touch_drags: false,
            touch_drag_axis: None,
            claims_touch: true,
            drawing: false,
            every_sample: false,
            key_active: false,
            armed: false,
            hovered: false,
            hover_pos: None,
            active: false,
            pan_active: false,
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
            gestures_at: Callback::empty(),
            hit_at: Callback::empty(),
            drawing_at: Callback::empty(),
            on_forward: Callback::empty(),
            inhibits_shortcuts: false,
            clipboard_keys: false,
            forwards_wheel: true,
            claim_modifiers: None,
            claim_at: Callback::empty(),
            on_press_outside: Callback::empty(),
        }
    }

    pub fn hits(&self, pos: Pos2, rect: Rect, scale: f32) -> bool {
        self.hit_at.is_empty() || self.hit_at.call(local_in(rect, scale, pos))
    }

    pub fn claims(&self, pos: Pos2, rect: Rect, held: Modifiers) -> bool {
        let whole = self.claim_modifiers.is_some_and(|claimed| {
            claimed.any() && held.holds(claimed) && rect.contains_half_open(pos)
        });
        whole || (!self.claim_at.is_empty() && self.claim_at.call((pos, held)))
    }

    pub fn captures(&self, pos: Pos2, rect: Rect) -> bool {
        (self.capture_presses && rect.contains_half_open(pos))
            || (!self.capture_at.is_empty() && self.capture_at.call(pos))
    }

    pub fn draws_at(&self, pos: Pos2, rect: Rect) -> bool {
        (self.drawing && rect.contains_half_open(pos))
            || (!self.drawing_at.is_empty() && self.drawing_at.call(pos))
    }

    pub fn takes_gestures_at(&self, pos: Pos2) -> bool {
        !self.gestures_at.is_empty() && self.gestures_at.call(pos)
    }

    pub fn forwards(&self) -> bool {
        !self.on_forward.is_empty()
    }

    pub fn forwards_wheels(&self) -> bool {
        self.forwards() && self.forwards_wheel
    }

    pub fn takes_presses(&self) -> bool {
        self.capture_presses
            || !self.on_click.is_empty()
            || !self.on_click_at.is_empty()
            || !self.on_press.is_empty()
            || !self.on_secondary_press.is_empty()
    }

    pub fn holds_presses(&self) -> bool {
        self.takes_presses()
            || !self.on_drag.is_empty()
            || !self.on_cancel.is_empty()
            || !self.on_active_change.is_empty()
            || !self.capture_at.is_empty()
            || !self.claim_at.is_empty()
            || self.claim_modifiers.is_some()
    }

    pub fn takes_secondary(&self) -> bool {
        !self.on_secondary_press.is_empty() || !self.on_secondary_drag.is_empty()
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
        self.wants_autoscroll()
            || !self.on_pan_drag.is_empty()
            || !self.on_middle_click.is_empty()
            || self.forwards()
    }

    pub fn wants_wheel(&self, wheel: Vec2) -> bool {
        !self.on_scroll.is_empty() && self.along(wheel)
    }

    pub fn catches_drag(&self, direction: Direction) -> bool {
        self.holds_drag(direction)
            || (!self.on_scroll_drag.is_empty()
                && self.scroll_axis.is_none_or(|axis| axis == direction))
    }

    pub fn holds_drag(&self, direction: Direction) -> bool {
        self.touch_drags || self.touch_drag_axis == Some(direction)
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

    pub(crate) fn arm(&mut self, armed: bool) -> Option<(Callback<bool>, bool)> {
        self.armed = armed;
        self.report_active()
    }

    fn report_active(&mut self) -> Option<(Callback<bool>, bool)> {
        let active = self.is_active();
        if active == self.active {
            return None;
        }
        self.active = active;
        Some((self.on_active_change, active))
    }
}

pub fn press_at(
    rect: Rect,
    scale: f32,
    pos: Pos2,
    clicks: u32,
    modifiers: Modifiers,
    touch: bool,
    time: std::time::Instant,
) -> PointerPress {
    PointerPress {
        window: crate::window::WindowId::MAIN,
        pos,
        local: local_in(rect, scale, pos),
        size: rect.size() / scale,
        fraction: fraction(rect, pos),
        clicks,
        modifiers,
        touch,
        pen: None,
        predicted: None,
        time,
    }
}

pub fn local_in(rect: Rect, scale: f32, pos: Pos2) -> Pos2 {
    let local = (pos - rect.min) / scale;
    Pos2::new(local.x, local.y)
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
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
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

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        if let Some(child) = self.child {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
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
                    (focus.yields_tab_stop, "yields tab stop"),
                    (self.press_focus, "press"),
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

    pub fn set_interactive_locks_pointer(&mut self, id: NodeOf<InteractiveNode>, locks: bool) {
        if self.arena.get_as::<InteractiveNode>(id).locks_pointer == locks {
            return;
        }
        self.arena.touch_mut_as::<InteractiveNode>(id).locks_pointer = locks;
        if locks {
            self.pointer_lockers.borrow_mut().push(id);
        }
        self.publish_pointer_lock(false);
    }

    pub(crate) fn publish_pointer_lock(&self, reattached: bool) {
        let mut lockers = self.pointer_lockers.borrow_mut();
        lockers.retain(|locker| {
            self.arena
                .kind_of::<InteractiveNode>(locker.id())
                .is_some_and(|locker| self.arena.get_as(locker).locks_pointer)
        });
        let locked = !lockers.is_empty();
        let changed = self.pointer_locked.replace(locked) != locked;
        if (changed || (reattached && locked))
            && let Some(context) = self.context()
        {
            context.set_pointer_locked(locked);
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

    pub fn set_interactive_press_focus(&mut self, id: NodeOf<InteractiveNode>, press_focus: bool) {
        if self.arena.get_as::<InteractiveNode>(id).press_focus != press_focus {
            self.arena.touch_mut_as::<InteractiveNode>(id).press_focus = press_focus;
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
            self.arena.touch_mut_as::<InteractiveNode>(id).touch_drags = touch_drags;
        }
    }

    pub fn set_interactive_touch_drag_axis(
        &mut self,
        id: NodeOf<InteractiveNode>,
        axis: Option<Direction>,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).touch_drag_axis != axis {
            self.arena
                .touch_mut_as::<InteractiveNode>(id)
                .touch_drag_axis = axis;
        }
    }

    pub fn set_interactive_claims_touch(
        &mut self,
        id: NodeOf<InteractiveNode>,
        claims_touch: bool,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).claims_touch != claims_touch {
            self.arena.touch_mut_as::<InteractiveNode>(id).claims_touch = claims_touch;
        }
    }

    pub fn set_interactive_drawing(&mut self, id: NodeOf<InteractiveNode>, drawing: bool) {
        if self.arena.get_as::<InteractiveNode>(id).drawing == drawing {
            return;
        }
        self.arena.touch_mut_as::<InteractiveNode>(id).drawing = drawing;
        match drawing {
            true => self.drawing_surfaces.insert(id.id()),
            false => self.drawing_surfaces.remove(&id.id()),
        };
    }

    pub fn set_interactive_every_sample(&mut self, id: NodeOf<InteractiveNode>, every: bool) {
        if self.arena.get_as::<InteractiveNode>(id).every_sample == every {
            return;
        }
        self.arena.get_mut_as::<InteractiveNode>(id).every_sample = every;
        match every {
            true => self.every_sample_surfaces.insert(id.id()),
            false => self.every_sample_surfaces.remove(&id.id()),
        };
    }

    pub fn wants_every_sample(&self) -> bool {
        self.every_sample_surfaces
            .iter()
            .any(|id| self.arena.contains(*id))
    }

    pub fn drawing_rects(&self) -> Vec<Rect> {
        self.drawing_surfaces
            .iter()
            .filter(|id| self.arena.contains(**id))
            .filter_map(|id| self.rects.visible(id))
            .filter(|rect| rect.is_positive())
            .collect()
    }

    pub fn set_interactive_inhibits_shortcuts(
        &mut self,
        id: NodeOf<InteractiveNode>,
        inhibits: bool,
    ) {
        if self.arena.get_as::<InteractiveNode>(id).inhibits_shortcuts != inhibits {
            self.arena
                .touch_mut_as::<InteractiveNode>(id)
                .inhibits_shortcuts = inhibits;
        }
    }

    pub fn set_interactive_clipboard_keys(&mut self, id: NodeOf<InteractiveNode>, keys: bool) {
        if self.arena.get_as::<InteractiveNode>(id).clipboard_keys != keys {
            self.arena
                .touch_mut_as::<InteractiveNode>(id)
                .clipboard_keys = keys;
        }
    }

    pub fn clipboard_keys(&self) -> bool {
        let mut current = self.focused.filter(|focused| self.arena.contains(*focused));
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            let Some(id) = current else {
                return false;
            };
            let claims = self
                .arena
                .get(id)
                .as_any()
                .downcast_ref::<InteractiveNode>()
                .is_some_and(|node| node.clipboard_keys);
            if claims {
                return true;
            }
            current = self.arena.parent(id);
        }
        false
    }

    pub fn set_interactive_forwards_wheel(&mut self, id: NodeOf<InteractiveNode>, forwards: bool) {
        if self.arena.get_as::<InteractiveNode>(id).forwards_wheel != forwards {
            self.arena
                .touch_mut_as::<InteractiveNode>(id)
                .forwards_wheel = forwards;
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
        let node = self.arena.touch_mut_as::<InteractiveNode>(id);
        node.claim_modifiers = claimed;
        let claims = claimed.is_some() || !node.claim_at.is_empty();
        match claims {
            true => self.press_claimants.insert(id.id()),
            false => self.press_claimants.remove(&id.id()),
        };
    }

    pub fn set_interactive_claim_at(
        &mut self,
        id: NodeOf<InteractiveNode>,
        claim_at: Callback<(Pos2, Modifiers), bool>,
    ) {
        let node = self.arena.touch_mut_as::<InteractiveNode>(id);
        node.claim_at = claim_at;
        let claims = !node.claim_at.is_empty() || node.claim_modifiers.is_some();
        match claims {
            true => self.press_claimants.insert(id.id()),
            false => self.press_claimants.remove(&id.id()),
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
        self.arena
            .touch_mut_as::<InteractiveNode>(id)
            .on_press_outside = on_press_outside;
    }

    pub(crate) fn pressed_outside(
        &mut self,
        rects: &Rects,
        layers: &[NodeOf<OverlayNode>],
        window: crate::window::WindowId,
        pos: Pos2,
        primary: bool,
    ) {
        self.press_outside(layers);
        if !primary {
            return;
        }
        let outside: Vec<Callback<Pos2>> = self
            .outside_watchers
            .iter()
            .filter(|id| self.arena.contains(**id))
            .filter(|id| {
                self.window_of(**id) != window
                    || !rects
                        .visible(id)
                        .is_some_and(|rect| rect.contains_half_open(pos))
            })
            .filter_map(|id| {
                self.arena
                    .get(*id)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()
                    .map(|node| node.on_press_outside)
            })
            .collect();
        for callback in outside {
            callback.call(pos);
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
        if !self.contains(id) || self.arena.get_as::<InteractiveNode>(id).key_active == key_active {
            return;
        }
        let interactive = self.arena.touch_mut_as::<InteractiveNode>(id);
        interactive.key_active = key_active;
        let active = interactive.is_active();
        if active == interactive.active {
            return;
        }
        interactive.active = active;
        let on_active_change = interactive.on_active_change;
        on_active_change.call(active);
    }
}
