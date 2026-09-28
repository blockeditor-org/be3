use std::any::Any;

use crate::base::frame::FrameNode;
use crate::base::overlay::OverlayNode;
use crate::geometry::{Rect, Vec2};
use crate::input::{ImeArea, Key, KeyPress};
use crate::painter::Painter;

use crate::callback::{Callback, ClickCallback};
use crate::current::with_document;
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, Rects};

pub type KeyCallback = Callback<KeyPress, bool>;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImeCursor {
    pub node: NodeId,
    pub rect: Option<Rect>,
}

pub struct FocusableNode {
    pub child: Option<NodeId>,
    pub focused: bool,
    pub tab_stop: bool,
    pub ime: bool,
    pub ime_cursor: Option<ImeCursor>,
    pub on_focus_change: Callback<bool>,
    pub on_activate_change: Callback<bool>,
    pub on_activate: ClickCallback,
    pub on_step: Callback<f32>,
    pub on_text: Callback<String>,
    pub on_preedit: Callback<String>,
    pub on_key: KeyCallback,
    pub on_ancestor_key: KeyCallback,
    pub on_motion: Callback<Vec2>,
}

impl Default for FocusableNode {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusableNode {
    pub fn new() -> Self {
        Self {
            child: None,
            focused: false,
            tab_stop: true,
            ime: false,
            ime_cursor: None,
            on_focus_change: Callback::empty(),
            on_activate_change: Callback::empty(),
            on_activate: ClickCallback::empty(),
            on_step: Callback::empty(),
            on_text: Callback::empty(),
            on_preedit: Callback::empty(),
            on_key: Callback::empty(),
            on_ancestor_key: Callback::empty(),
            on_motion: Callback::empty(),
        }
    }
}

impl Element for FocusableNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
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

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        let hovered = input.pointer_over(rect);
        if hovered
            && ((input.pressed_this_frame && !input.touch_started)
                || (input.touch_ended && !input.touch_dragged && !input.touch_cancelled))
        {
            *focus_target = Some(id);
        }
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "focusable"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_focusable(&mut self) -> NodeId {
        self.arena.insert(FocusableNode::new())
    }

    pub fn set_focusable_child(&mut self, focusable: NodeId, child: NodeId) {
        if self.arena.get_as::<FocusableNode>(focusable).child != Some(child) {
            self.arena.get_mut_as::<FocusableNode>(focusable).child = Some(child);
        }
    }

    pub fn set_focusable_tab_stop(&mut self, focusable: NodeId, tab_stop: bool) {
        if !self.contains(focusable) {
            return;
        }
        if self.arena.get_as::<FocusableNode>(focusable).tab_stop != tab_stop {
            self.arena.touch_mut_as::<FocusableNode>(focusable).tab_stop = tab_stop;
        }
    }

    pub fn focused_node(&self) -> Option<NodeId> {
        self.focused
    }

