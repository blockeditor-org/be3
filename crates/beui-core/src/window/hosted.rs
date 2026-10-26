use std::rc::Rc;

use foldhash::{HashMap, HashSet};
use reactive::{Get, Set};

use crate::base::overlay::OverlayNode;
use crate::context::{Context, Moved};
use crate::damage::{Region, Taken};
use crate::display::Display;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::layer::Layers;
use crate::node::{ANCESTOR_LIMIT, NodeId, NodeOf};

use super::{WindowId, WindowReport, WindowRequest};

pub(crate) type Painting = Vec<(Rc<Display>, Vec2)>;

pub(crate) struct Windows {
    nodes: Vec<(WindowId, NodeOf<OverlayNode>)>,
    opens: ::reactive::OwnedSignal<bool>,
    placed: ::reactive::OwnedSignal<bool>,
    asked: Vec<WindowId>,
    wanted: HashMap<WindowId, Vec2>,
    closed_by_host: HashSet<WindowId>,
    requests: Vec<WindowRequest>,
    shown: HashMap<WindowId, Vec2>,
    pub(crate) paintings: Vec<(WindowId, Painting)>,
    pub(crate) viewport: Option<(WindowId, Rect)>,
    kept_focus: HashMap<WindowId, NodeId>,
    host_focused: Option<WindowId>,
    pub(crate) focus_resized: bool,
    pub(crate) appearances: HashMap<WindowId, crate::window::Appearance>,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            opens: ::reactive::OwnedSignal::new(false),
            placed: ::reactive::OwnedSignal::new(false),
            asked: Vec::new(),
            wanted: HashMap::default(),
            closed_by_host: HashSet::default(),
            requests: Vec::new(),
            shown: HashMap::default(),
            paintings: Vec::new(),
            viewport: None,
            kept_focus: HashMap::default(),
            host_focused: None,
            focus_resized: false,
            appearances: HashMap::default(),
        }
    }
}

impl Windows {
    pub(crate) fn note(&mut self, id: WindowId, overlay: NodeOf<OverlayNode>) {
        self.nodes
            .retain(|(held, node)| *held != id && *node != overlay);
        self.nodes.push((id, overlay));
    }
}

impl Document {
    pub fn opens_windows(&self) -> bool {
        self.windows.opens.get_untracked() || self.windows.placed.get_untracked()
    }

    pub fn watch_opens_windows(&self) -> ::reactive::ReadSignal<bool> {
        self.windows.opens.read_only()
    }

    pub fn watch_placed_windows(&self) -> ::reactive::ReadSignal<bool> {
        self.windows.placed.read_only()
    }

    pub(crate) fn hosts_window(&self, spec: &crate::base::overlay::WindowSpec) -> bool {
        self.windows.opens.get_untracked()
            || (self.windows.placed.get_untracked()
                && matches!(spec.placement, super::WindowPlacement::Host(_)))
    }

    pub(crate) fn shows_in(&self, window: WindowId) -> bool {
        self.opens_windows() && (window.is_main() || self.window_node(window).is_some())
    }

    pub(crate) fn shown_in(&self, overlay: NodeOf<OverlayNode>) -> Option<WindowId> {
        self.arena
            .get_as::<OverlayNode>(overlay)
            .shown_in
            .filter(|window| self.shows_in(*window))
    }

    pub(crate) fn real_window(&self, overlay: NodeOf<OverlayNode>) -> Option<WindowId> {
        self.arena
            .get_as::<OverlayNode>(overlay)
            .window
            .as_ref()
            .filter(|window| self.hosts_window(window))
            .map(|window| window.id)
    }

    pub(crate) fn window_node(&self, window: WindowId) -> Option<NodeOf<OverlayNode>> {
        self.windows
            .nodes
            .iter()
            .find(|(id, node)| {
                *id == window
                    && self.contains(*node)
                    && self
                        .arena
                        .get_as::<OverlayNode>(*node)
                        .window
                        .as_ref()
                        .is_some_and(|spec| spec.id == window)
            })
            .map(|(_, node)| *node)
    }

