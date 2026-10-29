use std::rc::Rc;

use foldhash::HashMap;

use crate::base::overlay::OverlayNode;
use crate::color::Color32;
use crate::context::{Context, EscapedOutput, WindowOutput};
use crate::damage::Region;
use crate::display::{Display, Group, Layer as Shown};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Rotation, Vec2};
use crate::layer::{Dismiss, LayerPolicy};
use crate::node::{NodeId, NodeOf};
use crate::painter::{Corners, Shape};
use crate::window::{WindowId, WindowState};

pub use beui_types::layer::{
    EscapedLayer, HostTransform, LayerEdge, LayerPlacement, LayerSpace, Placement,
};

pub(crate) type Painting = Vec<(Rc<Display>, Vec2)>;

struct Escape {
    overlay: NodeOf<OverlayNode>,
    id: u64,
    paint: WindowId,
    shown: Option<EscapedLayer>,
}

#[derive(Default)]
pub(crate) struct Escapes {
    spaces: HashMap<WindowId, LayerSpace>,
    held: Vec<Escape>,
    next: u64,
    pub(crate) paintings: Vec<(WindowId, Painting)>,
    asked: Vec<NodeOf<OverlayNode>>,
}

impl Escapes {
    pub(crate) fn asked(&mut self, overlay: NodeOf<OverlayNode>) {
        if !self.asked.contains(&overlay) {
            self.asked.push(overlay);
        }
    }

    pub(crate) fn paint_window(&self, overlay: NodeOf<OverlayNode>) -> Option<WindowId> {
        self.held
            .iter()
            .find(|escape| escape.overlay == overlay)
            .map(|escape| escape.paint)
    }

    pub(crate) fn any(&self) -> bool {
        !self.held.is_empty()
    }

    pub(crate) fn overlays(&self) -> impl Iterator<Item = (NodeId, u64)> + '_ {
        self.held
            .iter()
            .map(|escape| (escape.overlay.id(), escape.id))
    }

    pub(crate) fn paints_into(&self, window: WindowId) -> bool {
        self.held.iter().any(|escape| escape.paint == window)
    }
}

impl Document {
    pub fn layer_space(&self, window: WindowId) -> Option<LayerSpace> {
        self.escapes.spaces.get(&window).copied()
    }

    pub(crate) fn escape_space(
        &self,
        placement: Placement,
        window: WindowId,
    ) -> Option<LayerSpace> {
        placement
            .escapes()
            .then(|| self.layer_space(window))
            .flatten()
    }

