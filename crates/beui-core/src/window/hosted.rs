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
    asked: Vec<WindowId>,
    wanted: HashMap<WindowId, Vec2>,
    closed_by_host: HashSet<WindowId>,
    requests: Vec<WindowRequest>,
    shown: HashMap<WindowId, Vec2>,
    pub(crate) paintings: Vec<(WindowId, Painting)>,
    pub(crate) viewport: Option<(WindowId, Rect)>,
    kept_focus: HashMap<WindowId, NodeId>,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            opens: ::reactive::OwnedSignal::new(false),
            asked: Vec::new(),
            wanted: HashMap::default(),
            closed_by_host: HashSet::default(),
            requests: Vec::new(),
            shown: HashMap::default(),
            paintings: Vec::new(),
            viewport: None,
            kept_focus: HashMap::default(),
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
        self.windows.opens.get_untracked()
    }

    pub fn watch_opens_windows(&self) -> ::reactive::ReadSignal<bool> {
        self.windows.opens.read_only()
    }

    pub(crate) fn real_window(&self, overlay: NodeOf<OverlayNode>) -> Option<WindowId> {
        if !self.opens_windows() {
            return None;
        }
        self.arena
            .get_as::<OverlayNode>(overlay)
            .window
            .as_ref()
            .map(|window| window.id)
    }

    fn window_node(&self, window: WindowId) -> Option<NodeOf<OverlayNode>> {
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
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id)
                && let Some(window) = self.real_window(overlay)
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
        self.window_rect(window).map(|rect| (window, rect))
    }

    pub(crate) fn in_window_viewport(&self) -> bool {
        self.windows.viewport.is_some()
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
        let opens = ctx.opens_windows();
        if opens != self.opens_windows() {
            self.windows.opens.set(opens);
            self.layers_changed();
            self.arena.invalidate();
        }
        let reports = ctx.take_window_reports();
        if reports.is_empty() {
            return;
        }
        let mut heard = Vec::new();
        for report in reports {
            let node = self.window_node(report.window());
            match &report {
                WindowReport::Closed(window) => {
                    self.windows.closed_by_host.insert(*window);
                    self.windows.asked.retain(|asked| asked != window);
                    self.layers_changed();
                }
                WindowReport::Resized { .. } => {
                    if let Some(node) = node {
                        self.arena.invalidate_node(node);
                    }
                }
                WindowReport::Scaled { .. } => self.arena.invalidate(),
                WindowReport::Placed { .. } => {}
                WindowReport::Focused { window, focused } => self.window_focused(*window, *focused),
            }
            if let Some(spec) = node.and_then(|node| self.overlay_window(node)) {
                heard.push((spec.on_report, report));
            }
        }
        let context = self.reactive_scope().context();
        let _guard = crate::current::install(self);
        context.run(|| {
            for (on_report, report) in heard {
                on_report.call(report);
            }
        });
    }

    fn window_focused(&mut self, window: WindowId, focused: bool) {
        let current = self.focused.filter(|node| self.window_of(*node) == window);
        match focused {
            false => {
                if let Some(node) = current {
                    self.windows.kept_focus.insert(window, node);
                    crate::current::run_in(self, |document| document.update_focus(None));
                }
            }
            true => {
                let kept = self.windows.kept_focus.remove(&window);
                if self.focused.is_none()
                    && let Some(node) = kept.filter(|node| self.contains(*node))
                {
                    crate::current::run_in(self, |document| document.update_focus(Some(node)));
                }
            }
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
            ctx.show_window(window, rect.size(), &painting, damage, moved, title);
        }
    }
}
