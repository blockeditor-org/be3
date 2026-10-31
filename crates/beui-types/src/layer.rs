use geometry::{Pos2, Rect, Vec2, pos2};

use crate::color::Color32;
use crate::window::WindowId;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LayerPolicy {
    Inert,
    Pass,
    BlocksParent,
    #[default]
    BlocksAll,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AboveEmbeds {
    Reaches,
    Paints,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LayerLevel {
    #[default]
    Normal,
    Raised,
    Top,
    Lock,
}

impl LayerLevel {
    pub fn stacked(self, policy: LayerPolicy) -> LayerLevel {
        match (self, policy) {
            (LayerLevel::Normal, LayerPolicy::BlocksAll) => LayerLevel::Raised,
            (level, _) => level,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dismiss {
    Escape,
    Back,
    OutsidePress,
    WithParent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    At,
    Above,
    Over(u16),
    Around,
    BelowStart,
    RightStart,
    Center,
    Fill,
    InsideTop,
    InsideTopStart,
    InsideTopEnd,
    InsideBottom,
    InsideBottomStart,
    InsideBottomEnd,
    CenterOn,
    Popover { gap: u16, margin: u16 },
    Edge(LayerEdge),
}

impl Placement {
    pub fn escapes(self) -> bool {
        match self {
            Placement::Above
            | Placement::Over(_)
            | Placement::Around
            | Placement::BelowStart
            | Placement::RightStart
            | Placement::Center
            | Placement::CenterOn
            | Placement::Popover { .. }
            | Placement::Edge(_) => true,
            Placement::At
            | Placement::Fill
            | Placement::InsideTop
            | Placement::InsideTopStart
            | Placement::InsideTopEnd
            | Placement::InsideBottom
            | Placement::InsideBottomStart
            | Placement::InsideBottomEnd => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LayerEdge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct HostTransform {
    pub origin: Pos2,
    pub scale: Vec2,
}

impl HostTransform {
    pub const IDENTITY: Self = Self {
        origin: Pos2::ZERO,
        scale: Vec2::splat(1.0),
    };

    pub fn point(self, point: Pos2) -> Pos2 {
        pos2(
            self.origin.x + point.x * self.scale.x,
            self.origin.y + point.y * self.scale.y,
        )
    }

    pub fn rect(self, rect: Rect) -> Rect {
        Rect::from_points(&[self.point(rect.min), self.point(rect.max)])
    }

    pub fn inverse_point(self, point: Pos2) -> Pos2 {
        pos2(
            (point.x - self.origin.x) / self.scale.x,
            (point.y - self.origin.y) / self.scale.y,
        )
    }

    pub fn inverse_rect(self, rect: Rect) -> Rect {
        Rect::from_points(&[self.inverse_point(rect.min), self.inverse_point(rect.max)])
    }

    pub fn inverse_vec(self, by: Vec2) -> Vec2 {
        Vec2::new(by.x / self.scale.x, by.y / self.scale.y)
    }
}

impl Default for HostTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LayerSpace {
    pub to_host: HostTransform,
    pub bounds: Rect,
}

impl LayerSpace {
    pub fn area(self) -> Rect {
        self.to_host.inverse_rect(self.bounds)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LayerPlacement {
    Anchored(Rect),
    Edge(LayerEdge),
}

#[derive(Clone, PartialEq, Debug)]
pub struct EscapedLayer {
    pub id: u64,
    pub parent: Option<u64>,
    pub window: WindowId,
    pub placement: LayerPlacement,
    pub rect: Rect,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub scrims: Vec<(Rect, Color32)>,
    pub dismiss_on_outside_press: bool,
    pub trigger: Option<Rect>,
    pub keyboard: bool,
    pub blocks_owner: bool,
}

pub fn resolve_rect(
    viewport: Rect,
    anchor_rect: Rect,
    placement: Placement,
    content_size: Vec2,
) -> Rect {
    if placement == Placement::At {
        return Rect::from_min_size(anchor_rect.min, content_size);
    }
    if let Placement::Over(inset) = placement {
        let inset = f32::from(inset);
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let origin = pos2(
            (anchor_rect.left() - inset).min(right).max(viewport.left()),
            (anchor_rect.top() - inset).min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::Around {
        return Rect::from_center_size(anchor_rect.center(), content_size);
    }
    if placement == Placement::Above {
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let y = match anchor_rect.top() - content_size.y >= viewport.top() {
            true => anchor_rect.top() - content_size.y,
            false => anchor_rect.bottom(),
        };
        let origin = pos2(
            (anchor_rect.center().x - content_size.x / 2.0)
                .min(right)
                .max(viewport.left()),
            y.min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::CenterOn {
        let size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let centred = Rect::from_center_size(anchor_rect.center(), size);
        let x = centred
            .left()
            .min(viewport.right() - size.x)
            .max(viewport.left());
        let y = centred
            .top()
            .min(viewport.bottom() - size.y)
            .max(viewport.top());
        return Rect::from_min_size(pos2(x, y), size);
    }
    if let Placement::Popover { gap, margin } = placement {
        let gap = f32::from(gap);
        let centre = anchor_rect.center().x;
        let reach = Rect::from_min_max(
            pos2(centre - content_size.x / 2.0, anchor_rect.top() - gap),
            pos2(centre + content_size.x / 2.0, anchor_rect.bottom() + gap),
        );
        let room = viewport.shrink(f32::from(margin));
        return resolve_rect(room, reach, Placement::BelowStart, content_size);
    }
    if let Placement::Fill | Placement::Edge(_) = placement {
        return viewport;
    }
    if placement == Placement::Center {
        let content_size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let origin = pos2(
            viewport.left() + (viewport.width() - content_size.x) / 2.0,
            viewport.top() + (viewport.height() - content_size.y) / 2.0,
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTopStart
    | Placement::InsideTopEnd
    | Placement::InsideBottomStart
    | Placement::InsideBottomEnd = placement
    {
        let y = match placement {
            Placement::InsideTopStart | Placement::InsideTopEnd => anchor_rect.top(),
            _ => (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
        };
        let x = match placement {
            Placement::InsideTopStart | Placement::InsideBottomStart => anchor_rect.left(),
            _ => (anchor_rect.right() - content_size.x).max(anchor_rect.left()),
        };
        let origin = pos2(x, y);
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTop | Placement::InsideBottom = placement {
        let y = match placement {
            Placement::InsideTop => anchor_rect.top(),
            _ => anchor_rect.bottom() - content_size.y,
        };
        let origin = pos2(
            anchor_rect.center().x - content_size.x / 2.0,
            y.clamp(
                anchor_rect.top(),
                (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
            ),
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    let mut content_size = content_size.min(viewport.size());
    if placement == Placement::BelowStart {
        let below = viewport.bottom() - anchor_rect.bottom();
        let above = anchor_rect.top() - viewport.top();
        if content_size.y > below && content_size.y > above && below.max(above) > 0.0 {
            content_size.y = below.max(above);
            let top = match below >= above {
                true => anchor_rect.bottom(),
                false => anchor_rect.top() - content_size.y,
            };
            let left = anchor_rect
                .left()
                .min(viewport.right() - content_size.x)
                .max(viewport.left());
            return Rect::from_min_size(pos2(left, top), content_size);
        }
    }
    let mut origin = match placement {
        Placement::BelowStart => pos2(anchor_rect.left(), anchor_rect.bottom()),
        Placement::RightStart => pos2(anchor_rect.right(), anchor_rect.top()),
        Placement::At
        | Placement::Above
        | Placement::Over(_)
        | Placement::Around
        | Placement::Center
        | Placement::Fill
        | Placement::InsideTop
        | Placement::InsideTopStart
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomStart
        | Placement::InsideBottomEnd
        | Placement::CenterOn
        | Placement::Popover { .. }
        | Placement::Edge(_) => {
            unreachable!("a centred or pinned overlay is placed above")
        }
    };
    if origin.x + content_size.x > viewport.right() {
        origin.x = match placement {
            Placement::RightStart => anchor_rect.left() - content_size.x,
            _ => (viewport.right() - content_size.x).max(viewport.left()),
        };
    }
    if origin.y + content_size.y > viewport.bottom() {
        origin.y = match placement {
            Placement::BelowStart => anchor_rect.top() - content_size.y,
            _ => (viewport.bottom() - content_size.y).max(viewport.top()),
        };
    }
    origin.x = origin.x.max(viewport.left());
    origin.y = origin.y.max(viewport.top());
    Rect::from_min_size(origin, content_size)
}

#[derive(Clone, PartialEq)]
pub enum OverlayAnchor {
    Node(crate::node::NodeRef),
    Point(geometry::Pos2),
    Rect(Rect),
    Pinned {
        node: crate::node::NodeRef,
        rect: Rect,
    },
    Band {
        node: crate::node::NodeRef,
        band: crate::node::NodeRef,
    },
}

impl OverlayAnchor {
    pub fn node(&self) -> Option<crate::node::NodeRef> {
        match self {
            OverlayAnchor::Node(node)
            | OverlayAnchor::Pinned { node, .. }
            | OverlayAnchor::Band { node, .. } => Some(*node),
            OverlayAnchor::Point(_) | OverlayAnchor::Rect(_) => None,
        }
    }
}

pub fn banded(node: Rect, band: Rect, placement: Placement) -> Rect {
    match placement {
        Placement::BelowStart | Placement::Above | Placement::Popover { .. } => {
            Rect::from_min_max(pos2(node.min.x, band.min.y), pos2(node.max.x, band.max.y))
        }
        Placement::RightStart => {
            Rect::from_min_max(pos2(band.min.x, node.min.y), pos2(band.max.x, node.max.y))
        }
        Placement::At
        | Placement::Over(_)
        | Placement::Around
        | Placement::Center
        | Placement::Fill
        | Placement::InsideTop
        | Placement::InsideTopStart
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomStart
        | Placement::InsideBottomEnd
        | Placement::CenterOn
        | Placement::Edge(_) => node,
    }
}

impl beui_tree::reactive::IntoProp<OverlayAnchor> for &crate::node::NodeRef {
    fn into_prop(self) -> beui_tree::reactive::Prop<OverlayAnchor> {
        beui_tree::reactive::Prop::Static(OverlayAnchor::Node(*self))
    }
}

impl beui_tree::reactive::IntoProp<OverlayAnchor> for crate::node::NodeRef {
    fn into_prop(self) -> beui_tree::reactive::Prop<OverlayAnchor> {
        beui_tree::reactive::Prop::Static(OverlayAnchor::Node(self))
    }
}

impl Default for OverlayAnchor {
    fn default() -> Self {
        OverlayAnchor::Point(geometry::Pos2::ZERO)
    }
}