    pub fn opener_of(&self, node: NodeId) -> NodeId {
        let mut opener = node;
        let mut current = Some(node);
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            let Some(id) = current else {
                break;
            };
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id) {
                let held = self.arena.get_as::<OverlayNode>(overlay);
                let trigger = held
                    .trigger
                    .as_ref()
                    .and_then(crate::callback::NodeRef::try_get);
                let menu = held.policy() == LayerPolicy::BlocksAll && trigger.is_some();
                let from = trigger.or_else(|| held.anchor_node());
                match (menu || !held.is_open(), from) {
                    (true, Some(from)) if self.contains(from) => {
                        opener = from;
                        current = Some(from);
                        continue;
                    }
                    _ => return opener,
                }
            }
            if Some(id) == self.root {
                break;
            }
            current = self.arena.parent(id);
        }
        opener
    }

    pub(crate) fn follow_layer_spaces(&mut self, ctx: &Context) {
        let spaces = ctx.layer_spaces();
        if spaces != self.escapes.spaces {
            self.escapes.spaces = spaces;
            self.layers_changed();
        }
    }

    pub(crate) fn begin_escapes(&mut self) {
        self.escapes.asked.clear();
    }

    pub(crate) fn sync_escapes(&mut self) {
        let layers = self.layers();
        let escaping: Vec<NodeOf<OverlayNode>> = layers
            .iter()
            .filter(|layer| layer.escapes)
            .filter_map(|layer| layer.overlay)
            .collect();
        self.escapes
            .held
            .retain(|escape| escaping.contains(&escape.overlay));
        for overlay in escaping {
            if self.escapes.paint_window(overlay).is_some() {
                continue;
            }
            self.escapes.next += 1;
            let id = self.escapes.next;
            self.escapes.held.push(Escape {
                overlay,
                id,
                paint: WindowId::next(),
                shown: None,
            });
        }
    }

    fn escape_id(&self, overlay: NodeOf<OverlayNode>) -> Option<u64> {
        self.escapes
            .held
            .iter()
            .find(|escape| escape.overlay == overlay)
            .map(|escape| escape.id)
    }

    fn painting_bounds(
        &self,
        paint: WindowId,
        overlay: NodeOf<OverlayNode>,
        to_host: HostTransform,
    ) -> Rect {
        let scrim = self.overlay_scrim(overlay);
        self.escapes
            .paintings
            .iter()
            .find(|(window, _)| *window == paint)
            .map_or(Rect::NOTHING, |(_, painting)| {
                painting
                    .iter()
                    .fold(Rect::NOTHING, |bounds, (display, at)| {
                        let drawn = display.bounds.translate(*at);
                        if !drawn.is_positive() {
                            return bounds;
                        }
                        bounds.union(match display.node == scrim {
                            true => to_host.rect(drawn),
                            false => drawn,
                        })
                    })
            })
    }

    pub fn escaped_layers(&self) -> Vec<EscapedLayer> {
        let layers = self.layers();
        let mut escaped = Vec::new();
        for layer in layers.iter() {
            if !layer.escapes {
                continue;
            }
            let (Some(overlay), Some(space)) = (layer.overlay, self.layer_space(layer.window))
            else {
                continue;
            };
            let Some(id) = self.escape_id(overlay) else {
                continue;
            };
            let Some(paint) = self.escapes.paint_window(overlay) else {
                continue;
            };
            let to_host = space.to_host;
            let node = self.arena.get_as::<OverlayNode>(overlay);
            let parent = std::iter::successors(layer.parent, |at| layers[*at].parent)
                .find(|at| layers[*at].escapes)
                .and_then(|at| layers[at].overlay)
                .and_then(|parent| self.escape_id(parent));
            let anchor = node.anchor_rect.unwrap_or(Rect::NOTHING);
            let placement = match node.placement() {
                Placement::Edge(edge) => LayerPlacement::Edge(edge),
                _ => LayerPlacement::Anchored(match parent {
                    Some(_) => to_host.rect(anchor),
                    None => anchor,
                }),
            };
            let painted: Vec<Rect> = self
                .layer_paints(layer)
                .into_iter()
                .map(|rect| to_host.rect(rect))
                .collect();
            let content = layer.content.and_then(|content| self.node_rect(content));
            let floating = match layer.policy {
                LayerPolicy::Inert => Vec::new(),
                _ => content.map(|rect| to_host.rect(rect)).into_iter().collect(),
            };
            let scrims = self
                .layer_scrim(layer)
                .map(|(rect, color)| (to_host.rect(rect), color))
                .into_iter()
                .collect();
            let drawn = painted.iter().fold(
                self.painting_bounds(paint, overlay, to_host),
                |bounds, rect| bounds.union(*rect),
            );
            let drawn = drawn.intersect(space.bounds);
            let painted: Vec<Rect> = drawn.is_positive().then_some(drawn).into_iter().collect();
            let rect = match node.placement() {
                Placement::Edge(_) => space.bounds,
                _ => drawn,
            };
            escaped.push(EscapedLayer {
                id,
                parent,
                window: layer.window,
                placement,
                rect: match rect.is_positive() {
                    true => rect,
                    false => Rect::from_min_size(rect.min, Vec2::ZERO),
                },
                painted,
                floating,
                scrims,
                dismiss_on_outside_press: layer.dismissable && layer.policy != LayerPolicy::Inert,
                keyboard: layer.traps,
                blocks_owner: matches!(
                    layer.policy,
                    LayerPolicy::BlocksAll | LayerPolicy::BlocksParent
                ),
            });
        }
        escaped
    }

    pub(crate) fn escape_holding(&self, node: NodeId) -> Option<u64> {
        let mut current = Some(node);
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            let id = current?;
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id)
                && let Some(escape) = self
                    .escapes
                    .held
                    .iter()
                    .find(|escape| escape.overlay == overlay)
            {
                return Some(escape.id);
            }
            if Some(id) == self.root {
                return None;
            }
            current = self.arena.parent(id);
        }
        None
    }

    pub fn escaped_layer_of(&self, node: NodeId) -> Option<(u64, HostTransform)> {
        let mut current = Some(node);
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            let id = current?;
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(id)
                && let Some(escape) = self
                    .escapes
                    .held
                    .iter()
                    .find(|escape| escape.overlay == overlay)
            {
                let window = self
                    .layers()
                    .iter()
                    .find(|layer| layer.overlay == Some(overlay))?
                    .window;
                return Some((escape.id, self.layer_space(window)?.to_host));
            }
            if Some(id) == self.root {
                return None;
            }
            current = self.arena.parent(id);
        }
        None
    }

    pub(crate) fn publish_escapes(&mut self, ctx: &Context, damaged: &dyn Fn(WindowId) -> bool) {
        let layers = self.escaped_layers();
        let mut outputs = Vec::with_capacity(layers.len());
        let mut targets = Vec::with_capacity(layers.len());
        for layer in layers {
            let Some(space) = self.layer_space(layer.window) else {
                continue;
            };
            let Some(escape) = self
                .escapes
                .held
                .iter()
                .find(|escape| escape.id == layer.id)
            else {
                continue;
            };
            let paint = escape.paint;
            let changed = damaged(paint) || escape.shown.as_ref() != Some(&layer);
            let painting = self
                .escapes
                .paintings
                .iter()
                .find(|(window, _)| *window == paint)
                .map(|(_, painting)| painting.clone())
                .unwrap_or_default();
            let to_host = space.to_host;
            let scrim = self.overlay_scrim(escape.overlay);
            let offset = to_host.origin - layer.rect.min;
            let size = layer.rect.size();
            let scale = to_host.scale.x;
            let shown: Vec<Shown> = painting
                .iter()
                .map(|(display, at)| {
                    let (offset, scale) = match display.node == scrim {
                        true => (offset, scale),
                        false => (-layer.rect.min.to_vec2(), 1.0),
                    };
                    Shown::Group(
                        Group::Transform { offset, scale },
                        Rc::new([Shown::Display(*at, Rc::clone(display))]),
                    )
                })
                .collect();
            let output = WindowOutput {
                id: paint,
                size,
                layers: Rc::new(shown),
                window: WindowState {
                    appearance: crate::window::Appearance {
                        clear_color: Color32::TRANSPARENT,
                        ..crate::window::Appearance::default()
                    },
                    ..WindowState::default()
                },
                changed,
                damage: match changed {
                    true => Region::from(Rect::from_min_size(Pos2::ZERO, size)),
                    false => Region::NOTHING,
                },
                moved: None,
                pixels_per_point: ctx.window_pixels_per_point(layer.window) / scale,
            };
            targets.push((layer.id, layer.window, to_host));
            if let Some(escape) = self
                .escapes
                .held
                .iter_mut()
                .find(|escape| escape.id == layer.id)
            {
                escape.shown = Some(layer.clone());
            }
            outputs.push(EscapedOutput { layer, output });
        }
        ctx.show_escaped(outputs, targets);
    }

    pub(crate) fn dismiss_layers(&mut self, ctx: &Context) {
        for id in ctx.take_layer_dismissals() {
            let Some(overlay) = self
                .escapes
                .held
                .iter()
                .find(|escape| escape.id == id)
                .map(|escape| escape.overlay)
            else {
                continue;
            };
            if self.escapes.asked.contains(&overlay) || !self.is_overlay_open(overlay.id()) {
                continue;
            }
            crate::current::run_in(self, |document| {
                document.request_dismiss(overlay, Dismiss::OutsidePress);
            });
        }
    }
}

