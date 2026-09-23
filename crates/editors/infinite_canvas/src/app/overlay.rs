use block_client::blocks::infinite_canvas::{CanvasPoint, CanvasPreviewRegion};
use block_editor_plugin::beui::{
    Color32, FontId, Painter, Rect, TextAlign, TextLayout, Vec2, pos2,
};
use block_editor_plugin::{ResizeMode, block_ui};

use crate::geometry::*;

use super::paint::{Camera, PREVIEW_REGION, Palette, SELECTION, arrowhead, handle, outline};
use super::state::{Gesture, Presence, Tool};

const HINT: &str = "Drag to draw  ·  Space-drag to pan  ·  Scroll to move\nDrop or paste images, or use Block to add content";
const SELECT_BOX_FILL: Color32 = Color32::from_rgba_unmultiplied(66, 153, 225, 28);
const BADGE_SIDE: f32 = 22.0;

#[derive(Clone, PartialEq)]
pub(crate) struct Overlay {
    pub(crate) camera: Camera,
    pub(crate) palette: Palette,
    pub(crate) stage: Rect,
    pub(crate) empty: bool,
    pub(crate) region: Option<CanvasPreviewRegion>,
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
                    ..TextLayout::DEFAULT
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
        if let Some(region) = self.region {
            let rect = self.camera.rect(preview_region_bounds(region));
            painter.rect_stroke(rect, 0.0, 1.5, PREVIEW_REGION);
            let label = painter.layout("Preview", FontId::proportional(12.0), f32::INFINITY);
            painter.galley(
                pos2(rect.left() + 5.0, rect.top() + 4.0),
                label,
                PREVIEW_REGION,
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
                Tool::Rectangle | Tool::Text => {
                    let bounds = match tool {
                        Tool::Rectangle => gesture_rect(*start, *current, *from_center),
                        _ => WorldRect::from_points(*start, *current),
                    };
                    painter.rect_stroke(self.camera.rect(bounds), 0.0, 2.0, color);
                }
                Tool::Select | Tool::Pen => {}
            },
            Some(Gesture::Pen { points }) => {
                for window in points.windows(2) {
                    painter.line(
                        self.camera.at(window[0]),
                        self.camera.at(window[1]),
                        2.0,
                        color,
                    );
                }
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
            let color = block_ui::presence_color(selection.color);
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
            let color = block_ui::presence_color(cursor.color);
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
            Tool::Line => block_editor_plugin::beui::icons::ICON_DIAGONAL_LINE,
            Tool::Rectangle => block_editor_plugin::beui::icons::ICON_RECTANGLE,
            Tool::Text => block_editor_plugin::beui::icons::ICON_TEXT_FIELDS,
            Tool::Pen => block_editor_plugin::beui::icons::ICON_DRAW,
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
