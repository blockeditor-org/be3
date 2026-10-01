use crate::base::frame::FrameNode;
use crate::base::interactive::InteractiveNode;
use crate::base::overlay::OverlayNode;
use crate::geometry::{Rect, Vec2};
use crate::input::{ImeArea, Key, KeyPress};

use crate::callback::{Callback, ClickCallback};
use crate::current::with_document;
use crate::document::Document;
use crate::node::{Element, NodeId, NodeOf};

pub type KeyCallback = Callback<KeyPress, bool>;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImeCursor {
    pub node: NodeId,
    pub rect: Option<Rect>,
}

pub struct Focus {
    pub tab_stop: bool,
    pub press_focus: bool,
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

impl Default for Focus {
    fn default() -> Self {
        Self::new()
    }
}

impl Focus {
    pub fn new() -> Self {
        Self {
            tab_stop: true,
            press_focus: true,
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

pub fn focus_of(element: &dyn Element) -> Option<&Focus> {
    element
        .as_any()
        .downcast_ref::<InteractiveNode>()?
        .focus
        .as_ref()
}

impl Document {
    pub fn set_focusable_tab_stop(&mut self, focusable: NodeOf<InteractiveNode>, tab_stop: bool) {
        if !self.contains(focusable) {
            return;
        }
        if focus_of(self.arena.get(focusable)).is_some_and(|focus| focus.tab_stop != tab_stop) {
            self.focus_mut(focusable).tab_stop = tab_stop;
        }
    }

    pub fn set_focusable_press_focus(
        &mut self,
        focusable: NodeOf<InteractiveNode>,
        press_focus: bool,
    ) {
        if !self.contains(focusable) {
            return;
        }
        if focus_of(self.arena.get(focusable)).is_some_and(|focus| focus.press_focus != press_focus)
        {
            self.focus_mut(focusable).press_focus = press_focus;
        }
    }

    fn focus_mut(&mut self, id: NodeOf<InteractiveNode>) -> &mut Focus {
        self.arena
            .touch_mut_as::<InteractiveNode>(id)
            .focus
            .as_mut()
            .expect("the node is focusable")
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

    pub fn set_focusable_ime(&mut self, focusable: NodeOf<InteractiveNode>, ime: bool) {
        if self.contains(focusable) {
            self.focus_mut(focusable).ime = ime;
        }
    }

    pub fn set_focusable_ime_cursor(
        &mut self,
        focusable: NodeOf<InteractiveNode>,
        cursor: Option<ImeCursor>,
    ) {
        if self.contains(focusable) {
            self.focus_mut(focusable).ime_cursor = cursor;
        }
    }

    pub fn focused_ime_area(&self) -> Option<ImeArea> {
        let focused = self.focused?;
        let node = focus_of(self.arena.get(focused))?;
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

    pub fn focus_types(&self) -> bool {
        self.focused.is_some_and(|focused| {
            focus_of(self.arena.get(focused))
                .is_some_and(|node| node.ime && !node.on_text.is_empty())
        })
    }

    pub fn focus_takes_text(&self) -> bool {
        self.focused.is_some_and(|focused| {
            focus_of(self.arena.get(focused)).is_some_and(|node| !node.on_text.is_empty())
        })
    }

    pub fn key_focused(&mut self, press: KeyPress) -> bool {
        let Some(focused) = self.focused else {
            return false;
        };
        let Some(on_key) = focus_of(self.arena.get(focused)).map(|node| node.on_key.clone()) else {
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
            if focus_of(self.arena.get(id)).is_none_or(|focus| focus.tab_stop) {
                self.update_focus(Some(id));
                return;
            }
        }
        self.update_focus(None);
    }

    pub(crate) fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let start = self
            .overlay_stack
            .iter()
            .rev()
            .copied()
            .find(|overlay| self.overlay_traps_focus(*overlay))
            .map(NodeOf::id)
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
        if focus_of(element).is_some() {
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
        let on_activate = focus_of(self.arena.get(id)).map(|node| node.on_activate.clone());
        if let Some(on_activate) = on_activate {
            on_activate.call();
        }
    }

    fn set_focusable_focused(&mut self, id: NodeId, focused: bool) {
        if !self.contains(id) {
            return;
        }
        self.arena.touch_mut(id);
        self.call_focusable_handler(id, focused, |node| &node.on_focus_change);
    }

    fn call_focusable_handler<V>(
        &mut self,
        id: NodeId,
        value: V,
        select: impl Fn(&Focus) -> &Callback<V>,
    ) {
        if !self.contains(id) {
            return;
        }
        let handler = focus_of(self.arena.get(id)).map(|node| select(node).clone());
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
        if focus_of(self.arena.get(focused)).is_some_and(|node| !node.on_step.is_empty()) {
            return false;
        }
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return false;
        }
        for id in path.into_iter().rev().skip(1) {
            let handler = focus_of(self.arena.get(id)).map(|node| node.on_ancestor_key.clone());
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

pub fn focus_within(node: NodeId) -> bool {
    with_document(|document| {
        let Some(focusable) = document.focusables_within(node).first().copied() else {
            return false;
        };
        document.focus_focusable(focusable);
        true
    })
}
