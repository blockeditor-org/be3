use block_editor_beui::ResizeMode;
use block_editor_beui::be_block::canvas::CanvasPoint;
use block_editor_beui::beui::{
    Color32, FontId, Painter, Pos2, Rect, TextAlign, TextLayout, Vec2, pos2,
};

use crate::geometry::*;

use super::paint::{Camera, Palette, SELECTION, arrowhead, handle, outline, stroke};
use super::state::{Gesture, Presence, Tool};

const HINT: &str = "Scroll or use two fingers to move around  ·  Pick a tool to draw\nDrop or paste images, or use Block to add content";
const HINT_MARGIN: f32 = 16.0;
const SELECT_BOX_FILL: Color32 = Color32::from_rgba_unmultiplied(66, 153, 225, 28);
const BADGE_SIDE: f32 = 22.0;
const OUTSIDE_ARTBOARDS: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 56);
const ARTBOARD_LABEL_SIZE: f32 = 12.0;

#[derive(Clone, PartialEq)]
pub(crate) struct Artboard {
    pub(crate) bounds: WorldRect,
    pub(crate) name: String,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Backdrop {
    pub(crate) camera: Camera,
    pub(crate) stage: Rect,
    pub(crate) background: Color32,
    pub(crate) label: Color32,
    pub(crate) artboards: Vec<Artboard>,
}

impl Backdrop {
    pub(crate) fn draw(&self, painter: &Painter) {
        if self.artboards.is_empty() {
            return;
        }
        painter.rect_filled(self.stage, 0.0, OUTSIDE_ARTBOARDS);
        for artboard in &self.artboards {
            painter.rect_filled(self.camera.rect(artboard.bounds), 0.0, self.background);
        }
        for artboard in &self.artboards {
            let rect = self.camera.rect(artboard.bounds);
            let galley = painter.layout(
                &artboard.name,
                FontId::proportional(ARTBOARD_LABEL_SIZE),
                f32::INFINITY,
            );
            let height = galley.size().y;
            painter.galley(
                pos2(
                    rect.left(),
                    rect.top() - height - (ARTBOARD_LABEL_HEIGHT - height) / 2.0,
                ),
                galley,
                self.label,
            );
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Overlay {
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) stage: Rect,
    pub(crate) empty: bool,
    pub(crate) gesture: Option<Gesture>,
    pub(crate) frame: Option<SelectionFrame>,
    pub(crate) resize: ResizeMode,
    pub(crate) rotatable: bool,
    pub(crate) presence: Presence,
    pub(crate) tool: Tool,
    pub(crate) pointer: Option<CanvasPoint>,
}

impl Overlay {
    pub(crate) fn draw(&self, painter: &Painter) {
        if self.empty && self.gesture.is_none() {
            let galley = painter.layout_text(
                HINT,
                FontId::proportional(16.0),
                TextLayout {
                    align: TextAlign::Center,
                    ..TextLayout::wrapped((self.stage.width() - HINT_MARGIN * 2.0).max(1.0))
                },
            );
            let size = galley.size();
            painter.galley(
                pos2(
                    self.stage.center().x - size.x / 2.0,
                    self.stage.center().y - size.y / 2.0,
                ),
                galley,
                self.palette.muted,
            );
        }
        self.draw_gesture(painter);
        if let Some(frame) = self.frame {
            self.draw_selection(painter, frame);
        }
        self.draw_presence(painter);
        self.draw_badge(painter);
    }

    fn draw_gesture(&self, painter: &Painter) {
        let color = self.palette.auto;
        match &self.gesture {
            Some(Gesture::Create {
                tool,
                start,
                current,
                from_center,
                ..
            }) => match tool {
                Tool::Line => {
                    painter.line(self.camera.at(*start), self.camera.at(*current), 2.0, color)
                }
                Tool::Rectangle | Tool::Text | Tool::Artboard => {
                    let bounds = match tool {
                        Tool::Rectangle | Tool::Artboard => {
                            gesture_rect(*start, *current, *from_center)
                        }
                        _ => WorldRect::from_points(*start, *current),
                    };
                    painter.rect_stroke(self.camera.rect(bounds), 0.0, 2.0, color);
                }
                Tool::Select | Tool::Pen => {}
            },
            Some(Gesture::Pen { points }) => {
                let points: Vec<Pos2> = points.iter().map(|point| self.camera.at(*point)).collect();
                stroke(painter, &points, 2.0, color);
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
            Some(Gesture::Move { .. })
            | Some(Gesture::Resize { .. })
            | Some(Gesture::Rotate { .. })
            | None => {}
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

    fn draw_presence(&self, painter: &Painter) {
        for selection in &self.presence.selections {
            let color = block_editor_beui::presence_color(selection.color);
            let corners = selection
                .frame
                .corners()
                .map(|corner| self.camera.at(corner));
            outline(painter, corners, 1.5, color);
        }
        for cursor in &self.presence.cursors {
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
    }

    fn draw_badge(&self, painter: &Painter) {
        let glyph = match self.tool {
            Tool::Select => return,
            Tool::Line => block_editor_beui::beui::icons::ICON_DIAGONAL_LINE,
            Tool::Rectangle => block_editor_beui::beui::icons::ICON_RECTANGLE,
            Tool::Text => block_editor_beui::beui::icons::ICON_TEXT_FIELDS,
            Tool::Pen => block_editor_beui::beui::icons::ICON_DRAW,
            Tool::Artboard => block_editor_beui::beui::icons::ICON_CROP_FREE,
        };
        let Some(pointer) = self.pointer else {
            return;
        };
        let at = self.camera.at(pointer);
        let center = pos2(at.x, at.y + 20.0);
        if !self.stage.contains(center) {
            return;
        }
        let badge = Rect::from_min_max(
            pos2(center.x - BADGE_SIDE / 2.0, center.y - BADGE_SIDE / 2.0),
            pos2(center.x + BADGE_SIDE / 2.0, center.y + BADGE_SIDE / 2.0),
        );
        painter.rect_filled(badge, 5.0, self.palette.surface);
        painter.rect_stroke(badge, 5.0, 1.0, self.palette.border);
        let icon = painter.layout(glyph, FontId::icons(16.0), f32::INFINITY);
        let size = icon.size();
        painter.galley(
            pos2(center.x - size.x / 2.0, center.y - size.y / 2.0),
            icon,
            self.palette.auto,
        );
    }
}
