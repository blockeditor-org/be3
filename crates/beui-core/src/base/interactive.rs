use crate::window::WindowId;
use std::any::Any;

use crate::base::list::Direction;
use crate::base::overlay::OverlayNode;
use crate::focus::Focus;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{CursorIcon, Modifiers, PointerInput, PointerPress, TouchDrags, WheelInput};
use crate::painter::Painter;
use beui_types::region::{Area, Claim};

use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::node::{Element, NodeId, NodeOf, Rects};

pub struct InteractiveNode {
    pub child: Option<NodeId>,
    pub focus: Option<Focus>,
    pub cursor: Option<CursorIcon>,
    pub locks_pointer: bool,
    pub grabs_pointer: bool,
    pub captures: Area,
    pub press_focus: bool,
    pub repeat_drag: bool,
    pub touch_drags: TouchDrags,
    pub claims_touch: bool,
    pub drawing: Area,
    pub every_sample: bool,
    pub key_active: bool,
    pub armed: bool,
    pub hovered: bool,
    pub hover_pos: Option<Pos2>,
    pub active: bool,
    pub on_click: ClickCallback,
    pub on_click_at: Callback<PointerPress>,
    pub on_hover_change: Callback<bool>,
    pub on_hover_move: Callback<PointerPress>,
    pub on_active_change: Callback<bool>,
    pub on_cancel: ClickCallback,
    pub on_middle_click: ClickCallback,
    pub on_press: Callback<PointerPress>,
    pub on_secondary_press: Callback<PointerPress>,
    pub on_drag: Callback<PointerPress>,
    pub on_pointer: Callback<PointerInput, bool>,
    pub on_wheel: Callback<WheelInput, bool>,
    pub gesture_area: Area,
    pub hit_area: Area,
    pub on_forward: Callback<crate::interact::forward::ForwardedInput>,
    pub inhibits_shortcuts: bool,
    pub clipboard_keys: bool,
    pub forwards_wheel: bool,
    pub claims: Vec<Claim>,
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
            grabs_pointer: false,
            captures: Area::Nowhere,
            press_focus: true,
            repeat_drag: false,
            touch_drags: TouchDrags::None,
            claims_touch: true,
            drawing: Area::Nowhere,
            every_sample: false,
            key_active: false,
            armed: false,
            hovered: false,
            hover_pos: None,
            active: false,
            on_click: ClickCallback::empty(),
            on_click_at: Callback::empty(),
            on_hover_change: Callback::empty(),
            on_hover_move: Callback::empty(),
            on_active_change: Callback::empty(),
            on_cancel: ClickCallback::empty(),
            on_middle_click: ClickCallback::empty(),
            on_press: Callback::empty(),
            on_secondary_press: Callback::empty(),
            on_drag: Callback::empty(),
            on_pointer: Callback::empty(),
            on_wheel: Callback::empty(),
            gesture_area: Area::Nowhere,
            hit_area: Area::Whole,
            on_forward: Callback::empty(),
            inhibits_shortcuts: false,
            clipboard_keys: false,
            forwards_wheel: true,
            claims: Vec::new(),
            on_press_outside: Callback::empty(),
        }
    }

    pub fn forwards(&self) -> bool {
        !self.on_forward.is_empty()
    }

    pub fn forwards_wheels(&self) -> bool {
        self.forwards() && self.forwards_wheel
    }

    pub fn takes_presses(&self) -> bool {
        self.captures == Area::Whole
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
            || !matches!(self.captures, Area::Nowhere | Area::Whole)
            || !self.claims.is_empty()
    }

    pub fn takes_secondary(&self) -> bool {
        !self.on_secondary_press.is_empty()
    }

    pub fn hears_pointers(&self) -> bool {
        !self.on_pointer.is_empty()
    }

    pub fn claims_touches(&self) -> bool {
        self.claims_touch && self.takes_presses()
    }

    pub fn stops_middle(&self) -> bool {
        !self.on_middle_click.is_empty() || self.forwards()
    }

    pub fn holds_drag(&self, direction: Direction) -> bool {
        self.touch_drags.holds(direction)
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
            (self.on_middle_click.is_empty(), "middle click"),
            (self.on_cancel.is_empty(), "cancel"),
            (self.on_hover_change.is_empty(), "hover"),
            (self.on_hover_move.is_empty(), "hover move"),
            (self.on_active_change.is_empty(), "active"),
            (self.on_drag.is_empty(), "drag"),
            (self.on_pointer.is_empty(), "pointer"),
            (self.on_wheel.is_empty(), "wheel"),
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
            (self.grabs_pointer, "grabbing the pointer"),
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
                    (
                        focus.keyboard != beui_types::input::SoftKeyboard::OnPress,
                        "keyboard",
                    ),
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

    pub fn set_interactive_captures(&mut self, id: NodeOf<InteractiveNode>, captures: Area) {
        if self.arena.get_as::<InteractiveNode>(id).captures != captures {
            self.arena.touch_mut_as::<InteractiveNode>(id).captures = captures;
        }
    }

    pub fn set_interactive_hit_area(&mut self, id: NodeOf<InteractiveNode>, area: Area) {
        if self.arena.get_as::<InteractiveNode>(id).hit_area != area {
            self.arena.touch_mut_as::<InteractiveNode>(id).hit_area = area;
        }
    }

    pub fn set_interactive_gesture_area(&mut self, id: NodeOf<InteractiveNode>, area: Area) {
        if self.arena.get_as::<InteractiveNode>(id).gesture_area != area {
            self.arena.touch_mut_as::<InteractiveNode>(id).gesture_area = area;
        }
    }

    pub fn set_interactive_press_focus(&mut self, id: NodeOf<InteractiveNode>, press_focus: bool) {
        if self.arena.get_as::<InteractiveNode>(id).press_focus != press_focus {
            self.arena.touch_mut_as::<InteractiveNode>(id).press_focus = press_focus;
        }
    }

    pub fn set_interactive_grabs_pointer(&mut self, id: NodeOf<InteractiveNode>, grabs: bool) {
        if self.arena.get_as::<InteractiveNode>(id).grabs_pointer == grabs {
            return;
        }
        self.arena.touch_mut_as::<InteractiveNode>(id).grabs_pointer = grabs;
        match grabs {
            true => self.pointers.grabber = Some(id.id()),
            false if self.pointers.grabber == Some(id.id()) => self.pointers.grabber = None,
            false => {}
        }
    }

    pub fn set_interactive_touch_drags(&mut self, id: NodeOf<InteractiveNode>, drags: TouchDrags) {
        if self.arena.get_as::<InteractiveNode>(id).touch_drags != drags {
            self.arena.touch_mut_as::<InteractiveNode>(id).touch_drags = drags;
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

    pub fn set_interactive_drawing(&mut self, id: NodeOf<InteractiveNode>, drawing: Area) {
        if self.arena.get_as::<InteractiveNode>(id).drawing == drawing {
            return;
        }
        match drawing {
            Area::Nowhere => self.drawing_surfaces.remove(&id.id()),
            _ => self.drawing_surfaces.insert(id.id()),
        };
        self.arena.touch_mut_as::<InteractiveNode>(id).drawing = drawing;
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

    pub fn wants_every_sample(&self, window: WindowId) -> bool {
        self.every_sample_surfaces
            .iter()
            .any(|id| self.arena.contains(*id) && self.window_of(*id) == window)
    }

    pub fn drawing_rects(&self, window: WindowId) -> Vec<Rect> {
        self.drawing_surfaces
            .iter()
            .filter(|id| self.arena.contains(**id) && self.window_of(**id) == window)
            .filter_map(|id| {
                let node = self
                    .arena
                    .get(*id)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()?;
                Some(self.area_rects(*id, &node.drawing))
            })
            .flatten()
            .filter(|rect| rect.is_positive())
            .collect()
    }

    pub fn area_rects(&self, id: NodeId, area: &Area) -> Vec<Rect> {
        match area {
            Area::Nowhere | Area::CaretHandles => Vec::new(),
            Area::Whole => self.rects.visible(&id).into_iter().collect(),
            Area::Within(region) => {
                self.rects
                    .scaled(&id)
                    .map_or_else(Vec::new, |(rect, scale)| {
                        region
                            .rects()
                            .iter()
                            .map(|local| {
                                Rect::from_min_max(
                                    rect.min + local.min.to_vec2() * scale,
                                    rect.min + local.max.to_vec2() * scale,
                                )
                            })
                            .collect()
                    })
            }
        }
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

    pub fn set_interactive_claims(&mut self, id: NodeOf<InteractiveNode>, claims: Vec<Claim>) {
        if self.arena.get_as::<InteractiveNode>(id).claims == claims {
            return;
        }
        match claims.is_empty() {
            true => self.press_claimants.remove(&id.id()),
            false => self.press_claimants.insert(id.id()),
        };
        self.arena.touch_mut_as::<InteractiveNode>(id).claims = claims;
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

    pub fn press_claims(&self, window: WindowId) -> Vec<(Modifiers, Rect)> {
        self.press_claimants
            .iter()
            .filter(|id| self.arena.contains(**id) && self.window_of(**id) == window)
            .filter_map(|id| {
                let node = self
                    .arena
                    .get(*id)
                    .as_any()
                    .downcast_ref::<InteractiveNode>()?;
                Some(
                    node.claims
                        .iter()
                        .filter(|claim| claim.modifiers.any())
                        .flat_map(|claim| {
                            self.area_rects(*id, &claim.area)
                                .into_iter()
                                .map(|rect| (claim.modifiers, rect))
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .filter(|(_, rect)| rect.is_positive())
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