    pub fn window_of(&self, node: NodeId) -> WindowId {
        if !self.opens_windows() {
            return WindowId::MAIN;
        }
        let mut current = Some(node);
        for _ in 0..ANCESTOR_LIMIT {
            let Some(id) = current else {
                break;
            };
            if self.arena.current(id) && self.arena.try_get(id).is_none() {
                return self.viewport_window();
            }
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id)
                && let Some(window) = self.real_window(overlay).or_else(|| self.shown_in(overlay))
            {
                return window;
            }
            if Some(id) == self.root {
                break;
            }
            current = self.arena.parent(id);
        }
        WindowId::MAIN
    }

    pub fn host_window_of(&self, window: WindowId) -> WindowId {
        self.windows
            .nodes
            .iter()
            .find(|(id, node)| *id == window && self.contains(*node))
            .map_or(WindowId::MAIN, |(_, node)| self.window_of((*node).into()))
    }

    pub fn window_hosts(&self) -> Vec<(WindowId, WindowId)> {
        self.windows
            .nodes
            .iter()
            .filter(|(_, node)| self.contains(*node))
            .map(|(id, node)| (*id, self.window_of((*node).into())))
            .collect()
    }

    pub fn key_window(&self) -> Option<WindowId> {
        if !self.opens_windows() {
            return None;
        }
        self.focused
            .map(|focused| self.window_of(focused))
            .or(self.windows.host_focused)
    }

    pub fn window_layers(&self, window: WindowId) -> Option<Layers> {
        self.layers().of_window(window)
    }

    pub fn window_size(&self, window: WindowId) -> Option<Vec2> {
        let node = self.window_node(window)?;
        let spec = self.arena.get_as::<OverlayNode>(node).window.as_ref()?;
        let reported = self.context().and_then(|ctx| ctx.window_size(window));
        Some(reported.unwrap_or(spec.size))
    }

    pub fn window_rect(&self, window: WindowId) -> Option<Rect> {
        if window.is_main() || !self.opens_windows() {
            return None;
        }
        Some(Rect::from_min_size(Pos2::ZERO, self.window_size(window)?))
    }

    pub(crate) fn swap_window_viewport(
        &mut self,
        viewport: Option<(WindowId, Rect)>,
    ) -> Option<(WindowId, Rect)> {
        std::mem::replace(&mut self.windows.viewport, viewport)
    }

    pub(crate) fn viewport_window(&self) -> WindowId {
        self.windows
            .viewport
            .map_or(WindowId::MAIN, |(window, _)| window)
    }

    pub(crate) fn window_viewport(&self, window: WindowId) -> Option<(WindowId, Rect)> {
        let rect = self.window_rect(window)?;
        let rect = self
            .context()
            .map_or(rect, |ctx| ctx.keyboard_trimmed(window, rect));
        Some((window, rect))
    }

    pub(crate) fn relayout_window(&mut self, window: WindowId) {
        if let Some(node) = self.window_node(window) {
            self.arena.invalidate_node(node);
        }
    }

    pub fn open_windows(&self) -> Vec<WindowId> {
        if !self.opens_windows() {
            return Vec::new();
        }
        self.open_overlays()
            .into_iter()
            .filter_map(|overlay| self.real_window(overlay))
            .filter(|window| !self.windows.closed_by_host.contains(window))
            .collect()
    }

    pub fn request_window(&mut self, request: WindowRequest) -> bool {
        let shown = self.open_windows().contains(&request.window());
        if shown {
            self.windows.requests.push(request);
            if let Some(ctx) = self.context() {
                ctx.request_repaint();
            }
        }
        shown
    }

    pub(crate) fn follow_windows(&mut self, ctx: &Context) {
        let hosting = ctx.window_hosting();
        let opens = hosting == super::WindowHosting::Any;
        let placed = hosting == super::WindowHosting::Placed;
        if opens != self.windows.opens.get_untracked()
            || placed != self.windows.placed.get_untracked()
        {
            self.windows.opens.set(opens);
            self.windows.placed.set(placed);
            self.layers_changed();
            self.arena.invalidate();
        }
        let keyboard = ctx.keyboard(|state| state.own_window());
        let reports = ctx.take_window_reports(|window| {
            Some(window) != keyboard || self.window_node(window).is_some()
        });
        if reports.is_empty() {
            return;
        }
        let mut heard = Vec::new();
        let mut unfocused = Vec::new();
        let mut rastered = false;
        for report in reports {
            let node = self.window_node(report.window());
            match &report {
                WindowReport::Closed(window) => {
                    if self.windows.host_focused == Some(*window) {
                        self.windows.host_focused = None;
                    }
                    self.windows.closed_by_host.insert(*window);
                    self.windows.asked.retain(|asked| asked != window);
                    self.layers_changed();
                }
                WindowReport::Resized { window, .. } => {
                    self.windows.focus_resized |= self
                        .focused
                        .is_some_and(|focused| self.window_of(focused) == *window);
                    if let Some(node) = node {
                        self.arena.invalidate_node(node);
                    }
                }
                WindowReport::Rastered { .. } => {
                    rastered = true;
                    if let Some(shown) = node.map(NodeId::from).or(self.root) {
                        self.arena.repaint_node(shown);
                    }
                }
                WindowReport::Placed { .. } | WindowReport::Moved { .. } => {}
                WindowReport::Focused { window, focused } => {
                    match focused {
                        true => self.windows.host_focused = Some(*window),
                        false if self.windows.host_focused == Some(*window) => {
                            self.windows.host_focused = None;
                        }
                        false => {}
                    }
                    if !self.window_focused(*window, *focused) {
                        unfocused.extend(node);
                    }
                }
            }
            if let Some(spec) = node.and_then(|node| self.overlay_window(node)) {
                heard.push((spec.on_report, report));
            }
        }
        let context = self.reactive_scope().context();
        let guard = crate::current::install(self);
        context.run(|| {
            for (on_report, report) in heard {
                on_report.call(report);
            }
        });
        drop(guard);
        if rastered {
            crate::current::run_in(self, Document::rerasterise_embeds);
        }
        for node in unfocused {
            let root = node.into();
            if self.contains(root)
                && !self.focus_is_within(root)
                && !self.focus_pending_within(root)
            {
                crate::current::run_in(self, |document| document.focus_into(root));
            }
        }
    }

    fn window_focused(&mut self, window: WindowId, focused: bool) -> bool {
        if self.windows.placed.get_untracked() {
            if focused {
                self.move_focus_into(window);
            }
            return true;
        }
        let current = self.focused.filter(|node| self.window_of(*node) == window);
        match focused {
            false => {
                if let Some(node) = current {
                    self.windows.kept_focus.insert(window, node);
                    crate::current::run_in(self, |document| document.update_focus(None));
                }
                true
            }
            true => {
                let kept = self.windows.kept_focus.remove(&window);
                match kept.filter(|node| self.contains(*node)) {
                    Some(node) => {
                        if self.focused.is_none() {
                            crate::current::run_in(self, |document| {
                                document.update_focus(Some(node))
                            });
                        }
                        true
                    }
                    None => false,
                }
            }
        }
    }

    fn move_focus_into(&mut self, window: WindowId) {
        if let Some(node) = self.focused {
            let holder = self.window_of(node);
            if holder == window {
                return;
            }
            self.windows.kept_focus.insert(holder, node);
            crate::current::run_in(self, |document| document.update_focus(None));
        }
        if let Some(node) = self
            .windows
            .kept_focus
            .remove(&window)
            .filter(|node| self.contains(*node))
        {
            crate::current::run_in(self, |document| document.update_focus(Some(node)));
        }
    }

    pub(crate) fn ask_for_windows(&mut self, ctx: &Context) {
        let open = self.open_windows();
        let hosted: HashSet<WindowId> = self
            .open_overlays()
            .into_iter()
            .filter_map(|overlay| self.real_window(overlay))
            .collect();
        self.windows
            .closed_by_host
            .retain(|window| hosted.contains(window));
        for window in std::mem::take(&mut self.windows.asked) {
            match open.contains(&window) {
                true => self.windows.asked.push(window),
                false => {
                    if !self.windows.closed_by_host.contains(&window) {
                        ctx.request_window(WindowRequest::Close(window));
                    }
                }
            }
        }
        let asked = self.windows.asked.clone();
        self.windows
            .wanted
            .retain(|window, _| asked.contains(window));
        for window in &open {
            let Some(spec) = self
                .window_node(*window)
                .and_then(|node| self.overlay_window(node))
            else {
                continue;
            };
            let size = spec.size;
            if !self.windows.asked.contains(window) {
                ctx.request_window(WindowRequest::Open {
                    window: *window,
                    placement: spec.placement.clone(),
                    size,
                    role: spec.role,
                });
                self.windows.asked.push(*window);
                self.windows.wanted.insert(*window, size);
                continue;
            }
            let wanted = self.windows.wanted.insert(*window, size);
            if wanted != Some(size) && ctx.window_size(*window) != Some(size) {
                ctx.request_window(WindowRequest::Resize {
                    window: *window,
                    size,
                });
            }
        }
        for request in std::mem::take(&mut self.windows.requests) {
            if self.windows.asked.contains(&request.window()) {
                ctx.request_window(request);
            }
        }
    }

    pub(crate) fn show_windows(
        &mut self,
        ctx: &Context,
        damage: Option<&Taken>,
        moves: &[(WindowId, Moved)],
    ) {
        let open = self.open_windows();
        self.windows.shown.retain(|window, _| open.contains(window));
        for window in open {
            let Some(rect) = self.window_rect(window) else {
                continue;
            };
            let resized = self.windows.shown.insert(window, rect.size()) != Some(rect.size());
            let (damage, moved) = match resized {
                true => (Region::from(rect), None),
                false => (
                    damage.map_or(Region::NOTHING, |damage| damage.of(window, rect)),
                    moves
                        .iter()
                        .find(|(held, _)| *held == window)
                        .map(|(_, moved)| *moved),
                ),
            };
            let painting = self
                .windows
                .paintings
                .iter()
                .find(|(id, _)| *id == window)
                .map(|(_, painting)| painting.clone())
                .unwrap_or_default();
            let title = self
                .window_node(window)
                .and_then(|node| self.overlay_window(node))
                .and_then(|spec| spec.title.clone());
            let appearance = self.windows.appearances.get(&window);
            ctx.show_window(
                window,
                rect.size(),
                &painting,
                damage,
                moved,
                title,
                appearance,
            );
        }
    }
}
