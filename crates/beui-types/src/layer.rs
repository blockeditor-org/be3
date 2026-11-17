use be_lib_geometry::{Pos2, Rect, Vec2, pos2};

use crate::color::Color32;
use crate::window::WindowId;

/// How an open layer treats input around it.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LayerPolicy {
    /// Takes no input at all (a tooltip, a drag preview).
    Inert,
    /// Takes the pointer where its content is; everything around stays live (a window, a
    /// toast, a popover).
    Pass,
    /// Modal to the node it hangs from alone: a press there asks the layer to close, the
    /// focus cannot enter it and assistive technology skips it; everything else stays
    /// live (a modal `DockDialog`).
    BlocksParent,
    /// A press anywhere outside is shut out and asks the layer to close, and nothing
    /// beneath is hovered (a menu, a dialog). With a scrim, or at `LayerLevel::Lock`, it is
    /// modal to assistive technology too: what it covers is hidden and an `Embed` beneath
    /// hears it is inert.
    #[default]
    BlocksAll,
}

/// Marks a node that must stay above `Embed`s painted before it (`above_embeds`). The marks
/// join the overlays' rects the host reads, and the host draws the document over the embed
/// there, so mark only what paints a few pixels: a mark over a large empty area hides the
/// embed under it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AboveEmbeds {
    /// Keeps its rect for its own input (a dock splitter's reach over a child editor).
    Reaches,
    /// Also has the host draw the document's pixels there (the splitter's line and focus
    /// ring), so it counts in `overlay_painted_rects` too.
    Paints,
    /// Paints, and stays above the windows a host opens over the document, as an overlay
    /// does (a raised dock bar's band), where `Reaches` and `Paints` stay under them.
    Raised,
}

/// Puts a layer and everything above it over every layer of a lower level.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LayerLevel {
    #[default]
    Normal,
    /// Over its window but under menus and dialogs (a dock's floating tool bars).
    Raised,
    /// Over every window and dialog (a drag preview, toasts, so a toast stays pressable
    /// while a dialog is open).
    Top,
    /// The lock screen.
    Lock,
}

impl LayerLevel {
    /// The level a layer stacks at: a `Normal` layer that blocks everything counts as
    /// `Raised`, so a menu or modal still opens over `Raised` bars.
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

/// The side of its anchor a `Placement::Beside` overlay opens on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnchorSide {
    Above,
    Below,
    Left,
    Right,
}

impl AnchorSide {
    /// Whether it opens along the vertical axis (`Above`, `Below`).
    pub fn vertical(self) -> bool {
        matches!(self, AnchorSide::Above | AnchorSide::Below)
    }

    /// The side across the anchor from this one, where it flips to.
    pub fn opposite(self) -> AnchorSide {
        match self {
            AnchorSide::Above => AnchorSide::Below,
            AnchorSide::Below => AnchorSide::Above,
            AnchorSide::Left => AnchorSide::Right,
            AnchorSide::Right => AnchorSide::Left,
        }
    }
}

/// Where a `Placement::Beside` overlay lines up along its side: `Start` with the anchor's
/// left (or top, beside it) edge, `Center` on its centre, `End` with its right (or bottom).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnchorAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    At,
    Over(u16),
    Around,
    /// Opens on `side` of its anchor, `gap` points away from it, lined up by `align`, and
    /// keeps `margin` points inside its window's edges. It flips to the other side when only
    /// that side has room, and when neither has, takes the roomier one and shrinks (scrolling
    /// what it holds), so it never covers its anchor: completions, tooltips and popovers keep
    /// their input or button in view. Along its side it shifts to stay in the window.
    /// `layer::room` is the room it gets. The constants (`BELOW_START`, `LEFT_END`, ...) name
    /// every side and alignment with no gap; `gap` and `margin` set the rest.
    Beside {
        side: AnchorSide,
        align: AnchorAlign,
        gap: u16,
        margin: u16,
    },
    /// A menu or a select's list: below its anchor, else above it, and where it fits on
    /// neither side, over the anchor and as low as the window allows rather than cut short
    /// to scroll. Only menus cover what opened them; everything else opens `Beside`.
    Menu,
    Center,
    Fill,
    InsideTop,
    InsideTopStart,
    InsideTopEnd,
    InsideBottom,
    InsideBottomStart,
    InsideBottomEnd,
    CenterOn,
    Edge(LayerEdge),
}

