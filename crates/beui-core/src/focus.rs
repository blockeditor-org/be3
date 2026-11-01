use crate::base::interactive::InteractiveNode;
use crate::base::overlay::OverlayNode;
use beui_types::SoftKeyboard;
use beui_types::{ImeArea, ImeEvent, ImeText, Key, KeyPress, Keyboard};
use beui_types::{PasteKind, Pasted};
use beui_types::{Rect, Vec2};

use crate::current::with_document;
use crate::document::Document;
use crate::node::{Element, FocusChild, NodeOf, Rects};
use beui_tree::callback::{Callback, ClickCallback};
use beui_types::NodeId;
use std::ops::Range;

pub type KeyCallback = Callback<KeyPress, bool>;

const TAB_BUILD_LIMIT: usize = 4096;

enum TabStop {
    Node(NodeId),
    Yielded(NodeId),
    Unbuilt(NodeId, Range<usize>),
}

use beui_types::ImeCursor;

pub struct Focus {
    pub tab_stop: bool,
    pub yields_tab_stop: bool,
    pub ime: bool,
    pub keyboard: SoftKeyboard,
    pub ime_cursor: Option<ImeCursor>,
    pub ime_text: Option<ImeText>,
    pub on_focus_change: Callback<bool>,
    pub on_activate_change: Callback<bool>,
    pub on_activate: ClickCallback,
    pub on_step: Callback<f32>,
    pub on_text: Callback<String>,
    pub pastes: Vec<PasteKind>,
    pub on_paste: Callback<Pasted>,
    pub on_ime: Callback<ImeEvent>,
    pub on_key: KeyCallback,
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
            yields_tab_stop: false,
            ime: false,
            keyboard: SoftKeyboard::OnFocus,
            ime_cursor: None,
            ime_text: None,
            on_focus_change: Callback::empty(),
            on_activate_change: Callback::empty(),
            on_activate: ClickCallback::empty(),
            on_step: Callback::empty(),
            on_text: Callback::empty(),
            pastes: Vec::new(),
            on_paste: Callback::empty(),
            on_ime: Callback::empty(),
            on_key: Callback::empty(),
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

    pub fn paste_kinds(&self) -> Vec<PasteKind> {
        let Some(focus) = self
            .focused
            .and_then(|focused| focus_of(self.arena.get(focused)))
        else {
            return Vec::new();
        };
        match (focus.pastes.is_empty(), focus.on_text.is_empty()) {
            (false, _) => focus.pastes.clone(),
            (true, false) => vec![PasteKind::Text],
            (true, true) => Vec::new(),
        }
    }

    pub fn paste_focused(&mut self, pasted: Pasted) {
        let Some(focused) = self.focused else {
            return;
        };
        let handles = focus_of(self.arena.get(focused)).is_some_and(|focus| {
            focus.pastes.contains(&pasted.kind()) && !focus.on_paste.is_empty()
        });
        match pasted {
            pasted if handles => {
                self.call_focusable_handler(focused, pasted, |node| &node.on_paste);
            }
            Pasted::Text(text) => self.text_focused(&text),
            Pasted::Image(_) => {}
        }
    }

    pub fn set_focusable_pastes(
        &mut self,
        focusable: NodeOf<InteractiveNode>,
        pastes: Vec<PasteKind>,
    ) {
        if self.contains(focusable)
            && focus_of(self.arena.get(focusable)).is_some_and(|focus| focus.pastes != pastes)
        {
            self.focus_mut(focusable).pastes = pastes;
        }
    }

    pub fn ime_focused(&mut self, event: ImeEvent) {
        let Some(focused) = self.focused else {
            return;
        };
        if focus_of(self.arena.get(focused)).is_some_and(|node| !node.on_ime.is_empty()) {
            self.call_focusable_handler(focused, event, |node| &node.on_ime);
            return;
        }
        let typed = match event {
            ImeEvent::SetComposingText(text) => {
                self.composed = text;
                return;
            }
            ImeEvent::CommitText(text) => {
                self.composed.clear();
                text
            }
            ImeEvent::FinishComposing | ImeEvent::Disabled => std::mem::take(&mut self.composed),
            _ => return,
        };
        if !typed.is_empty() {
            self.text_focused(&typed);
        }
    }

