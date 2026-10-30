use crate::back::BackHandlerId;
use crate::base::overlay::OverlayNode;
use crate::callback::Callback;
use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::input::BackGesture;
use crate::node::{NodeId, NodeOf};
use crate::window::WindowId;

use crate::node::ANCESTOR_LIMIT;

pub use beui_types::layer::{AboveEmbeds, Dismiss, LayerLevel, LayerPolicy};

#[derive(Default)]
pub struct OpenLayers {
    open: Vec<Opened>,
    next: u64,
    generation: u64,
    cache: std::cell::RefCell<Option<(LayersKey, Layers)>>,
}

type LayersKey = (u64, u64, Option<NodeId>);

impl OpenLayers {
    pub(crate) fn changed(&mut self) {
        self.generation += 1;
    }
}

struct Opened {
    overlay: NodeOf<OverlayNode>,
    seq: u64,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub overlay: Option<NodeOf<OverlayNode>>,
    pub parent: Option<usize>,
    pub hangs_from: Option<NodeId>,
    pub content: Option<NodeId>,
    pub policy: LayerPolicy,
    pub level: LayerLevel,
    pub traps: bool,
    pub dismissable: bool,
    pub locks: bool,
    pub window: WindowId,
    pub escapes: bool,
}

impl Layer {
    pub fn blocks_all(&self) -> bool {
        self.policy == LayerPolicy::BlocksAll
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DismissTarget {
    Handler(BackHandlerId),
    Overlay(NodeOf<OverlayNode>),
}

type StackKey = (LayerLevel, Vec<(bool, u64)>);

type Hanging = (
    NodeOf<OverlayNode>,
    Option<NodeOf<OverlayNode>>,
    Option<NodeId>,
);

#[derive(Clone)]
pub struct Layers(std::rc::Rc<[Layer]>);

impl std::ops::Deref for Layers {
    type Target = [Layer];

    fn deref(&self) -> &[Layer] {
        &self.0
    }
}

impl Layers {
    pub fn index_of(&self, overlay: NodeOf<OverlayNode>) -> Option<usize> {
        self.0
            .iter()
            .position(|layer| layer.overlay == Some(overlay))
    }

    pub fn keyboard(&self) -> usize {
        self.0.iter().rposition(|layer| layer.traps).unwrap_or(0)
    }

    pub fn blocking(&self) -> usize {
        self.0.iter().rposition(Layer::blocks_all).unwrap_or(0)
    }

    pub fn locked(&self) -> bool {
        self.0.iter().any(|layer| layer.locks)
    }

    pub fn reachable(&self, index: usize) -> bool {
        let blocking = self.blocking();
        if index >= blocking {
            return true;
        }
        let mut chain = self.0[blocking].parent;
        while let Some(at) = chain.filter(|at| *at > 0) {
            if at == index {
                return true;
            }
            if !self.0[at].blocks_all() {
                return false;
            }
            chain = self.0[at].parent;
        }
        false
    }

    pub fn takes_keyboard(&self) -> bool {
        self.keyboard() > 0
    }

    pub fn has_windows(&self) -> bool {
        self.0.iter().any(|layer| !layer.window.is_main())
    }

    pub fn of_window(&self, window: WindowId) -> Option<Layers> {
        if window.is_main() && !self.has_windows() {
            return Some(self.clone());
        }
        let kept: Vec<usize> = (0..self.0.len())
            .filter(|index| self.0[*index].window == window)
            .collect();
        let first = *kept.first()?;
        let layers: Vec<Layer> = kept
            .iter()
            .map(|index| {
                let mut layer = self.0[*index].clone();
                layer.parent = layer
                    .parent
                    .and_then(|parent| kept.iter().position(|kept| *kept == parent));
                if *index == first {
                    layer.parent = None;
                    layer.policy = LayerPolicy::Pass;
                }
                layer
            })
            .collect();
        Some(Layers(layers.into()))
    }

    pub fn within(&self, index: usize, ancestor: usize) -> bool {
        let mut current = Some(index);
        while let Some(at) = current {
            if at == ancestor {
                return true;
            }
            current = self.0[at].parent;
        }
        false
    }
}

impl Document {
    pub(crate) fn open_overlays(&self) -> Vec<NodeOf<OverlayNode>> {
        self.layer_state
            .open
            .iter()
            .map(|opened| opened.overlay)
            .filter(|overlay| self.contains(*overlay) && self.arena.get_as(*overlay).open)
            .collect()
    }

    pub(crate) fn layers_changed(&mut self) {
        self.layer_state.generation += 1;
        self.focus_dirty = true;
        for overlay in self.open_overlays() {
            self.arena.invalidate_node(overlay);
        }
    }

    pub(crate) fn is_within(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut current = node;
        for _ in 0..ANCESTOR_LIMIT {
            if current == ancestor {
                return true;
            }
            if Some(current) == self.root {
                return false;
            }
            match self.arena.parent(current) {
                Some(parent) => current = parent,
                None => return false,
            }
        }
        false
    }

    fn host_layer(
        &self,
        start: Option<NodeId>,
        own: NodeOf<OverlayNode>,
        open: &[NodeOf<OverlayNode>],
    ) -> Option<NodeOf<OverlayNode>> {
        let mut current = start;
        for _ in 0..ANCESTOR_LIMIT {
            let id = current?;
            if id != own.id()
                && let Some(found) = open.iter().find(|overlay| overlay.id() == id)
            {
                return Some(*found);
            }
            current = self.arena.parent(id);
        }
        None
    }

    pub fn layers(&self) -> Layers {
        let key = (self.arena.revision, self.layer_state.generation, self.root);
        if let Some((cached, layers)) = &*self.layer_state.cache.borrow()
            && *cached == key
            && layers.iter().skip(1).all(|layer| {
                layer.overlay.is_some_and(|overlay| self.contains(overlay))
                    && layer.hangs_from.is_none_or(|from| self.contains(from))
            })
        {
            return layers.clone();
        }
        let layers = self.build_layers();
        *self.layer_state.cache.borrow_mut() = Some((key, layers.clone()));
        layers
    }

    fn build_layers(&self) -> Layers {
        let open = self.open_overlays();
        let mut entries: Vec<Hanging> = Vec::with_capacity(open.len());
        for overlay in &open {
            let node = self.arena.get_as::<OverlayNode>(*overlay);
            let hangs_from = node
                .anchor_node()
                .filter(|anchor| self.contains(*anchor))
                .or_else(|| self.arena.parent(overlay.id()));
            let parent = self.host_layer(hangs_from, *overlay, &open);
            entries.push((*overlay, parent, hangs_from));
        }
        let seq_of = |overlay: NodeOf<OverlayNode>| {
            self.layer_state
                .open
                .iter()
                .find(|opened| opened.overlay == overlay)
                .map_or(0, |opened| opened.seq)
        };
        let parent_of = |overlay: NodeOf<OverlayNode>| {
            entries
                .iter()
                .find(|(id, _, _)| *id == overlay)
                .and_then(|(_, parent, _)| *parent)
        };
        let mut keyed: Vec<(StackKey, usize)> = Vec::with_capacity(entries.len());
        for (index, (overlay, _, _)) in entries.iter().enumerate() {
            let mut chain = vec![*overlay];
            let mut current = parent_of(*overlay);
            while let Some(parent) = current {
                if chain.contains(&parent) || chain.len() > open.len() {
                    break;
                }
                chain.push(parent);
                current = parent_of(parent);
            }
            chain.reverse();
            let level = chain
                .iter()
                .map(|overlay| {
                    let node = self.arena.get_as::<OverlayNode>(*overlay);
                    node.level.stacked(node.policy)
                })
                .max()
                .unwrap_or_default();
            let seqs = chain
                .iter()
                .map(|overlay| {
                    let blocks =
                        self.arena.get_as::<OverlayNode>(*overlay).policy == LayerPolicy::BlocksAll;
                    (blocks, seq_of(*overlay))
                })
                .collect();
            keyed.push(((level, seqs), index));
        }
        keyed.sort();
        let mut layers = Vec::with_capacity(keyed.len() + 1);
        layers.push(Layer {
            overlay: None,
            parent: None,
            hangs_from: None,
            content: self.root,
            policy: LayerPolicy::Pass,
            level: LayerLevel::Normal,
            traps: false,
            dismissable: false,
            locks: false,
            window: WindowId::MAIN,
            escapes: false,
        });
        for ((level, _), index) in &keyed {
            let (overlay, _, hangs_from) = entries[*index];
            let node = self.arena.get_as::<OverlayNode>(overlay);
            let locks = node.policy == LayerPolicy::BlocksAll && node.level == LayerLevel::Lock;
            layers.push(Layer {
                overlay: Some(overlay),
                parent: None,
                hangs_from,
                content: node.content,
                policy: node.policy,
                level: *level,
                traps: node.traps_focus && node.policy != LayerPolicy::Inert,
                dismissable: !node.on_dismiss.is_empty() && !locks,
                locks,
                window: self
                    .real_window(overlay)
                    .or_else(|| self.shown_in(overlay))
                    .unwrap_or(WindowId::MAIN),
                escapes: false,
            });
        }
        for at in 1..layers.len() {
            let overlay = layers[at]
                .overlay
                .expect("only the root layer has no overlay");
            layers[at].parent = Some(
                parent_of(overlay)
                    .and_then(|parent| {
                        layers
                            .iter()
                            .position(|layer| layer.overlay == Some(parent))
                    })
                    .unwrap_or(0),
            );
            if layers[at].window.is_main()
                && self.shown_in(overlay).is_none()
                && let Some(parent) = layers[at].parent
            {
                layers[at].window = layers[parent].window;
            }
            let node = self.arena.get_as::<OverlayNode>(overlay);
            layers[at].escapes = self.real_window(overlay).is_none()
                && self
                    .escape_space(node.placement(), layers[at].window)
                    .is_some();
        }
        Layers(layers.into())
    }

    pub fn layer_of(&self, layers: &Layers, node: NodeId) -> Option<usize> {
        let mut current = Some(node);
        for _ in 0..ANCESTOR_LIMIT {
            let id = current?;
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id) {
                return layers.index_of(overlay);
            }
            if Some(id) == self.root {
                return Some(0);
            }
            current = self.arena.parent(id);
        }
        None
    }

    pub fn locked(&self) -> bool {
        self.layers().locked()
    }

    pub fn open_overlay(&mut self, overlay: NodeOf<OverlayNode>) {
        if self.arena.get_as::<OverlayNode>(overlay).open {
            return;
        }
        let traps = self.arena.get_as::<OverlayNode>(overlay).traps_focus;
        self.push_layer_scope(overlay.id(), traps);
        let node = self.arena.get_mut_as::<OverlayNode>(overlay);
        node.open = true;
        self.layer_state
            .open
            .retain(|opened| opened.overlay != overlay);
        self.layer_state.open.push(Opened { overlay, seq: 0 });
        self.raise_layer(overlay);
        self.layers_changed();
        self.redirect_blocked_focus();
    }

    pub fn raise_overlay(&mut self, overlay: NodeId) {
        if let Some(overlay) = self.arena.kind_of::<OverlayNode>(overlay)
            && self.is_overlay_open(overlay.id())
        {
            self.raise_layer(overlay);
            if let Some(window) = self.real_window(overlay) {
                self.request_window(crate::window::WindowRequest::Raise(window));
            }
        }
    }

    fn raise_layer(&mut self, overlay: NodeOf<OverlayNode>) {
        let layers = self.layers();
        let Some(index) = layers.index_of(overlay) else {
            return;
        };
        let mut chain = vec![];
        let mut current = Some(index);
        let alone = layers[index].policy == LayerPolicy::Inert;
        while let Some(at) = current.filter(|at| *at > 0) {
            chain.push(layers[at].overlay.expect("not the root layer"));
            current = layers[at].parent.filter(|_| !alone);
        }
        for raised in chain.into_iter().rev() {
            let next = self.layer_state.next;
            self.layer_state.next += 1;
            if let Some(opened) = self
                .layer_state
                .open
                .iter_mut()
                .find(|opened| opened.overlay == raised)
                && opened.seq != next
            {
                opened.seq = next;
                self.layer_state.generation += 1;
                self.arena.invalidate_node(raised);
            }
        }
    }

    pub fn close_overlay(&mut self, overlay: NodeOf<OverlayNode>) {
        self.end_layer(overlay);
    }

    fn end_layer(&mut self, overlay: NodeOf<OverlayNode>) {
        if !self.contains(overlay) || !self.arena.get_as::<OverlayNode>(overlay).open {
            return;
        }
        let nested: Vec<NodeOf<OverlayNode>> = self
            .open_overlays()
            .into_iter()
            .filter(|other| *other != overlay && self.is_within(other.id(), overlay.id()))
            .collect();
        for id in std::iter::once(overlay).chain(nested.iter().copied()) {
            let node = self.arena.get_mut_as::<OverlayNode>(id);
            node.open = false;
            node.content = None;
        }
        self.layer_state
            .open
            .retain(|opened| opened.overlay != overlay && !nested.contains(&opened.overlay));
        self.layers_changed();
        for id in &nested {
            self.forget_focus_scope(id.id());
        }
        self.pop_scope(overlay.id());
        for id in nested {
            self.call_overlay_dismiss(id, Dismiss::WithParent);
        }
    }

    pub(crate) fn forget_layer(&mut self, id: NodeId) {
        self.layer_state.changed();
        self.layer_state
            .open
            .retain(|opened| opened.overlay.id() != id);
    }

    fn call_overlay_dismiss(&mut self, id: NodeOf<OverlayNode>, reason: Dismiss) {
        if !self.contains(id) {
            return;
        }
        let handler = self.arena.get_as::<OverlayNode>(id).on_dismiss;
        if !handler.is_empty() {
            handler.call(reason);
        }
    }

    fn back_handler_in(&self, layers: &Layers, index: usize) -> Option<BackHandlerId> {
        self.back_handlers
            .handlers
            .iter()
            .rev()
            .find(|handler| {
                handler.enabled
                    && handler.node(self.root).is_some_and(|node| {
                        self.contains(node)
                            && self.live(node)
                            && self.layer_of(layers, node) == Some(index)
                    })
            })
            .map(|handler| handler.id)
    }

    fn back_handler(&self, id: BackHandlerId) -> Option<Callback<BackGesture>> {
        self.back_handlers.get(id).map(|handler| handler.on_back)
    }

    fn dismiss_target_exists(&self, target: DismissTarget) -> bool {
        match target {
            DismissTarget::Handler(handler) => self.back_handlers.get(handler).is_some(),
            DismissTarget::Overlay(overlay) => self.contains(overlay.id()),
        }
    }

    pub(crate) fn live(&self, node: NodeId) -> bool {
        let mut id = node;
        for _ in 0..ANCESTOR_LIMIT {
            if Some(id) == self.root {
                return true;
            }
            let element = self.arena.get(id);
            if element
                .as_any()
                .downcast_ref::<OverlayNode>()
                .is_some_and(|overlay| !overlay.open)
            {
                return false;
            }
            let Some(parent) = self.arena.parent(id) else {
                return false;
            };
            let mut listed = false;
            self.arena
                .get(parent)
                .for_each_child(&mut |child| listed |= child == id);
            if !listed {
                return false;
            }
            id = parent;
        }
        false
    }

    fn deliver_to(&self, layers: &Layers, index: usize) -> Option<DismissTarget> {
        if let Some(handler) = self.back_handler_in(layers, index) {
            return Some(DismissTarget::Handler(handler));
        }
        let layer = &layers[index];
        match (layer.overlay, layer.dismissable) {
            (Some(overlay), true) => Some(DismissTarget::Overlay(overlay)),
            _ => None,
        }
    }

    pub(crate) fn dismiss_target(&self, reason: Dismiss) -> Option<DismissTarget> {
        let layers = self.layers();
        match reason {
            Dismiss::Back => {
                for index in (0..layers.len()).rev() {
                    let layer = &layers[index];
                    if layer.policy == LayerPolicy::Inert {
                        continue;
                    }
                    if let Some(handler) = self.back_handler_in(&layers, index) {
                        return Some(DismissTarget::Handler(handler));
                    }
                    if let (Some(overlay), true) = (layer.overlay, layer.dismissable) {
                        return Some(DismissTarget::Overlay(overlay));
                    }
                    if layer.blocks_all() {
                        return None;
                    }
                }
                None
            }
            Dismiss::Escape => {
                let index = self.escape_index(&layers)?;
                self.deliver_to(&layers, index)
            }
            Dismiss::OutsidePress | Dismiss::WithParent => None,
        }
    }

    fn escape_index(&self, layers: &Layers) -> Option<usize> {
        let mut current = match self
            .focused
            .and_then(|focused| self.layer_of(layers, focused))
        {
            Some(index) => Some(index),
            None => Some(layers.keyboard()),
        };
        while let Some(index) = current.filter(|index| *index > 0) {
            if layers[index].dismissable {
                return Some(index);
            }
            current = layers[index].parent;
        }
        None
    }

    pub fn escape_layer(&self) -> Option<NodeId> {
        let layers = self.layers();
        let index = self.escape_index(&layers)?;
        layers[index].overlay.map(|overlay| overlay.id())
    }

    pub fn dismiss_overlay(&mut self, overlay: NodeId, reason: Dismiss) {
        if let Some(overlay) = self.arena.kind_of::<OverlayNode>(overlay) {
            self.request_dismiss(overlay, reason);
        }
    }

    pub fn handles_back(&self) -> bool {
        self.dismiss_target(Dismiss::Back).is_some()
    }

    pub fn dismiss(&mut self, reason: Dismiss) -> bool {
        let Some(target) = self.dismiss_target(reason) else {
            return false;
        };
        self.deliver_dismiss(target, reason);
        true
    }

    pub fn request_dismiss(&mut self, overlay: NodeOf<OverlayNode>, reason: Dismiss) {
        self.escapes.asked(overlay);
        let layers = self.layers();
        let Some(index) = layers.index_of(overlay) else {
            return;
        };
        if !layers[index].dismissable {
            return;
        }
        if let Some(target) = self.deliver_to(&layers, index) {
            self.deliver_dismiss(target, reason);
        }
    }

    fn deliver_dismiss(&mut self, target: DismissTarget, reason: Dismiss) {
        match target {
            DismissTarget::Overlay(overlay) => self.call_overlay_dismiss(overlay, reason),
            DismissTarget::Handler(handler) => {
                if let Some(on_back) = self.back_handler(handler) {
                    on_back.call(BackGesture::Invoked);
                }
            }
        }
    }

    pub fn back(&mut self, gesture: BackGesture) {
        let target = match (gesture, self.back_gesture) {
            (BackGesture::Started { .. }, _) | (_, None) => self.dismiss_target(Dismiss::Back),
            (_, Some(target)) => Some(target),
        };
        self.back_gesture = match gesture {
            BackGesture::Started { .. } => target,
            BackGesture::Progressed(_) => self.back_gesture,
            BackGesture::Cancelled | BackGesture::Invoked => None,
        };
        let Some(target) = target.filter(|target| self.dismiss_target_exists(*target)) else {
            return;
        };
        match target {
            DismissTarget::Handler(handler) => {
                if let Some(on_back) = self.back_handler(handler) {
                    on_back.call(match gesture {
                        BackGesture::Progressed(progress) => {
                            BackGesture::Progressed(progress.clamp(0.0, 1.0))
                        }
                        gesture => gesture,
                    });
                }
            }
            DismissTarget::Overlay(overlay) => {
                if gesture == BackGesture::Invoked {
                    self.call_overlay_dismiss(overlay, Dismiss::Back);
                }
            }
        }
    }

    pub(crate) fn forget_back_gesture(&mut self, id: NodeId) {
        if self.back_gesture.is_some_and(
            |target| matches!(target, DismissTarget::Overlay(overlay) if overlay.id() == id),
        ) {
            self.back_gesture = None;
        }
    }

    pub(crate) fn forget_back_handler_gesture(&mut self, id: BackHandlerId) {
        if self.back_gesture == Some(DismissTarget::Handler(id)) {
            self.back_gesture = None;
        }
    }

    pub(crate) fn press_outside(&mut self, layers: &[NodeOf<OverlayNode>]) {
        for overlay in layers {
            if self.is_overlay_open(overlay.id()) {
                self.request_dismiss(*overlay, Dismiss::OutsidePress);
            }
        }
    }

    fn focus_blocker<'a>(&self, layers: &'a Layers, node: NodeId) -> Option<&'a Layer> {
        layers.iter().find(|layer| {
            layer.policy == LayerPolicy::BlocksParent
                && layer
                    .hangs_from
                    .is_some_and(|blocked| self.is_within(node, blocked))
                && layer
                    .overlay
                    .is_some_and(|overlay| !self.is_within(node, overlay.id()))
        })
    }

    pub(crate) fn focus_blocked(&self, layers: &Layers, node: NodeId) -> bool {
        self.focus_blocker(layers, node).is_some()
    }

    pub fn blocked_focus_redirect(&self, layers: &Layers, node: NodeId) -> Option<Option<NodeId>> {
        let mut node = node;
        let mut visited = Vec::new();
        loop {
            let blocker = self.focus_blocker(layers, node)?;
            let overlay = blocker.overlay?;
            if visited.contains(&overlay) {
                return Some(None);
            }
            visited.push(overlay);
            let Some(content) = self.overlay_content(overlay) else {
                return Some(None);
            };
            let content = match self.focus_into {
                Some((into, _)) if self.contains(into) && self.is_within(into, content) => into,
                _ => content,
            };
            match self.focusables_within(content).first().copied() {
                Some(first) => node = first,
                None => return Some(None),
            }
            if !self.focus_blocked(layers, node) {
                return Some(Some(node));
            }
        }
    }

    pub(crate) fn blocked_roots(&self, layers: &Layers) -> Vec<NodeId> {
        layers
            .iter()
            .filter(|layer| layer.policy == LayerPolicy::BlocksParent)
            .filter_map(|layer| layer.hangs_from)
            .collect()
    }

    fn redirect_blocked_focus(&mut self) {
        let Some(focused) = self.focused else {
            return;
        };
        let layers = self.layers();
        if let Some(redirect) = self.blocked_focus_redirect(&layers, focused) {
            self.update_focus(redirect);
        }
    }

    pub fn layer_covers(&self, layer: &Layer) -> Vec<Rect> {
        let Some(content) = layer.content.and_then(|content| self.node_rect(content)) else {
            return Vec::new();
        };
        let scrim = layer
            .overlay
            .filter(|overlay| self.arena.get_as::<OverlayNode>(*overlay).scrim_covers())
            .and_then(|overlay| self.node_rect(self.overlay_scrim(overlay)));
        let blocked = layer
            .hangs_from
            .filter(|_| layer.policy == LayerPolicy::BlocksParent)
            .and_then(|node| self.node_rect(node));
        let covers = match (layer.policy, scrim.or(blocked)) {
            (LayerPolicy::BlocksAll, Some(scrim)) => vec![scrim.union(content)],
            (_, Some(scrim)) => vec![scrim, content],
            (_, None) => vec![content],
        };
        covers.into_iter().filter(Rect::is_positive).collect()
    }

    pub(crate) fn layer_paints(&self, layer: &Layer) -> Vec<Rect> {
        let Some(content) = layer.content.and_then(|content| self.node_rect(content)) else {
            return Vec::new();
        };
        let scrim = layer
            .overlay
            .filter(|_| !self.hosts_scrims())
            .filter(|overlay| self.arena.get_as::<OverlayNode>(*overlay).dims())
            .and_then(|overlay| self.node_rect(self.overlay_scrim(overlay)));
        scrim
            .into_iter()
            .chain([content])
            .filter(Rect::is_positive)
            .collect()
    }

    pub(crate) fn hosts_scrims(&self) -> bool {
        self.context().is_some_and(|ctx| ctx.hosts_scrims())
    }

    pub fn overlay_scrims(&self, window: WindowId) -> Vec<(Rect, Color32)> {
        self.window_overlays(window)
            .iter()
            .filter_map(|(_, layer)| self.layer_scrim(layer))
            .collect()
    }

    pub(crate) fn layer_scrim(&self, layer: &Layer) -> Option<(Rect, Color32)> {
        layer.content?;
        let overlay = layer.overlay?;
        let node = self.arena.get_as::<OverlayNode>(overlay);
        let rect = self.node_rect(self.overlay_scrim(overlay))?;
        (node.dims() && rect.is_positive()).then(|| (rect, self.resolve_color(node.scrim_color())))
    }

    fn window_overlays(&self, window: WindowId) -> Vec<(usize, Layer)> {
        let own = self.window_node(window);
        self.layers()
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, layer)| layer.window == window && !layer.escapes)
            .filter(|(_, layer)| own.is_none() || layer.overlay != own)
            .map(|(index, layer)| (index, layer.clone()))
            .collect()
    }

    pub fn overlay_rects(&self, window: WindowId) -> Vec<Rect> {
        self.window_overlays(window)
            .iter()
            .flat_map(|(_, layer)| self.layer_covers(layer))
            .chain(self.above_embeds_in(window, false).map(|(_, rect)| rect))
            .collect()
    }

    pub fn overlay_painted_rects(&self, window: WindowId) -> Vec<Rect> {
        self.window_overlays(window)
            .iter()
            .flat_map(|(_, layer)| self.layer_paints(layer))
            .chain(self.above_embeds_in(window, true).map(|(_, rect)| rect))
            .collect()
    }

    pub fn set_above_embeds(&mut self, node: NodeId, above: Option<AboveEmbeds>) {
        match above {
            Some(above) => self.above_embeds.insert(node, above),
            None => self.above_embeds.remove(&node),
        };
    }

    fn above_embeds_in(
        &self,
        window: WindowId,
        painted: bool,
    ) -> impl Iterator<Item = (NodeId, Rect)> + '_ {
        let mut found: Vec<(Vec<usize>, NodeId, Rect)> = self
            .above_embeds
            .iter()
            .filter(move |(_, above)| !painted || **above == AboveEmbeds::Paints)
            .filter(|(node, _)| self.arena.contains(**node))
            .filter(move |(node, _)| self.window_of(**node) == window)
            .filter_map(|(node, _)| Some((*node, self.node_rect(*node)?)))
            .filter(|(_, rect)| rect.is_positive())
            .map(|(node, rect)| (self.paint_order(node), node, rect))
            .collect();
        found.sort_by(|(left, ..), (right, ..)| left.cmp(right));
        found.into_iter().map(|(_, node, rect)| (node, rect))
    }

    pub fn set_kept_clear(&mut self, node: NodeId, rects: Option<Vec<Rect>>) {
        match rects {
            Some(rects) => self.clear_of.insert(node, rects),
            None => self.clear_of.remove(&node),
        };
    }

    pub fn kept_clear(&self, window: WindowId) -> Vec<Rect> {
        let mut nodes: Vec<(Vec<usize>, &NodeId, &Vec<Rect>)> = self
            .clear_of
            .iter()
            .filter(|(node, _)| self.arena.contains(**node) && self.window_of(**node) == window)
            .map(|(node, rects)| (self.paint_order(*node), node, rects))
            .collect();
        nodes.sort_by(|(left, ..), (right, ..)| left.cmp(right));
        nodes
            .into_iter()
            .map(|(_, node, rects)| (node, rects))
            .filter_map(|(node, rects)| Some((self.node_rect(*node)?, rects)))
            .flat_map(|(at, rects)| {
                rects
                    .iter()
                    .map(move |rect| rect.translate(at.min.to_vec2()))
            })
            .filter(|rect| rect.is_positive())
            .collect()
    }

    pub fn floating_rects(&self, window: WindowId) -> Vec<Rect> {
        self.window_overlays(window)
            .iter()
            .filter(|(_, layer)| layer.policy == LayerPolicy::Pass)
            .filter_map(|(_, layer)| self.node_rect(layer.content?))
            .collect()
    }

    pub fn overlay_layers(&self, window: WindowId) -> Vec<(Vec<usize>, Rect)> {
        let mut layers: Vec<(Vec<usize>, Rect)> = self
            .window_overlays(window)
            .iter()
            .flat_map(|(index, layer)| {
                self.layer_covers(layer)
                    .into_iter()
                    .map(move |cover| (vec![*index], cover))
            })
            .chain(
                self.above_embeds_in(window, false)
                    .map(|(node, rect)| (self.paint_order(node), rect)),
            )
            .collect();
        layers.sort_by(|(left, _), (right, _)| left.cmp(right));
        layers
    }

    pub fn paint_order(&self, node: NodeId) -> Vec<usize> {
        let layers = self.layers();
        let mut path = Vec::new();
        let mut current = node;
        let layer = loop {
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(current)
                && let Some(index) = layers.index_of(overlay)
            {
                break index;
            }
            let Some(parent) = self.arena.parent(current) else {
                break 0;
            };
            let index = self
                .children(parent)
                .iter()
                .position(|child| *child == current)
                .unwrap_or_default();
            path.push(index);
            current = parent;
        };
        path.push(layer);
        path.reverse();
        path
    }

    pub fn in_blocked(
        &self,
        layers: &Layers,
        at: usize,
        node: Option<NodeId>,
        pos: Pos2,
    ) -> Option<NodeOf<OverlayNode>> {
        layers
            .iter()
            .enumerate()
            .skip(at + 1)
            .find_map(|(_, layer)| {
                if layer.policy != LayerPolicy::BlocksParent {
                    return None;
                }
                let blocked = layer.hangs_from?;
                let inside = node.is_some_and(|node| self.is_within(node, blocked))
                    || (self.layer_of(layers, blocked) == Some(at)
                        && self
                            .rects
                            .visible(&blocked)
                            .is_some_and(|rect| rect.contains_half_open(pos)));
                inside.then_some(layer.overlay?)
            })
    }
}