impl Placement {
    pub const ABOVE_START: Placement = Placement::beside(AnchorSide::Above, AnchorAlign::Start);
    pub const ABOVE_CENTER: Placement = Placement::beside(AnchorSide::Above, AnchorAlign::Center);
    pub const ABOVE_END: Placement = Placement::beside(AnchorSide::Above, AnchorAlign::End);
    pub const BELOW_START: Placement = Placement::beside(AnchorSide::Below, AnchorAlign::Start);
    pub const BELOW_CENTER: Placement = Placement::beside(AnchorSide::Below, AnchorAlign::Center);
    pub const BELOW_END: Placement = Placement::beside(AnchorSide::Below, AnchorAlign::End);
    pub const LEFT_START: Placement = Placement::beside(AnchorSide::Left, AnchorAlign::Start);
    pub const LEFT_CENTER: Placement = Placement::beside(AnchorSide::Left, AnchorAlign::Center);
    pub const LEFT_END: Placement = Placement::beside(AnchorSide::Left, AnchorAlign::End);
    pub const RIGHT_START: Placement = Placement::beside(AnchorSide::Right, AnchorAlign::Start);
    pub const RIGHT_CENTER: Placement = Placement::beside(AnchorSide::Right, AnchorAlign::Center);
    pub const RIGHT_END: Placement = Placement::beside(AnchorSide::Right, AnchorAlign::End);

    const fn beside(side: AnchorSide, align: AnchorAlign) -> Placement {
        Placement::Beside {
            side,
            align,
            gap: 0,
            margin: 0,
        }
    }

    /// This `Beside` placement `gap` points away from its anchor; any other unchanged.
    pub const fn gap(self, gap: u16) -> Placement {
        match self {
            Placement::Beside {
                side,
                align,
                margin,
                ..
            } => Placement::Beside {
                side,
                align,
                gap,
                margin,
            },
            other => other,
        }
    }

    /// This `Beside` placement kept `margin` points inside its window's edges; any other
    /// unchanged.
    pub const fn margin(self, margin: u16) -> Placement {
        match self {
            Placement::Beside {
                side, align, gap, ..
            } => Placement::Beside {
                side,
                align,
                gap,
                margin,
            },
            other => other,
        }
    }

    /// The side a `Beside` placement opens on, and `Below` for a `Menu`.
    pub fn side(self) -> Option<AnchorSide> {
        match self {
            Placement::Beside { side, .. } => Some(side),
            Placement::Menu => Some(AnchorSide::Below),
            _ => None,
        }
    }

