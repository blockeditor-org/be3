use block_editor_beui::ResizeMode;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::{
    Color32, FillRule, FontId, LineCap, LineJoin, Path, Pos2, Rect, Stroke, TextAlign, TextLayout,
    Vec2, pos2, reactive::Painter,
};
use infinite_canvas_types::{CanvasPoint, CanvasStyle};

use crate::freehand;
use crate::geometry::*;

use super::paint::{
    Camera, Palette, SELECTION, arrowhead, handle, outline, resolve_color, with_opacity,
};
use super::state::{Gesture, RemoteCursor, RemoteSelection, Tool};

const HINT: &str = "Scroll or use two fingers to move around  ·  Pick a tool to draw\nDrop or paste images, or use Block to add content";
const HINT_MARGIN: f32 = 16.0;
const SELECT_BOX_FILL: Color32 = Color32::from_rgba_unmultiplied(66, 153, 225, 28);
const BADGE_SIDE: f32 = 22.0;
const BADGE_BELOW: f32 = 20.0;
const FRAME_OUTLINE: f32 = 1.0;
const FRAME_LABEL_SIZE: f32 = 12.0;
const DASHES: [f32; 2] = [4.0, 4.0];
const GROUP_MARGIN: f32 = 6.0;
const OVERLAY_REACH: f32 = 24.0;
const ANCHOR_RADIUS: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PathGrips {
    pub(crate) outline: Vec<CanvasPoint>,
    pub(crate) points: Vec<CanvasPoint>,
    pub(crate) middles: Vec<CanvasPoint>,
}
const ERASER_TRAIL_WIDTH: f32 = 6.0;

#[derive(Clone, PartialEq)]
pub(crate) struct FrameBackdrop {
    pub(crate) name: String,
    pub(crate) fill: Color32,
    pub(crate) label: Color32,
    pub(crate) outline: Color32,
    pub(crate) chrome: bool,
}