pub fn composite_layers(
    main: &WindowOutput,
    escaped: &[EscapedOutput],
    space: LayerSpace,
    region: Color32,
) -> WindowOutput {
    let corner = space.bounds.min.to_vec2();
    let to_host = space.to_host;
    let shown = Rect::from_min_size(Pos2::ZERO, main.size);
    let backdrop = Shown::Shape(Shape::Rect {
        rect: to_host.rect(shown).translate(-corner),
        corner_radius: Corners::default(),
        stroke_width: 0.0,
        color: region,
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    });
    let placed = Shown::Group(
        Group::Transform {
            offset: to_host.origin.to_vec2() - corner,
            scale: to_host.scale.x,
        },
        Rc::clone(&main.layers).iter().cloned().collect(),
    );
    let layers = std::iter::once(backdrop)
        .chain(std::iter::once(placed))
        .chain(escaped.iter().map(|escaped| {
            Shown::Group(
                Group::Transform {
                    offset: escaped.layer.rect.min.to_vec2() - corner,
                    scale: 1.0,
                },
                escaped.output.layers.iter().cloned().collect(),
            )
        }))
        .collect();
    WindowOutput {
        id: main.id,
        size: space.bounds.size(),
        layers: Rc::new(layers),
        window: main.window.clone(),
        changed: true,
        damage: Region::from(Rect::from_min_size(Pos2::ZERO, space.bounds.size())),
        moved: None,
        pixels_per_point: main.pixels_per_point,
    }
}
