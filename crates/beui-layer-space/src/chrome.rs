use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::context::{Context, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::font::{FontId, TextLayout};
use beui_core::geometry::{Pos2, Rect, Rotation, Vec2, pos2, vec2};
use beui_core::icons::{ICON_MOVE, ICON_ROTATE_3D, ICON_ROTATE_CW_SQUARE, ICON_SPLINE};
use beui_core::painter::{Corners, Shape};
use beui_core::window::{WindowEdge, WindowId, WindowState};
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
pub const TITLE: f32 = 0.016;
pub const TITLE_PADDING: f32 = 0.012;
pub const REFERENCE: f32 = 0.6;
pub const KEPT: f32 = 1.05;
const NEAREST: f32 = 0.05;

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
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

    pub fn edge(self) -> WindowEdge {
        match self {
            Self::Top => WindowEdge::Top,
            Self::Bottom => WindowEdge::Bottom,
            Self::Left => WindowEdge::Left,
            Self::Right => WindowEdge::Right,
        }
    }

    pub fn of(edge: WindowEdge) -> Option<Self> {
        match edge {
            WindowEdge::Top => Some(Self::Top),
            WindowEdge::Bottom => Some(Self::Bottom),
            WindowEdge::Left => Some(Self::Left),
            WindowEdge::Right => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fit {
    pub scale: f32,
    pub title: f32,
}

impl Default for Fit {
    fn default() -> Self {
        Self {
            scale: 1.0,
            title: 0.0,
        }
    }
}

pub fn fitted(panel: &Panel, eye: Vec3) -> f32 {
    let held = panel.chrome.scale;
    let wanted = (panel.world(pos2(panel.size.x / 2.0, panel.size.y / 2.0)).distance(eye) / REFERENCE).max(NEAREST);
    match (wanted / held).max(held / wanted) > KEPT {
        true => wanted,
        false => held,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Part {
    Scale(Corner),
    Resize(Side),
    Title,
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
        by: Part,
    },
    Resize {
        window: WindowId,
        side: Side,
    },
    Turn {
        window: WindowId,
        rotation: Quat,
        start: Ray,
        sight: Sight,
        aim: Vec3,
        grab: Option<Pos2>,
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
            Self::Move { window, .. }
            | Self::Resize { window, .. }
            | Self::Turn { window, .. }
            | Self::Press { window } => *window,
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
            Self::Move { by, .. } => *by,
            Self::Resize { side, .. } => Part::Resize(*side),
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
    pub slider: Option<WindowId>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Look {
    pub metres: Vec2,
    pub fit: Fit,
    pub title: Option<String>,
    pub hovered: Option<Part>,
    pub held: Option<Part>,
    pub slider: bool,
    pub bend: f32,
    pub pixels_per_point: f32,
}

pub struct Layout {
    pub parts: Vec<(Part, Rect)>,
    pub buttons: Vec<(Part, Rect)>,
    pub brackets: Vec<(Corner, [Rect; 2])>,
    pub sides: Vec<(Side, Rect)>,
    pub pill: Rect,
    pub track: Rect,
    pub title: Option<Rect>,
    pub bounds: Rect,
}

fn between(a: Pos2, b: Pos2) -> Rect {
    Rect::from_min_max(
        pos2(a.x.min(b.x), a.y.min(b.y)),
        pos2(a.x.max(b.x), a.y.max(b.y)),
    )
}

pub fn layout(metres: Vec2, fit: Fit, slider: bool) -> Layout {
    let scale = fit.scale;
    let (thickness, slop) = (THICKNESS * scale, SLOP * scale);
    let (bar, button) = (BAR * scale, BUTTON * scale);
    let reach = (GAP + THICKNESS) * scale;
    let arm = (ARM * scale).min(reach + metres.x.min(metres.y) / 2.0);
    let mut parts = Vec::new();

    let padding = (bar - button) / 2.0;
    let top = metres.y + BAR_GAP * scale;
    let middle = top + bar / 2.0;
    let centre = metres.x / 2.0;
    let kinds = [Part::Move, Part::Turn, Part::Roll, Part::Bend];
    let step = button + padding;
    let first = centre - step * (kinds.len() - 1) as f32 / 2.0;
    let buttons = kinds.map(|part| {
        let at = kinds.iter().position(|each| *each == part).unwrap_or(0);
        let x = first + step * at as f32;
        (part, Rect::from_center_size(pos2(x, middle), vec2(button, button)))
    });
    let last = buttons.len() - 1;
    let closed = Rect::from_min_max(
        pos2(buttons[0].1.min.x - padding, top),
        pos2(buttons[last].1.max.x + padding, top + bar),
    );
    let track = Rect::from_min_max(
        pos2(closed.max.x, top),
        pos2(closed.max.x + TRACK * scale, top + bar),
    );
    let open = Rect::from_min_max(closed.min, pos2(track.max.x + padding, closed.max.y));
    let below = top + bar + slop;
    let split = |rect: Rect| rect.max.x + padding / 2.0;
    for (at, (part, rect)) in buttons.iter().enumerate() {
        let left = match at {
            0 => closed.min.x - slop,
            _ => split(buttons[at - 1].1),
        };
        let right = match (at == last, slider) {
            (true, false) => closed.max.x + slop,
            _ => split(*rect),
        };
        parts.push((*part, Rect::from_min_max(pos2(left, metres.y), pos2(right, below))));
    }
    let sliding = Rect::from_min_max(
        pos2(split(buttons[last].1), metres.y),
        pos2(open.max.x + slop, below),
    );
    if slider {
        parts.push((Part::Slider, sliding));
    }

    let tip = arm - reach;
    let label = Some((fit.title + 2.0 * TITLE_PADDING) * scale)
        .filter(|_| fit.title > 0.0)
        .map(|width| width.min(metres.x - 2.0 * tip))
        .filter(|width| *width >= button)
        .map(|width| {
            Rect::from_min_max(
                pos2(centre - width / 2.0, -(BAR_GAP * scale) - bar),
                pos2(centre + width / 2.0, -(BAR_GAP * scale)),
            )
        });
    if let Some(label) = label {
        parts.push((
            Part::Title,
            Rect::from_min_max(label.min - vec2(slop, slop), pos2(label.max.x + slop, 0.0)),
        ));
    }

    let mut brackets = Vec::new();
    for corner in Corner::ALL {
        let (x, y) = corner.signs();
        let at = corner.of(metres);
        let outer = pos2(at.x + x * reach, at.y + y * reach);
        let across = between(outer, pos2(outer.x - x * arm, outer.y - y * thickness));
        let down = between(outer, pos2(outer.x - x * thickness, outer.y - y * arm));
        let beyond = pos2(outer.x + x * slop, outer.y + y * slop);
        parts.push((
            Part::Scale(corner),
            between(beyond, pos2(outer.x - x * (arm + slop), at.y)),
        ));
        parts.push((
            Part::Scale(corner),
            between(beyond, pos2(at.x, outer.y - y * (arm + slop))),
        ));
        brackets.push((corner, [across, down]));
    }

    let (width, height) = (metres.x, metres.y);
    let mut sides = Vec::new();
    for side in Side::ALL {
        let (drawn, hit) = match side {
            Side::Top => (
                between(pos2(tip, -reach), pos2(width - tip, thickness - reach)),
                between(pos2(0.0, -reach - slop), pos2(width, 0.0)),
            ),
            Side::Bottom => (
                between(
                    pos2(tip, height + reach - thickness),
                    pos2(width - tip, height + reach),
                ),
                between(pos2(0.0, height), pos2(width, height + reach + slop)),
            ),
            Side::Left => (
                between(pos2(-reach, tip), pos2(thickness - reach, height - tip)),
                between(pos2(-reach - slop, 0.0), pos2(0.0, height)),
            ),
            Side::Right => (
                between(
                    pos2(width + reach - thickness, tip),
                    pos2(width + reach, height - tip),
                ),
                between(pos2(width, 0.0), pos2(width + reach + slop, height)),
            ),
        };
        parts.push((Part::Resize(side), hit));
        if (width - 2.0 * tip).min(height - 2.0 * tip) > 0.0 {
            sides.push((side, drawn));
        }
    }

    let bounds = parts
        .iter()
        .fold(sliding.union(open.expand(slop)), |bounds, (_, rect)| {
            bounds.union(*rect)
        });
    Layout {
        parts,
        buttons: buttons.to_vec(),
        brackets,
        sides,
        pill: if slider { open } else { closed },
        track,
        title: label,
        bounds,
    }
}

fn elided(context: &Context, title: &str, font: FontId, room: f32) -> String {
    let width = |text: &str| {
        context
            .layout(text, font, TextLayout::DEFAULT)
            .size()
            .x
    };
    if width(title) <= room {
        return title.to_owned();
    }
    let chars: Vec<char> = title.chars().collect();
    let cut = |kept: usize| {
        let mut text: String = chars[..kept].iter().collect();
        text.truncate(text.trim_end().len());
        text.push('…');
        text
    };
    let (mut fits, mut over) = (0, chars.len());
    while over - fits > 1 {
        let middle = (fits + over) / 2;
        match width(&cut(middle)) <= room {
            true => fits = middle,
            false => over = middle,
        }
    }
    cut(fits)
}

pub fn title_width(context: &Context, title: &str) -> f32 {
    let galley = context.layout(
        title,
        FontId::proportional(TITLE * POINTS_PER_METRE),
        TextLayout::DEFAULT,
    );
    galley.size().x / POINTS_PER_METRE
}

fn inset(track: Rect) -> f32 {
    (KNOB / 2.0 + 0.004) * track.height() / BAR
}

pub fn fraction(track: Rect, x: f32) -> f32 {
    let inset = inset(track);
    ((x - track.min.x - inset) / (track.width() - 2.0 * inset)).clamp(0.0, 1.0)
}

fn knob(track: Rect, fraction: f32) -> Pos2 {
    let inset = inset(track);
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
    layout(metres_of(panel), panel.chrome, true).bounds
}

pub fn pixels_per_point(panel: &Panel, window: f32) -> f32 {
    let size = bounds(panel).size() * POINTS_PER_METRE;
    let largest = crate::scale::LARGEST_TEXTURE / size.x.max(size.y).max(1.0);
    (window / (panel.metres_per_point * POINTS_PER_METRE)).min(largest)
}

pub fn part_at(panel: &Panel, pos: Pos2, slider: bool) -> Option<Part> {
    if panel.contains(pos) {
        return None;
    }
    let at = pos2(
        pos.x * panel.metres_per_point,
        pos.y * panel.metres_per_point,
    );
    layout(metres_of(panel), panel.chrome, slider)
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
    let track = layout(metres_of(original), original.chrome, true).track;
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sight {
    pub view: Quat,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
}

impl Sight {
    pub fn new(eye: Vec3, centre: Vec3, view: Quat) -> Self {
        let forward = (centre - eye).normalize_or(view * Vec3::NEG_Z);
        let up = view * Vec3::Y;
        let up = (up - forward * up.dot(forward)).normalize_or(forward.any_orthonormal_vector());
        Self {
            view,
            forward,
            up,
            right: forward.cross(up),
        }
    }

    fn on_screen(&self, direction: Vec3) -> glam::Vec2 {
        let local = self.view.inverse() * direction;
        glam::Vec2::new(local.x, local.y) / (-local.z).max(1e-4)
    }

    pub fn turn(&self, from: Vec3, to: Vec3) -> Quat {
        let along = self.on_screen(to) - self.on_screen(from);
        let angle = from.angle_between(to) * TURN_GAIN;
        let along = along.normalize_or_zero() * angle;
        Quat::from_axis_angle(self.up, along.x) * Quat::from_axis_angle(self.right, -along.y)
    }
}

pub fn turned(rotation: Quat, from: Vec3, to: Vec3, sight: Sight) -> Quat {
    (sight.turn(from, to) * rotation).normalize()
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

pub fn look(
    panel: &Panel,
    chrome: &Chrome,
    held: Option<Part>,
    pixels_per_point: f32,
    title: Option<&str>,
) -> Look {
    let mine = |window: WindowId| window == panel.window;
    Look {
        metres: metres_of(panel),
        fit: panel.chrome,
        title: title.map(str::to_owned),
        hovered: chrome
            .hovered
            .filter(|(window, _)| mine(*window))
            .map(|(_, part)| part),
        held,
        slider: chrome.slider.is_some_and(mine),
        bend: panel.bend / MOST_BEND,
        pixels_per_point,
    }
}

pub fn output(window: WindowId, look: &Look, context: Option<&Context>) -> WindowOutput {
    let layout = layout(look.metres, look.fit, look.slider);
    let scale = look.fit.scale;
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
            corner_radius: Corners::all(radius * scale * POINTS_PER_METRE),
            stroke_width: 0.0,
            color,
            rotation: Rotation::NONE,
            clip: Rect::EVERYTHING,
        }));
    };
    let hot =
        |part: Part| look.held == Some(part) || (look.held.is_none() && look.hovered == Some(part));
    for (side, drawn) in &layout.sides {
        if hot(Part::Resize(*side)) {
            fill(*drawn, THICKNESS / 2.0, ACCENT);
        }
    }
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
    let title = layout.title.zip(look.title.as_deref());
    if let Some((label, _)) = title {
        let color = if hot(Part::Title) { BUTTON_HOT } else { PILL };
        fill(label, BAR / 2.0, color);
    }
    let mut icons = Vec::new();
    for (part, rect) in &layout.buttons {
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
        let (knob_size, groove_height) = (KNOB * scale, GROOVE_HEIGHT * scale);
        let groove = Rect::from_center_size(
            track.center(),
            vec2(track.width() - knob_size, groove_height),
        );
        fill(groove, GROOVE_HEIGHT / 2.0, GROOVE);
        let at = knob(track, look.bend);
        let filled = Rect::from_min_max(groove.min, pos2(at.x, groove.max.y));
        fill(filled, GROOVE_HEIGHT / 2.0, ACCENT);
        let color = if hot(Part::Slider) { ACCENT } else { GLYPH };
        fill(
            Rect::from_center_size(at, vec2(knob_size, knob_size)),
            KNOB / 2.0,
            color,
        );
    }
    if let Some(context) = context {
        let raster = look.pixels_per_point / context.pixels_per_point();
        let mut text = |text: &str, font: FontId, centre: Pos2, clip: Rect| {
            let galley = context.layout(text, font, TextLayout::DEFAULT);
            let galley = context.rastered(&galley, raster);
            let centre = point(centre);
            layers.push(Layer::Shape(Shape::Text {
                origin: centre - galley.size() / 2.0,
                galley,
                scale: 1.0,
                color: GLYPH,
                rotation: Rotation::NONE,
                clip,
            }));
        };
        for (glyph, centre) in icons {
            let font = FontId::icons(ICON * scale * POINTS_PER_METRE);
            text(glyph.as_str(), font, centre, Rect::EVERYTHING);
        }
        if let Some((label, title)) = title {
            let font = FontId::proportional(TITLE * scale * POINTS_PER_METRE);
            let clip = points(label);
            let room = clip.width() - TITLE_PADDING * scale * POINTS_PER_METRE;
            let title = elided(context, title, font, room);
            text(&title, font, label.center(), clip);
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