    /// Whether an overlay with this placement leaves its window when the window has a
    /// `LayerSpace`: anchored placements, `Center` and `Edge` sheets do; `At` (windows),
    /// `Fill` and the `Inside*` placements (toasts, `Floating`) stay. An escaping `Center`
    /// is laid out in host space and centred on its window.
    pub fn escapes(self) -> bool {
        match self {
            Placement::Over(_)
            | Placement::Around
            | Placement::Beside { .. }
            | Placement::Menu
            | Placement::Center
            | Placement::CenterOn
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

/// Lets a window's escaping overlays out of its rectangle. `to_host` maps the window's
/// points to the host's; `bounds` is where layers may go, in host coordinates. An escaping
/// overlay is laid out at full size in host units whatever the window's scale, so a menu
/// from a region shown at half size is as large as any other.
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scrim {
    pub rect: Rect,
    pub color: Color32,
    pub radius: f32,
}

/// One escaped layer, painted into a surface of its own. `rect`, `painted`, `floating`
/// and `scrims` are in host coordinates; `placement` is in window points for a layer of the
/// window and host coordinates for one nested in another escaped layer (`parent`).
/// `blocks_owner` is false for a layer that blocks only its parent: the document itself
/// blocks it there and never asks it to close on a press outside.
#[derive(Clone, PartialEq, Debug)]
pub struct EscapedLayer {
    pub id: u64,
    pub parent: Option<u64>,
    pub window: WindowId,
    pub placement: LayerPlacement,
    pub rect: Rect,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub scrims: Vec<Scrim>,
    pub dismiss_on_outside_press: bool,
    pub trigger: Option<Rect>,
    pub keyboard: bool,
    pub blocks_owner: bool,
}

/// Where an overlay of `content_size` at `placement` against `anchor_rect` goes in
/// `viewport`.
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
    if let Placement::Beside {
        side,
        align,
        gap,
        margin,
    } = placement
    {
        return beside(viewport, anchor_rect, side, align, gap, margin, content_size);
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
    // A `Menu`, the only placement left.
    let content_size = content_size.min(viewport.size());
    let below = viewport.bottom() - anchor_rect.bottom();
    let above = anchor_rect.top() - viewport.top();
    let left = anchor_rect
        .left()
        .min(viewport.right() - content_size.x)
        .max(viewport.left());
    let top = match (content_size.y <= below, content_size.y <= above) {
        (true, _) => anchor_rect.bottom(),
        (false, true) => anchor_rect.top() - content_size.y,
        (false, false) => (viewport.bottom() - content_size.y).max(viewport.top()),
    };
    Rect::from_min_size(pos2(left, top), content_size)
}

/// The window less `margin` on every side, or its centre where it is too small for that.
fn inside_margin(viewport: Rect, margin: u16) -> Rect {
    let margin = f32::from(margin)
        .min(viewport.width().max(0.0) / 2.0)
        .min(viewport.height().max(0.0) / 2.0);
    viewport.shrink(margin)
}

/// A vector's length along the axis a side opens on (`main`) or across it.
fn along(by: Vec2, vertical: bool) -> f32 {
    match vertical {
        true => by.y,
        false => by.x,
    }
}

fn across(by: Vec2, vertical: bool) -> f32 {
    along(by, !vertical)
}

/// The room before (above or left of) and after (below or right of) an anchor along the axis
/// `side` opens on, less `gap`.
fn room_around(area: Rect, anchor: Rect, side: AnchorSide, gap: f32) -> (f32, f32) {
    match side.vertical() {
        true => (
            anchor.top() - gap - area.top(),
            area.bottom() - anchor.bottom() - gap,
        ),
        false => (
            anchor.left() - gap - area.left(),
            area.right() - anchor.right() - gap,
        ),
    }
}

fn beside(
    viewport: Rect,
    anchor: Rect,
    side: AnchorSide,
    align: AnchorAlign,
    gap: u16,
    margin: u16,
    content_size: Vec2,
) -> Rect {
    let area = inside_margin(viewport, margin);
    let gap = f32::from(gap);
    let vertical = side.vertical();
    let mut size = content_size.min(area.size()).max(Vec2::ZERO);
    let (before, after) = room_around(area, anchor, side, gap);
    let first = matches!(side, AnchorSide::Above | AnchorSide::Left);
    let (wanted, other) = match first {
        true => (before, after),
        false => (after, before),
    };
    let length = along(size, vertical);
    let opens_before = match (length <= wanted, length <= other) {
        (true, _) => first,
        (false, true) => !first,
        // Neither side has room: the roomier one, shrunk to it. An anchor that leaves no room
        // on either side is covered rather than the overlay shrunk away.
        (false, false) => {
            let roomier = wanted.max(other);
            if roomier > 0.0 {
                match vertical {
                    true => size.y = roomier,
                    false => size.x = roomier,
                }
            }
            match wanted >= other {
                true => first,
                false => !first,
            }
        }
    };
    let length = along(size, vertical);
    let main = match opens_before {
        true => along(anchor.min.to_vec2(), vertical) - gap - length,
        false => along(anchor.max.to_vec2(), vertical) + gap,
    };
    let main = main
        .min(along(area.max.to_vec2(), vertical) - length)
        .max(along(area.min.to_vec2(), vertical));
    let width = across(size, vertical);
    let (low, high) = (
        across(anchor.min.to_vec2(), vertical),
        across(anchor.max.to_vec2(), vertical),
    );
    let cross = match align {
        AnchorAlign::Start => low,
        AnchorAlign::Center => (low + high - width) / 2.0,
        AnchorAlign::End => high - width,
    };
    let cross = cross
        .min(across(area.max.to_vec2(), vertical) - width)
        .max(across(area.min.to_vec2(), vertical));
    let origin = match vertical {
        true => pos2(cross, main),
        false => pos2(main, cross),
    };
    Rect::from_min_size(origin, size)
}

/// The room an overlay at `placement` gets beside `anchor` in `viewport`, like Floating UI's
/// `size`: what `resolve_rect` shrinks it to at most. A `Beside` overlay has the roomier of its
/// side and the other along the axis it opens on (less its gap) and the whole window across
/// it, less its margin either way; a menu, which covers its anchor where neither side has
/// room, the whole window; an `Inside*` overlay the anchor's part of the window; anything
/// else the window.
pub fn room(viewport: Rect, anchor_rect: Rect, placement: Placement) -> Vec2 {
    let room = match placement {
        Placement::Beside {
            side, gap, margin, ..
        } => {
            let area = inside_margin(viewport, margin);
            let (before, after) = room_around(area, anchor_rect, side, f32::from(gap));
            let roomiest = before.max(after);
            match side.vertical() {
                true => Vec2::new(area.width(), roomiest),
                false => Vec2::new(roomiest, area.height()),
            }
        }
        Placement::InsideTop
        | Placement::InsideTopStart
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomStart
        | Placement::InsideBottomEnd => anchor_rect.intersect(viewport).size(),
        Placement::At
        | Placement::Over(_)
        | Placement::Around
        | Placement::Menu
        | Placement::Center
        | Placement::Fill
        | Placement::CenterOn
        | Placement::Edge(_) => viewport.size(),
    };
    room.max(Vec2::ZERO).min(viewport.size().max(Vec2::ZERO))
}

#[derive(Clone, PartialEq)]
pub enum OverlayAnchor {
    Node(crate::node::NodeRef),
    Point(be_lib_geometry::Pos2),
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

/// The anchor a `Band` gives: `node` pushed out to `band`'s edges along the axis `placement`
/// opens on, so it opens just past the band while lining up with the node.
pub fn banded(node: Rect, band: Rect, placement: Placement) -> Rect {
    match placement.side().map(AnchorSide::vertical) {
        Some(true) => Rect::from_min_max(pos2(node.min.x, band.min.y), pos2(node.max.x, band.max.y)),
        Some(false) => {
            Rect::from_min_max(pos2(band.min.x, node.min.y), pos2(band.max.x, node.max.y))
        }
        None => node,
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
        OverlayAnchor::Point(be_lib_geometry::Pos2::ZERO)
    }
}

#[cfg(test)]
mod tests;