impl FrameBackdrop {
    pub(crate) fn draw(&self, painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, self.fill);
        if !self.chrome {
            return;
        }
        painter.rect_stroke(rect, 0.0, FRAME_OUTLINE, self.outline);
        let label = painter.lay_out_text(
            &self.name,
            FontId::proportional(FRAME_LABEL_SIZE),
            TextLayout::DEFAULT,
        );
        let height = label.size().y;
        painter.draw_text(
            pos2(
                rect.left(),
                rect.top() - height - (FRAME_LABEL_HEIGHT - height) / 2.0,
            ),
            &label,
            self.label,
        );
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Hint {
    pub(crate) color: Color32,
}

impl Hint {
    pub(crate) fn draw(&self, painter: &Painter, stage: Rect) {
        let hint = painter.lay_out_text(
            HINT,
            FontId::proportional(16.0),
            TextLayout {
                align: TextAlign::Center,
                ..TextLayout::wrapped((stage.width() - HINT_MARGIN * 2.0).max(1.0))
            },
        );
        let size = hint.size();
        painter.draw_text(
            pos2(
                stage.center().x - size.x / 2.0,
                stage.center().y - size.y / 2.0,
            ),
            &hint,
            self.color,
        );
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Badge {
    pub(crate) glyph: Glyph,
    pub(crate) at: CanvasPoint,
}

impl Badge {
    fn draw(&self, painter: &Painter, at: Pos2, palette: &Palette) {
        let center = pos2(at.x, at.y + BADGE_BELOW);
        let badge = Rect::from_center_size(center, Vec2::splat(BADGE_SIDE));
        painter.rect_filled(badge, 5.0, palette.surface);
        painter.rect_stroke(badge, 5.0, 1.0, palette.border);
        let icon = painter.lay_out_text(
            self.glyph.as_str(),
            FontId::icons(16.0),
            TextLayout::DEFAULT,
        );
        let size = icon.size();
        painter.draw_text(
            pos2(center.x - size.x / 2.0, center.y - size.y / 2.0),
            &icon,
            palette.auto,
        );
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Overlay {
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) outlines: Vec<SelectionFrame>,
    pub(crate) frame: Option<SelectionFrame>,
    pub(crate) path: Option<PathGrips>,
    pub(crate) anchors: Vec<[CanvasPoint; 2]>,
    pub(crate) group: Option<WorldRect>,
    pub(crate) resize: ResizeMode,
    pub(crate) rotatable: bool,
    pub(crate) selections: Vec<RemoteSelection>,
}

impl Overlay {
    pub(crate) fn reach(&self) -> Option<WorldRect> {
        let mut points: Vec<CanvasPoint> = Vec::new();
        for frame in &self.outlines {
            points.extend(frame.corners());
        }
        if let Some(group) = self.group {
            points.extend([group.min, group.max]);
        }
        points.extend(self.anchors.iter().flatten());
        match (&self.path, self.frame) {
            (Some(grips), _) => {
                points.extend(
                    grips
                        .outline
                        .iter()
                        .chain(&grips.points)
                        .chain(&grips.middles),
                );
            }
            (None, Some(frame)) => {
                points.extend(frame.corners());
                if self.rotatable {
                    points.push(frame.rotate_handle());
                }
            }
            (None, None) => {}
        }
        for selection in &self.selections {
            points.extend(selection.frame.corners());
        }
        points
            .into_iter()
            .map(|point| WorldRect::from_points(point, point))
            .reduce(WorldRect::union)
    }

    pub(crate) fn margin(scale: f32) -> f32 {
        OVERLAY_REACH / scale.max(f32::EPSILON)
    }

    pub(crate) fn draw(&self, painter: &Painter) {
        for outline_frame in &self.outlines {
            let corners = outline_frame.corners().map(|corner| self.camera.at(corner));
            outline(painter, corners, 1.0, SELECTION);
        }
        self.draw_group(painter);
        self.draw_anchors(painter);
        match (&self.path, self.frame) {
            (Some(grips), _) => self.draw_path_grips(painter, grips),
            (None, Some(frame)) => self.draw_selection(painter, frame),
            (None, None) => {}
        }
        for selection in &self.selections {
            let color = block_editor_beui::presence_color(selection.color);
            let corners = selection
                .frame
                .corners()
                .map(|corner| self.camera.at(corner));
            outline(painter, corners, 1.5, color);
        }
    }

    fn draw_group(&self, painter: &Painter) {
        let Some(group) = self.group else {
            return;
        };
        let rect = self.camera.rect(group).expand(GROUP_MARGIN);
        let outline = Path::polygon([
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ]);
        painter.stroke_path(&outline, &Stroke::new(1.0).dashed(DASHES, 0.0), SELECTION);
    }

    fn draw_anchors(&self, painter: &Painter) {
        for [end, anchor] in &self.anchors {
            let (end, anchor) = (self.camera.at(*end), self.camera.at(*anchor));
            painter.stroke_path(
                &Path::polyline([end, anchor]),
                &Stroke::new(1.0).dashed(DASHES, 0.0),
                SELECTION,
            );
            let dot = Path::ellipse(anchor, Vec2::splat(ANCHOR_RADIUS));
            painter.fill_path(&dot, FillRule::NonZero, SELECTION);
        }
    }

    fn draw_path_grips(&self, painter: &Painter, grips: &PathGrips) {
        let outline = Path::polyline(grips.outline.iter().map(|at| self.camera.at(*at)));
        painter.stroke_path(&outline, &Stroke::new(1.0).join(LineJoin::Round), SELECTION);
        for middle in &grips.middles {
            let ring = Path::ellipse(self.camera.at(*middle), Vec2::splat(HANDLE_RADIUS - 1.0));
            painter.fill_path(&ring, FillRule::NonZero, self.palette.surface);
            painter.stroke_path(&ring, &Stroke::new(1.5), SELECTION);
        }
        for point in &grips.points {
            handle(painter, self.camera.at(*point), SELECTION);
        }
    }

    fn draw_selection(&self, painter: &Painter, frame: SelectionFrame) {
        let corners = frame.corners().map(|corner| self.camera.at(corner));
        outline(painter, corners, 1.0, SELECTION);
        if self.resize != ResizeMode::None {
            for (spot, at) in resize_handles(frame) {
                if resize_handle_allowed(spot, self.resize) {
                    handle(painter, self.camera.at(at), SELECTION);
                }
            }
        }
        if self.rotatable {
            let top = self.camera.at(frame.point(CanvasPoint::new(0.0, -0.5)));
            let grip = self.camera.at(frame.rotate_handle());
            painter.line(top, grip, 1.0, SELECTION);
            handle(painter, grip, SELECTION);
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Live {
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) gesture: Option<Gesture>,
    pub(crate) cursors: Vec<RemoteCursor>,
    pub(crate) pen: CanvasStyle,
    pub(crate) badge: Option<Badge>,
}

impl Live {
    pub(crate) fn is_empty(&self) -> bool {
        !self.gesture.as_ref().is_some_and(drawn)
            && self.cursors.iter().all(|cursor| cursor.pointer.is_none())
            && self.badge.is_none()
    }

    pub(crate) fn draw(&self, painter: &Painter, stage: Rect) {
        self.draw_gesture(painter);
        for cursor in &self.cursors {
            let Some(pointer) = cursor.pointer else {
                continue;
            };
            let color = block_editor_beui::presence_color(cursor.color);
            let tip = self.camera.at(pointer);
            let inward = Vec2::new(0.4, 1.0);
            let length = inward.length();
            arrowhead(
                painter,
                tip,
                Vec2::new(inward.x / length, inward.y / length),
                16.0,
                2.0,
                color,
            );
        }
        if let Some(badge) = &self.badge {
            let at = self.camera.at(badge.at);
            if stage.contains(pos2(at.x, at.y + BADGE_BELOW)) {
                badge.draw(painter, at, &self.palette);
            }
        }
    }

    fn draw_gesture(&self, painter: &Painter) {
        match &self.gesture {
            Some(Gesture::Create {
                tool: Tool::Text,
                start,
                current,
                ..
            }) => {
                let bounds = WorldRect::from_points(*start, *current);
                painter.rect_stroke(self.camera.rect(bounds), 0.0, 2.0, self.palette.auto);
            }
            Some(Gesture::Erase { trail, .. }) => {
                let points: Vec<Pos2> = trail.iter().map(|at| self.camera.at(*at)).collect();
                if let [only] = points.as_slice() {
                    let dot = Path::ellipse(*only, Vec2::splat(ERASER_TRAIL_WIDTH / 2.0));
                    painter.fill_path(&dot, FillRule::NonZero, self.palette.muted);
                } else if !points.is_empty() {
                    let stroke = Stroke::new(ERASER_TRAIL_WIDTH)
                        .join(LineJoin::Round)
                        .cap(LineCap::Round);
                    painter.stroke_path(&Path::polyline(points), &stroke, self.palette.muted);
                }
            }
            Some(
                gesture @ Gesture::Pen {
                    points,
                    pressures,
                    predicted,
                    ..
                },
            ) if drawn(gesture) => {
                let from = points.len().saturating_sub(2);
                let mut pressures = pressures.get(from..).unwrap_or_default().to_vec();
                let points: Vec<CanvasPoint> =
                    points[from..].iter().chain(predicted).copied().collect();
                if let Some(last) = pressures.last().copied().filter(|_| predicted.is_some()) {
                    pressures.push(last);
                }
                live_stroke(
                    painter,
                    self.camera,
                    &points,
                    &pressures,
                    self.pen.stroke_width,
                    stroke_ink(&self.pen, &self.palette),
                );
            }
            Some(Gesture::SelectBox {
                start,
                current,
                from_center,
                ..
            }) => {
                let rect = self
                    .camera
                    .rect(gesture_rect(*start, *current, *from_center));
                painter.rect_filled(rect, 0.0, SELECT_BOX_FILL);
                painter.rect_stroke(rect, 0.0, 1.0, SELECTION);
            }
            Some(Gesture::Lasso { points, .. }) => {
                let lasso = Path::polygon(points.iter().map(|point| self.camera.at(*point)));
                painter.fill_path(&lasso, FillRule::NonZero, SELECT_BOX_FILL);
                painter.stroke_path(
                    &lasso,
                    &Stroke::new(1.0).join(LineJoin::Round).dashed(DASHES, 0.0),
                    SELECTION,
                );
            }
            Some(Gesture::Create { .. })
            | Some(Gesture::Move { .. })
            | Some(Gesture::Resize { .. })
            | Some(Gesture::Rotate { .. })
            | Some(Gesture::Endpoint { .. })
            | Some(Gesture::Bend { .. })
            | Some(Gesture::Pen { .. })
            | None => {}
        }
    }
}

fn drawn(gesture: &Gesture) -> bool {
    if let Gesture::Pen {
        points, predicted, ..
    } = gesture
    {
        return points.len() < 2 || predicted.is_some();
    }
    matches!(
        gesture,
        Gesture::Create {
            tool: Tool::Text,
            ..
        } | Gesture::Erase { .. }
            | Gesture::SelectBox { .. }
            | Gesture::Lasso { .. }
    )
}

fn stroke_ink(style: &CanvasStyle, palette: &Palette) -> Color32 {
    with_opacity(
        resolve_color(style.stroke_color, palette.auto),
        style.opacity,
    )
}

fn live_stroke(
    painter: &Painter,
    camera: Camera,
    points: &[CanvasPoint],
    pressures: &[f32],
    width: f32,
    color: Color32,
) {
    let points: Vec<Pos2> = points.iter().map(|point| camera.at(*point)).collect();
    let outline = freehand::stroke(&points, pressures, width * camera.scale, false);
    painter.fill_path(&outline, FillRule::NonZero, color);
}