    pub fn text_focused(&mut self, text: &str) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, text.to_owned(), |node| &node.on_text);
        }
    }

    pub fn preedit_focused(&mut self, text: &str) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, text.to_owned(), |node| &node.on_preedit);
        }
    }

    pub fn motion_focused(&mut self, motion: Vec2) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, motion, |node| &node.on_motion);
        }
    }

    pub fn blur(&mut self) {
        if self.focused.is_none() {
            return;
        }
        crate::current::with_reactive_scope(self, || {
            crate::current::with_document(|document| document.update_focus(None));
        });
    }

    pub fn set_focusable_ime(&mut self, focusable: NodeId, ime: bool) {
        if self.contains(focusable) {
            self.arena.touch_mut_as::<FocusableNode>(focusable).ime = ime;
        }
    }

    pub fn set_focusable_ime_cursor(&mut self, focusable: NodeId, cursor: Option<ImeCursor>) {
        if self.contains(focusable) {
            self.arena
                .touch_mut_as::<FocusableNode>(focusable)
                .ime_cursor = cursor;
        }
    }

    pub fn focused_ime_area(&self) -> Option<ImeArea> {
        let focused = self.focused?;
        let node = self
            .arena
            .get(focused)
            .as_any()
            .downcast_ref::<FocusableNode>()?;
        if !node.ime {
            return None;
        }
        let rect = self.node_rect(focused)?;
        let cursor = node
            .ime_cursor
            .filter(|cursor| self.contains(cursor.node))
            .and_then(|cursor| {
                let anchor = self.node_rect(cursor.node)?;
                Some(match cursor.rect {
                    Some(rect) => rect.translate(anchor.min.to_vec2()),
                    None => anchor,
                })
            })
            .unwrap_or(rect);
        Some(ImeArea { rect, cursor })
    }

    pub fn focus_takes_text(&self) -> bool {
        self.focused.is_some_and(|focused| {
            self.arena
                .get(focused)
                .as_any()
                .downcast_ref::<FocusableNode>()
                .is_some_and(|node| !node.on_text.is_empty())
        })
    }

    pub fn key_focused(&mut self, press: KeyPress) -> bool {
        let Some(focused) = self.focused else {
            return false;
        };
        let Some(on_key) = self
            .arena
            .get(focused)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| node.on_key.clone())
        else {
            return false;
        };
        on_key.call(press)
    }

    pub fn step_focused(&mut self, delta: f32) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, delta, |node| &node.on_step);
        }
    }

    pub fn step_focusable(&mut self, focusable: NodeId, delta: f32) {
        self.call_focusable_handler(focusable, delta, |node| &node.on_step);
    }

    pub fn activate_focusable(&mut self, focusable: NodeId) {
        self.call_focusable_activate(focusable, false, true);
    }

    pub fn focus_focusable(&mut self, focusable: NodeId) {
        self.update_focus(Some(focusable));
    }

    pub fn focusables_within(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.collect_focusables(root, &mut out);
        out
    }

    pub fn focus_is_within(&self, root: NodeId) -> bool {
        self.focused
            .is_some_and(|focused| self.focusables_within(root).contains(&focused))
    }

    pub fn focus_next(&mut self) {
        self.move_focus(1);
    }

    pub fn focus_previous(&mut self) {
        self.move_focus(-1);
    }

    fn move_focus(&mut self, step: isize) {
        let order = self.focusables();
        if order.is_empty() {
            self.update_focus(None);
            return;
        }
        let index = self
            .focused
            .and_then(|focused| order.iter().position(|id| *id == focused));
        let start = index.map_or_else(
            || if step >= 0 { 0 } else { order.len() - 1 },
            |index| (index as isize + step).rem_euclid(order.len() as isize) as usize,
        );
        for offset in 0..order.len() {
            let id = order[(start as isize + offset as isize * step)
                .rem_euclid(order.len() as isize) as usize];
            let element = self.arena.get(id);
            if element
                .as_any()
                .downcast_ref::<FocusableNode>()
                .is_none_or(|node| node.tab_stop)
            {
                self.update_focus(Some(id));
                return;
            }
        }
        self.update_focus(None);
    }

    fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let start = self
            .overlay_stack
            .iter()
            .rev()
            .copied()
            .find(|overlay| self.overlay_traps_focus(*overlay))
            .or(self.root);
        if let Some(start) = start {
            self.collect_focusables(start, &mut out);
        }
        out
    }

    fn collect_focusables(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let element = self.arena.get(id);
        if element
            .as_any()
            .downcast_ref::<FrameNode>()
            .is_some_and(|node| !node.visible)
        {
            return;
        }
        if element
            .as_any()
            .downcast_ref::<OverlayNode>()
            .is_some_and(|node| !node.is_open())
        {
            return;
        }
        if element.as_any().is::<FocusableNode>() {
            out.push(id);
        }
        for child in element.children() {
            self.collect_focusables(child, out);
        }
    }

    pub fn validate_focus(&mut self) {
        if self
            .focused
            .is_some_and(|id| !self.focusables().contains(&id))
        {
            self.update_focus(None);
        }
    }

    pub fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        let old = self.focused;
        self.cancel_focus_activation();
        self.focused = new_focus;
        for id in [old, new_focus].into_iter().flatten() {
            self.arena.invalidate_node(id);
        }
        if let Some(old) = old {
            self.set_focusable_focused(old, false);
        }
        if let Some(new) = self.focused {
            self.set_focusable_focused(new, true);
        }
    }

    pub fn cancel_focus_activation(&mut self) {
        self.activation_key = None;
        if let Some(activated) = self.activated.take() {
            self.call_focusable_activate(activated, false, false);
        }
    }

    pub fn set_focus_key_pressed(&mut self, key: Key, pressed: bool, repeat: bool) {
        if pressed {
            let Some(focused) = self.focused else {
                return;
            };
            if repeat || self.activated.is_some() {
                return;
            }
            self.activated = Some(focused);
            self.activation_key = Some(key);
            self.call_focusable_activate(focused, true, false);
        } else if self.activation_key == Some(key) {
            self.activation_key = None;
            if let Some(activated) = self.activated.take() {
                self.call_focusable_activate(activated, false, self.focused == Some(activated));
            }
        }
    }

    fn call_focusable_activate(&mut self, id: NodeId, pressed: bool, activate: bool) {
        self.call_focusable_handler(id, pressed, |node| &node.on_activate_change);
        if !activate || !self.contains(id) {
            return;
        }
        let on_activate = self
            .arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| node.on_activate.clone());
        if let Some(on_activate) = on_activate {
            on_activate.call();
        }
    }

    fn set_focusable_focused(&mut self, id: NodeId, focused: bool) {
        if !self.contains(id) {
            return;
        }
        if let Some(node) = self
            .arena
            .touch_mut(id)
            .as_any_mut()
            .downcast_mut::<FocusableNode>()
        {
            node.focused = focused;
        }
        self.call_focusable_handler(id, focused, |node| &node.on_focus_change);
    }

    fn call_focusable_handler<V>(
        &mut self,
        id: NodeId,
        value: V,
        select: impl Fn(&FocusableNode) -> &Callback<V>,
    ) {
        if !self.contains(id) {
            return;
        }
        let handler = self
            .arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .map(|node| select(node).clone());
        if let Some(handler) = handler {
            handler.call(value);
        }
    }

    pub fn key_ancestor(&mut self, press: KeyPress) -> bool {
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
            .downcast_ref::<FocusableNode>()
            .is_some_and(|node| !node.on_step.is_empty())
        {
            return false;
        }
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return false;
        }
        for id in path.into_iter().rev().skip(1) {
            let handler = self
                .arena
                .get(id)
                .as_any()
                .downcast_ref::<FocusableNode>()
                .map(|node| node.on_ancestor_key.clone());
            if let Some(handler) = handler
                && handler.call(press)
            {
                return true;
            }
        }
        false
    }

    pub fn focus_path(&self, id: NodeId, focused: NodeId, path: &mut Vec<NodeId>) -> bool {
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

pub fn focus_within(node: NodeId) {
    with_document(|document| {
        let focusable = document
            .focusables_within(node)
            .first()
            .copied()
            .expect("focus_within needs a focusable inside the node it is given");
        document.focus_focusable(focusable);
    });
}