    pub fn motion_focused(&mut self, motion: Vec2) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, motion, |node| &node.on_motion);
        }
    }

    pub fn blur(&mut self) {
        self.focus_into = None;
        if self.focused.is_none() {
            return;
        }
        crate::current::with_reactive_scope(self, || {
            crate::current::with_document(|document| document.update_focus(None));
        });
    }

    pub fn set_focusable_ime(&mut self, focusable: NodeOf<InteractiveNode>, ime: bool) {
        if self.contains(focusable)
            && focus_of(self.arena.get(focusable)).is_some_and(|focus| focus.ime != ime)
        {
            self.focus_mut(focusable).ime = ime;
        }
    }

    pub fn set_focusable_keyboard(
        &mut self,
        focusable: NodeOf<InteractiveNode>,
        keyboard: SoftKeyboard,
    ) {
        if self.contains(focusable)
            && focus_of(self.arena.get(focusable)).is_some_and(|focus| focus.keyboard != keyboard)
        {
            self.focus_mut(focusable).keyboard = keyboard;
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

    pub fn set_focusable_ime_text(
        &mut self,
        focusable: NodeOf<InteractiveNode>,
        text: Option<ImeText>,
    ) {
        if self.contains(focusable) {
            self.focus_mut(focusable).ime_text = text;
        }
    }

    pub(crate) fn ime_cursor_in(&self, rects: &Rects, node: NodeId) -> Option<Rect> {
        let cursor = focus_of(self.arena.get(node))?.ime_cursor?;
        match cursor.rect {
            Some(rect) => rects.local_to_window(&cursor.node, rect),
            None => rects.get(&cursor.node),
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
            .and_then(|_| self.ime_cursor_in(&self.rects, focused))
            .unwrap_or(rect);
        Some(ImeArea {
            rect,
            cursor,
            text: node.ime_text.clone(),
            keyboard: match node.keyboard {
                SoftKeyboard::Held => Keyboard::Held,
                _ if self.keyboard_held => Keyboard::Held,
                SoftKeyboard::Shown { asks } => Keyboard::Shown { asks },
                SoftKeyboard::OnFocus | SoftKeyboard::OnPress => Keyboard::Shown {
                    asks: self.keyboard_asks,
                },
            },
        })
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
        let Some(on_key) = focus_of(self.arena.get(focused)).map(|node| node.on_key) else {
            return false;
        };
        on_key.call(press)
    }

    pub(crate) fn target_key(&mut self, node: NodeId, press: KeyPress) -> bool {
        if self.key_focused(press) {
            return true;
        }
        let modifiers = press.modifiers;
        match press.key {
            Key::Enter | Key::Space if !press.pressed => {
                let releases = self.activation_key == Some(press.key);
                if releases {
                    self.set_focus_key_pressed(press.key, false, press.repeat);
                }
                releases
            }
            Key::Enter | Key::Space
                if !modifiers.ctrl && !modifiers.alt && self.activates(node) =>
            {
                self.set_focus_key_pressed(press.key, true, press.repeat);
                true
            }
            Key::ArrowLeft | Key::ArrowDown | Key::ArrowRight | Key::ArrowUp
                if press.pressed && !modifiers.ctrl && !modifiers.alt && self.steps(node) =>
            {
                let delta = match press.key {
                    Key::ArrowLeft | Key::ArrowDown => -1.0,
                    _ => 1.0,
                };
                self.step_focused(delta);
                true
            }
            _ => false,
        }
    }

    pub fn step_focused(&mut self, delta: f32) {
        if let Some(focused) = self.focused {
            self.call_focusable_handler(focused, delta, |node| &node.on_step);
        }
    }

    pub(crate) fn activates(&self, node: NodeId) -> bool {
        self.contains(node)
            && focus_of(self.arena.get(node)).is_some_and(|focus| {
                !focus.on_activate.is_empty() || !focus.on_activate_change.is_empty()
            })
    }

    pub(crate) fn steps(&self, node: NodeId) -> bool {
        self.contains(node)
            && focus_of(self.arena.get(node)).is_some_and(|focus| !focus.on_step.is_empty())
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

    pub fn focus_into(&mut self, root: NodeId) {
        self.focus_into = None;
        match self.focusables_within(root).first().copied() {
            Some(first) => self.update_focus(Some(first)),
            None => self.focus_into = Some((root, self.layout_pass())),
        }
    }

    pub fn focus_pending_within(&self, root: NodeId) -> bool {
        self.focus_into
            .is_some_and(|(into, _)| self.contains(into) && self.is_within(into, root))
    }

    fn settle_focus_into(&mut self) {
        let Some((root, pass)) = self.focus_into else {
            return;
        };
        if !self.contains(root) {
            self.focus_into = None;
            return;
        }
        if let Some(first) = self.focusables_within(root).first().copied() {
            self.focus_into = None;
            self.update_focus(Some(first));
        } else if self.layout_pass() != pass {
            self.focus_into = None;
            if !self.focus_is_within(root) {
                self.update_focus(None);
            }
        }
    }

    pub fn focusables_within(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let blocked = self.blocked_roots(&self.layers());
        self.collect_focusables(root, &mut out, &blocked);
        out
    }

    pub fn focus_is_within(&self, root: NodeId) -> bool {
        self.focused
            .is_some_and(|focused| self.is_within(focused, root))
    }

    pub fn focus_next(&mut self) {
        self.move_focus(1);
    }

    pub fn focus_previous(&mut self) {
        self.move_focus(-1);
    }

    fn move_focus(&mut self, step: isize) {
        let layers = self.layers();
        let blocked = self.blocked_roots(&layers);
        let mut order = Vec::new();
        if let Some(start) = layers[layers.keyboard()].content {
            self.collect_tab_stops(start, &mut order, &blocked);
        }
        if order.is_empty() {
            self.update_focus(None);
            return;
        }
        let index = self.focused.and_then(|focused| {
            order.iter().position(
                |stop| matches!(stop, TabStop::Node(id) | TabStop::Yielded(id) if *id == focused),
            )
        });
        let start = index.map_or_else(
            || if step >= 0 { 0 } else { order.len() - 1 },
            |index| (index as isize + step).rem_euclid(order.len() as isize) as usize,
        );
        for offset in 0..order.len() {
            let stop = &order[(start as isize + offset as isize * step)
                .rem_euclid(order.len() as isize) as usize];
            if let Some(found) = self.tab_stop_at(stop, step, &blocked) {
                self.update_focus(Some(found));
                return;
            }
        }
        self.update_focus(None);
    }

    fn tab_stop_at(&mut self, stop: &TabStop, step: isize, blocked: &[NodeId]) -> Option<NodeId> {
        match stop {
            TabStop::Node(id) => focus_of(self.arena.get(*id))
                .is_none_or(|focus| focus.tab_stop)
                .then_some(*id),
            TabStop::Yielded(_) => None,
            TabStop::Unbuilt(list, rows) => self.build_tab_stop(*list, rows.clone(), step, blocked),
        }
    }

    fn build_tab_stop(
        &mut self,
        list: NodeId,
        rows: Range<usize>,
        step: isize,
        blocked: &[NodeId],
    ) -> Option<NodeId> {
        for at in 0..rows.len().min(TAB_BUILD_LIMIT) {
            let index = match step >= 0 {
                true => rows.start + at,
                false => rows.end - 1 - at,
            };
            let mut element = self.arena.take(list);
            let row = element.build_for_focus(self, list, index);
            self.arena.put_back(list, element);
            let row = row?;
            self.arena.invalidate_node(list);
            let mut stops = Vec::new();
            self.collect_tab_stops(row, &mut stops, blocked);
            if step < 0 {
                stops.reverse();
            }
            for stop in &stops {
                if let Some(found) = self.tab_stop_at(stop, step, blocked) {
                    return Some(found);
                }
            }
            self.arena.get_mut(list).release_for_focus(row);
        }
        None
    }

    pub(crate) fn focusables(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let layers = self.layers();
        let start = layers[layers.keyboard()].content;
        if let Some(start) = start {
            self.collect_focusables(start, &mut out, &self.blocked_roots(&layers));
        }
        out
    }

    fn collect_focusables(&self, id: NodeId, out: &mut Vec<NodeId>, blocked: &[NodeId]) {
        let mut stops = Vec::new();
        self.collect_tab_stops(id, &mut stops, blocked);
        out.extend(stops.into_iter().filter_map(|stop| match stop {
            TabStop::Node(id) | TabStop::Yielded(id) => Some(id),
            TabStop::Unbuilt(..) => None,
        }));
    }

    pub fn tab_stops_within(&self, root: NodeId) -> Vec<NodeId> {
        let mut stops = Vec::new();
        self.collect_tab_stops(root, &mut stops, &self.blocked_roots(&self.layers()));
        stops
            .into_iter()
            .filter_map(|stop| match stop {
                TabStop::Node(id) if self.is_tab_stop(id) => Some(id),
                _ => None,
            })
            .collect()
    }

    fn is_tab_stop(&self, id: NodeId) -> bool {
        focus_of(self.arena.get(id)).is_some_and(|focus| focus.tab_stop)
    }

    fn collect_tab_stops(&self, id: NodeId, out: &mut Vec<TabStop>, blocked: &[NodeId]) {
        if blocked.contains(&id) {
            return;
        }
        let Some(element) = self.arena.try_get(id) else {
            return;
        };
        if element
            .as_any()
            .downcast_ref::<OverlayNode>()
            .is_some_and(|node| !node.is_open())
        {
            return;
        }
        let own = focus_of(element).map(|focus| {
            out.push(TabStop::Node(id));
            (out.len(), focus.yields_tab_stop)
        });
        match element.focus_children() {
            Some(children) => {
                for child in children {
                    match child {
                        FocusChild::Node(child) => self.collect_tab_stops(child, out, blocked),
                        FocusChild::Unbuilt(rows) => out.push(TabStop::Unbuilt(id, rows)),
                    }
                }
            }
            None => {
                element.for_each_child(&mut |child| self.collect_tab_stops(child, out, blocked))
            }
        }
        if let Some((within, true)) = own
            && out[within..]
                .iter()
                .any(|stop| matches!(stop, TabStop::Node(stop) if self.is_tab_stop(*stop)))
        {
            out[within - 1] = TabStop::Yielded(id);
        }
    }

    pub fn validate_focus(&mut self) {
        let dirty = std::mem::take(&mut self.focus_dirty)
            | self.arena.take_reparented()
            | self.focus_into.is_some();
        if !dirty {
            return;
        }
        if let Some(wanted) = self.focus_wanted.take()
            && self.focus_valid(wanted)
        {
            self.update_focus(Some(wanted));
        }
        if let Some(focused) = self.focused
            && !self.focus_valid(focused)
        {
            self.update_focus(None);
        }
        self.settle_focus_into();
        self.watch_focus();
    }

    fn watch_focus(&mut self) {
        let mut path = Vec::new();
        let mut current = self.focused;
        while let Some(node) = current
            && path.len() < crate::node::ANCESTOR_LIMIT
        {
            path.push(node);
            if Some(node) == self.root {
                break;
            }
            current = self.arena.parent(node);
        }
        self.arena.watch(path);
    }

    pub(crate) fn focus_valid(&self, node: NodeId) -> bool {
        if !self.contains(node) || focus_of(self.arena.get(node)).is_none() {
            return false;
        }
        if !self.live(node) {
            return false;
        }
        let layers = self.layers();
        let Some(layer) = self.layer_of(&layers, node) else {
            return false;
        };
        layer >= layers.keyboard() && !self.focus_blocked(&layers, node)
    }

    pub fn update_focus(&mut self, new_focus: Option<NodeId>) {
        if new_focus == self.focused {
            return;
        }
        let layers = self.layers();
        let keyboard = layers.keyboard();
        match new_focus {
            Some(new) => {
                if self
                    .layer_of(&layers, new)
                    .is_some_and(|layer| layer < keyboard)
                {
                    if !layers.locked() {
                        self.focus_wanted = Some(new);
                        self.focus_dirty = true;
                    }
                    return;
                }
                if let Some(redirect) = self.blocked_focus_redirect(&layers, new) {
                    if redirect.is_some() && redirect != self.focused {
                        self.update_focus(redirect);
                    }
                    return;
                }
            }
            None => {
                let held = layers.locked()
                    && self.focused.is_some_and(|old| {
                        self.layer_of(&layers, old)
                            .is_some_and(|layer| layers.within(layer, keyboard))
                            && self.focus_valid(old)
                    });
                if held {
                    return;
                }
            }
        }
        if new_focus.is_some() {
            self.focus_into = None;
        }
        let old = self.focused;
        self.composed.clear();
        self.cancel_focus_activation();
        self.set_focused(new_focus);
        self.focus_before = old;
        if let Some(new) = new_focus {
            self.note_focus_outside_scopes(new);
        }
        self.watch_focus();
        self.keyboard_held = self.touched()
            && new_focus.is_some_and(|focused| {
                self.contains(focused)
                    && focus_of(self.arena.get(focused))
                        .is_some_and(|node| node.keyboard == SoftKeyboard::OnPress)
            });
        for id in [old, new_focus].into_iter().flatten() {
            self.arena.invalidate_node(id);
        }
        if let Some(culled) = new_focus.and_then(|id| self.culled_ancestor(id)) {
            self.arena.invalidate_node(culled);
        }
        if let Some(old) = old {
            self.set_focusable_focused(old, false);
        }
        if let Some(new) = self.focused {
            self.set_focusable_focused(new, true);
        }
    }

    pub fn press_focus(&mut self, target: Option<NodeId>) {
        self.update_focus(target);
        if let Some(target) = target
            && self.focused == Some(target)
            && self.contains(target)
            && crate::interact::hit::interactive(self, target).is_some_and(|node| node.press_focus)
            && let Some(keyboard) = focus_of(self.arena.get(target)).map(|node| {
                node.ime && matches!(node.keyboard, SoftKeyboard::OnFocus | SoftKeyboard::OnPress)
            })
        {
            if keyboard {
                self.keyboard_asks = self.keyboard_asks.wrapping_add(1);
            }
            self.keyboard_held = false;
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
        let on_activate = focus_of(self.arena.get(id)).map(|node| node.on_activate);
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

    fn call_focusable_handler<V: 'static>(
        &mut self,
        id: NodeId,
        value: V,
        select: impl Fn(&Focus) -> &Callback<V>,
    ) {
        if !self.contains(id) {
            return;
        }
        let handler = focus_of(self.arena.get(id)).map(|node| *select(node));
        if let Some(handler) = handler {
            handler.call(value);
        }
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
