use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::context::{Context, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::font::{FontId, TextLayout};
use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
use beui_core::icons::{ICON_MOVE, ICON_ROTATE_3D, ICON_ROTATE_CW_SQUARE, ICON_SPLINE};
use beui_core::painter::{Corners, Shape};
use beui_core::window::{WindowId, WindowState};
use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec3};

use crate::head::Ray;
use crate::panel::{Hit, MOST_BEND, POINTS_PER_METRE, Panel};

pub const GAP: f32 = 0.012;
pub const ARM: f32 = 0.045;
pub const THICKNESS: f32 = 0.007;
pub const SLOP: f32 = 0.005;
pub const BAR_GAP: f32 = 0.016;
pub const BAR: f32 = 0.04;
pub const BUTTON: f32 = 0.032;
pub const TRACK: f32 = 0.14;
pub const KNOB: f32 = 0.018;
pub const ICON: f32 = 0.018;
pub const FAINT: f32 = 0.35;
pub const TURN_GAIN: f32 = 2.0;
pub const SMALLEST_SCALE: f32 = 0.2;
pub const LARGEST_SCALE: f32 = 8.0;
pub const LIFT: f32 = 0.001;

const HANDLE: Color32 = Color32::from_rgba_unmultiplied(214, 220, 232, 220);
const ACCENT: Color32 = Color32::from_rgb(132, 170, 255);
const PILL: Color32 = Color32::from_rgba_unmultiplied(30, 34, 43, 230);
const BUTTON_HOT: Color32 = Color32::from_rgb(60, 68, 86);
const BUTTON_ON: Color32 = Color32::from_rgb(52, 84, 156);
const GLYPH: Color32 = Color32::from_rgb(226, 231, 240);
const GROOVE: Color32 = Color32::from_rgb(78, 86, 104);
const GROOVE_HEIGHT: f32 = 0.004;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl Corner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomRight,
        Self::BottomLeft,
    ];

    pub fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomRight => Self::TopLeft,
            Self::BottomLeft => Self::TopRight,
        }
    }

    fn signs(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (-1.0, -1.0),
            Self::TopRight => (1.0, -1.0),
            Self::BottomRight => (1.0, 1.0),
            Self::BottomLeft => (-1.0, 1.0),
        }
    }

    pub fn of(self, size: Vec2) -> Pos2 {
        let (x, y) = self.signs();
        pos2(
            if x > 0.0 { size.x } else { 0.0 },
            if y > 0.0 { size.y } else { 0.0 },
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Part {
    Scale(Corner),
    Move,
    Turn,
    Roll,
    Bend,
    Slider,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gesture {
    Scale {
        original: Panel,
        anchor: Pos2,
        grab: Pos2,
    },
    Move {
        window: WindowId,
    },
    Turn {
        window: WindowId,
        rotation: Quat,
        start: Ray,
    },
    Roll {
        original: Panel,
        start: f32,
    },
    Bend {
        original: Panel,
    },
    Press {
        window: WindowId,
    },
}

impl Gesture {
    pub fn window(&self) -> WindowId {
        match self {
            Self::Scale { original, .. }
            | Self::Roll { original, .. }
            | Self::Bend { original } => original.window,
            Self::Move { window } | Self::Turn { window, .. } | Self::Press { window } => *window,
        }
    }

    pub fn part(&self) -> Part {
        match self {
            Self::Scale {
                original, anchor, ..
            } => Part::Scale(
                Corner::ALL
                    .into_iter()
                    .find(|corner| corner.opposite().of(original.size) == *anchor)
                    .unwrap_or(Corner::BottomRight),
            ),
            Self::Move { .. } => Part::Move,
            Self::Turn { .. } => Part::Turn,
            Self::Roll { .. } => Part::Roll,
            Self::Bend { .. } => Part::Slider,
            Self::Press { .. } => Part::Bend,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Chrome {
    pub hovered: Option<(WindowId, Part)>,
    pub gesture: Option<Gesture>,
    pub slider: Option<WindowId>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    pub metres: Vec2,
    pub hovered: Option<Part>,
    pub held: Option<Part>,
    pub slider: bool,
    pub bend: f32,
    pub pixels_per_point: f32,
}

pub struct Layout {
    pub parts: Vec<(Part, Rect)>,
    pub brackets: Vec<(Corner, [Rect; 2])>,
    pub pill: Rect,
    pub track: Rect,
    pub bounds: Rect,
}

fn between(a: Pos2, b: Pos2) -> Rect {
    Rect::from_min_max(
        pos2(a.x.min(b.x), a.y.min(b.y)),
        pos2(a.x.max(b.x), a.y.max(b.y)),
    )
}

pub fn layout(metres: Vec2, slider: bool) -> Layout {
    let mut parts = Vec::new();
    let mut brackets = Vec::new();
    let mut bounds = Rect::NOTHING;
    let reach = GAP + THICKNESS;
    for corner in Corner::ALL {
        let (x, y) = corner.signs();
        let at = corner.of(metres);
        let outer = pos2(at.x + x * reach, at.y + y * reach);
        let across = between(outer, pos2(outer.x - x * ARM, outer.y - y * THICKNESS));
        let down = between(outer, pos2(outer.x - x * THICKNESS, outer.y - y * ARM));
        for arm in [across, down] {
            let hit = arm.expand(SLOP);
            bounds = bounds.union(hit);
            parts.push((Part::Scale(corner), hit));
        }
        brackets.push((corner, [across, down]));
    }
    let padding = (BAR - BUTTON) / 2.0;
    let top = metres.y + BAR_GAP;
    let middle = top + BAR / 2.0;
    let centre = metres.x / 2.0;
    let button = |x: f32| Rect::from_center_size(pos2(x, middle), vec2(BUTTON, BUTTON));
    let buttons = [Part::Move, Part::Turn, Part::Roll, Part::Bend];
    let step = BUTTON + padding;
    let first = centre - step * (buttons.len() - 1) as f32 / 2.0;
    let buttons = buttons.map(|part| {
        let at = buttons.iter().position(|each| *each == part).unwrap_or(0);
        (part, button(first + step * at as f32))
    });
    let closed = Rect::from_min_max(
        pos2(buttons[0].1.min.x - padding, top),
        pos2(buttons[buttons.len() - 1].1.max.x + padding, top + BAR),
    );
    let track = Rect::from_min_max(
        pos2(closed.max.x, top),
        pos2(closed.max.x + TRACK, top + BAR),
    );
    let open = Rect::from_min_max(closed.min, pos2(track.max.x + padding, closed.max.y));
    parts.extend(buttons);
    if slider {
        parts.push((Part::Slider, track));
    }
    let bounds = bounds.union(open.expand(SLOP));
    Layout {
        parts,
        brackets,
        pill: if slider { open } else { closed },
        track,
        bounds,
    }
}

pub fn fraction(track: Rect, x: f32) -> f32 {
    let inset = KNOB / 2.0 + 0.004;
    ((x - track.min.x - inset) / (track.width() - 2.0 * inset)).clamp(0.0, 1.0)
}

fn knob(track: Rect, fraction: f32) -> Pos2 {
    let inset = KNOB / 2.0 + 0.004;
    pos2(
        track.min.x + inset + fraction * (track.width() - 2.0 * inset),
        track.center().y,
    )
}

fn metres_of(panel: &Panel) -> Vec2 {
    let metres = panel.metres();
    vec2(metres.x, metres.y)
}

pub fn bounds(panel: &Panel) -> Rect {
    layout(metres_of(panel), true).bounds
}

pub fn part_at(panel: &Panel, pos: Pos2, slider: bool) -> Option<Part> {
    if panel.contains(pos) {
        return None;
    }
    let at = pos2(
        pos.x * panel.metres_per_point,
        pos.y * panel.metres_per_point,
    );
    layout(metres_of(panel), slider)
        .parts
        .into_iter()
        .find(|(_, rect)| rect.contains(at))
        .map(|(part, _)| part)
}

pub fn pick(panels: &[Panel], ray: Ray, slider: Option<WindowId>) -> Option<(Part, Hit)> {
    panels
        .iter()
        .filter_map(|panel| {
            let hit = panel.intersect(ray)?;
            let part = part_at(panel, hit.pos, slider == Some(panel.window))?;
            Some((part, hit))
        })
        .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))
}

pub fn bend_of(original: &Panel, pos: Pos2) -> f32 {
    let track = layout(metres_of(original), true).track;
    fraction(track, pos.x * original.metres_per_point) * MOST_BEND
}

pub fn scaled(original: &Panel, anchor: Pos2, grab: Pos2, pos: Pos2) -> Panel {
    let along = grab - anchor;
    let moved = pos - anchor;
    let factor =
        (moved.x * along.x + moved.y * along.y) / (along.x * along.x + along.y * along.y).max(1e-6);
    let factor = factor.clamp(SMALLEST_SCALE, LARGEST_SCALE);
    let mut panel = *original;
    panel.metres_per_point = original.metres_per_point * factor;
    panel.anchor(anchor, original.frame_at(anchor));
    panel
}

pub fn turned(rotation: Quat, from: Vec3, to: Vec3, head: Quat) -> Quat {
    let (from, to) = (head.inverse() * from, head.inverse() * to);
    let yaw = |direction: Vec3| direction.x.atan2(-direction.z);
    let pitch = |direction: Vec3| direction.normalize_or_zero().y.clamp(-1.0, 1.0).asin();
    let mut yawed = yaw(to) - yaw(from);
    if yawed > PI {
        yawed -= TAU;
    } else if yawed < -PI {
        yawed += TAU;
    }
    let pitched = pitch(to) - pitch(from);
    let turn =
        Quat::from_rotation_y(yawed * TURN_GAIN) * Quat::from_rotation_x(-pitched * TURN_GAIN);
    (head * turn * head.inverse() * rotation).normalize()
}

pub fn roll_angle(original: &Panel, ray: Ray) -> Option<f32> {
    let pos = original.intersect(ray)?.pos;
    let from = pos.to_vec2() - original.size / 2.0;
    (from.length() > 1e-3).then(|| (-from.y).atan2(from.x))
}

pub fn rolled(original: &Panel, start: f32, angle: f32) -> Quat {
    let mut turned = angle - start;
    if turned > PI {
        turned -= TAU;
    } else if turned < -PI {
        turned += TAU;
    }
    (original.pose.rotation * Quat::from_rotation_z(turned)).normalize()
}

pub fn span(panel: &Panel) -> [f32; 4] {
    let bounds = bounds(panel);
    let metres = panel.metres();
    [
        bounds.min.x / metres.x,
        bounds.min.y / metres.y,
        bounds.max.x / metres.x,
        bounds.max.y / metres.y,
    ]
}

pub fn look(panel: &Panel, chrome: &Chrome, pixels_per_point: f32) -> Look {
    let mine = |window: WindowId| window == panel.window;
    Look {
        metres: metres_of(panel),
        hovered: chrome
            .hovered
            .filter(|(window, _)| mine(*window))
            .map(|(_, part)| part),
        held: chrome
            .gesture
            .filter(|gesture| mine(gesture.window()))
            .map(|gesture| gesture.part()),
        slider: chrome.slider.is_some_and(mine),
        bend: panel.bend / MOST_BEND,
        pixels_per_point,
    }
}

pub fn output(window: WindowId, look: &Look, context: Option<&Context>) -> WindowOutput {
    let layout = layout(look.metres, look.slider);
    let origin = layout.bounds.min;
    let point = |at: Pos2| {
        pos2(
            (at.x - origin.x) * POINTS_PER_METRE,
            (at.y - origin.y) * POINTS_PER_METRE,
        )
    };
    let points = |rect: Rect| Rect::from_min_max(point(rect.min), point(rect.max));
    let size = layout.bounds.size() * POINTS_PER_METRE;
    let mut layers = Vec::new();
    let mut fill = |rect: Rect, radius: f32, color: Color32| {
        layers.push(Layer::Shape(Shape::Rect {
            rect: points(rect),
            corner_radius: Corners::all(radius * POINTS_PER_METRE),
            stroke_width: 0.0,
            color,
            rotation: Rotation::NONE,
            clip: Rect::EVERYTHING,
        }));
    };
    let hot =
        |part: Part| look.held == Some(part) || (look.held.is_none() && look.hovered == Some(part));
    for (corner, arms) in &layout.brackets {
        let color = if hot(Part::Scale(*corner)) {
            ACCENT
        } else {
            HANDLE
        };
        for arm in arms {
            fill(*arm, THICKNESS / 2.0, color);
        }
    }
    fill(layout.pill, BAR / 2.0, PILL);
    let buttons = layout
        .parts
        .iter()
        .filter(|(part, _)| matches!(part, Part::Move | Part::Turn | Part::Roll | Part::Bend));
    let mut icons = Vec::new();
    for (part, rect) in buttons {
        let on = *part == Part::Bend && look.slider;
        if on {
            fill(*rect, BUTTON / 2.0, BUTTON_ON);
        } else if hot(*part) {
            fill(*rect, BUTTON / 2.0, BUTTON_HOT);
        }
        let glyph = match part {
            Part::Move => ICON_MOVE,
            Part::Turn => ICON_ROTATE_3D,
            Part::Roll => ICON_ROTATE_CW_SQUARE,
            _ => ICON_SPLINE,
        };
        icons.push((glyph, rect.center()));
    }
    if look.slider {
        let track = layout.track;
        let groove =
            Rect::from_center_size(track.center(), vec2(track.width() - KNOB, GROOVE_HEIGHT));
        fill(groove, GROOVE_HEIGHT / 2.0, GROOVE);
        let at = knob(track, look.bend);
        let filled = Rect::from_min_max(groove.min, pos2(at.x, groove.max.y));
        fill(filled, GROOVE_HEIGHT / 2.0, ACCENT);
        let color = if hot(Part::Slider) { ACCENT } else { GLYPH };
        fill(
            Rect::from_center_size(at, vec2(KNOB, KNOB)),
            KNOB / 2.0,
            color,
        );
    }
    if let Some(context) = context {
        for (glyph, centre) in icons {
            let galley = context.layout(
                glyph.as_str(),
                FontId::icons(ICON * POINTS_PER_METRE),
                TextLayout::DEFAULT,
            );
            let centre = point(centre);
            layers.push(Layer::Shape(Shape::Text {
                origin: centre - galley.size() / 2.0,
                galley,
                scale: 1.0,
                color: GLYPH,
                rotation: Rotation::NONE,
                clip: Rect::EVERYTHING,
            }));
        }
    }
    let mut state = WindowState::default();
    state.appearance.clear_color = Color32::TRANSPARENT;
    WindowOutput {
        id: window,
        size,
        layers: Rc::new(layers),
        window: state,
        changed: true,
        damage: Region::NOTHING,
        moved: None,
        pixels_per_point: look.pixels_per_point,
    }
}
