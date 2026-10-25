use std::collections::BTreeMap;

use beui_core::color::Color32;
use beui_core::context::WindowOutput;
use beui_core::geometry::Rect;
use beui_core::painting::{
    self, Content, Fade, Frame as Paint, Painting, Pixels, Placement, Primitive, Punches,
    RoundedRect, Triangle, Turn, Vertex,
};
use glam::Vec3;

use crate::chrome::LIFT;
use crate::head::NEAR;
use crate::panel::bent;
use crate::render::{Frame, PanelFrame};
use crate::space::SKY;

const COLUMNS: usize = 24;
const ROWS: usize = 4;
const WHITE: u64 = u64::MAX;

pub fn painting(frame: &Frame, rect: Rect) -> Option<Painting> {
    let scale = frame.pixels_per_point;
    let whole = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    let mut out = Out {
        primitives: vec![Primitive {
            clip: whole,
            fade: Fade::NONE,
            content: Content::RoundedRect(RoundedRect {
                rect: whole,
                corner_radius: 0.0,
                stroke_width: 0.0,
                color: SKY.to_array(),
                turn: Turn::NONE,
            }),
        }],
        textures: BTreeMap::new(),
    };
    let eye = frame.eye;
    let mut panels: Vec<&PanelFrame> = frame.panels.iter().collect();
    let distance = |panel: &PanelFrame| panel.model.w_axis.truncate().distance(eye);
    panels.sort_by(|a, b| distance(b).total_cmp(&distance(a)));
    let find = |outputs: &[WindowOutput], panel: &PanelFrame| {
        outputs
            .iter()
            .find(|output| output.id == panel.window)
            .cloned()
    };
    for panel in panels {
        if let Some(output) = find(&frame.windows, panel) {
            let background = output.window.appearance.clear_color;
            let picture =
                painting::painting(&output, output.size, scale, background, Punches::Shown).ok()?;
            out.place(frame, rect, panel, &picture, [0.0, 0.0, 1.0, 1.0], 0.0, 1.0);
        }
        if let Some(chrome) = panel.chrome
            && let Some(output) = find(&frame.chrome, panel)
        {
            let picture = painting::painting(
                &output,
                output.size,
                scale,
                Color32::TRANSPARENT,
                Punches::Shown,
            )
            .ok()?;
            out.place(
                frame,
                rect,
                panel,
                &picture,
                chrome.span,
                LIFT,
                chrome.opacity,
            );
        }
    }
    Some(Painting {
        frame: Paint {
            size: [
                (rect.width() * scale).round().max(1.0) as u32,
                (rect.height() * scale).round().max(1.0) as u32,
            ],
            pixels_per_point: scale,
            background: SKY.to_array(),
            primitives: out.primitives,
        },
        textures: out.textures,
    })
}

struct Out {
    primitives: Vec<Primitive>,
    textures: BTreeMap<u64, Pixels>,
}

impl Out {
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        frame: &Frame,
        rect: Rect,
        panel: &PanelFrame,
        picture: &Painting,
        span: [f32; 4],
        lift: f32,
        opacity: f32,
    ) {
        let screen = |u: f32, v: f32| -> Option<[f32; 2]> {
            let u = span[0] + (span[2] - span[0]) * u;
            let v = span[1] + (span[3] - span[1]) * v;
            let local = bent(
                panel.metres.x,
                panel.bend,
                (u - 0.5) * panel.metres.x,
                (0.5 - v) * panel.metres.y,
                lift,
            );
            let world: Vec3 = panel.model.transform_point3(local);
            let clip = frame.view_projection * world.extend(1.0);
            if clip.w <= NEAR {
                return None;
            }
            let (x, y) = (clip.x / clip.w, clip.y / clip.w);
            Some([
                rect.min.x + (x + 1.0) / 2.0 * rect.width(),
                rect.min.y + (1.0 - y) / 2.0 * rect.height(),
            ])
        };
        let mut cells = Vec::new();
        for column in 0..COLUMNS {
            for row in 0..ROWS {
                let (left, right) = (
                    column as f32 / COLUMNS as f32,
                    (column + 1) as f32 / COLUMNS as f32,
                );
                let (top, bottom) = (row as f32 / ROWS as f32, (row + 1) as f32 / ROWS as f32);
                let (Some(c0), Some(c1), Some(c2), Some(c3)) = (
                    screen(left, top),
                    screen(right, top),
                    screen(right, bottom),
                    screen(left, bottom),
                ) else {
                    continue;
                };
                let facing = (c1[0] - c0[0]) * (c3[1] - c0[1]) - (c1[1] - c0[1]) * (c3[0] - c0[0]);
                if facing <= 0.0 {
                    continue;
                }
                let xs = [c0[0], c1[0], c2[0], c3[0]];
                let ys = [c0[1], c1[1], c2[1], c3[1]];
                let clip = [
                    xs.into_iter().fold(f32::MAX, f32::min).max(rect.min.x),
                    ys.into_iter().fold(f32::MAX, f32::min).max(rect.min.y),
                    xs.into_iter().fold(f32::MIN, f32::max).min(rect.max.x),
                    ys.into_iter().fold(f32::MIN, f32::max).min(rect.max.y),
                ];
                if clip[2] > clip[0] && clip[3] > clip[1] {
                    cells.push(([left, top, right, bottom], [c0, c1, c2, c3], clip));
                }
            }
        }
        let mut background = picture.frame.background;
        background[3] = (f32::from(background[3]) * opacity).round() as u8;
        if background[3] > 0 {
            self.textures.entry(WHITE).or_insert_with(|| Pixels {
                size: [1, 1],
                rgba: vec![[255; 4]],
            });
            let vertex = |pos: [f32; 2]| Vertex {
                pos,
                uv: [0.0, 0.0],
                color: background,
            };
            let triangles = cells
                .iter()
                .flat_map(|(_, [c0, c1, c2, c3], _)| {
                    [[*c0, *c1, *c2], [*c0, *c2, *c3]].map(|corners| Triangle {
                        texture: WHITE,
                        corners: corners.map(vertex),
                    })
                })
                .collect();
            self.primitives.push(Primitive {
                clip: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
                fade: Fade::NONE,
                content: Content::Mesh(triangles),
            });
        }
        let mut picture = picture.clone();
        picture.frame.background = [0; 4];
        for (source, corners, clip) in cells {
            let placed = picture.placed(Placement {
                source,
                corners,
                clip,
            });
            for mut primitive in placed {
                if !overlaps(&primitive) {
                    continue;
                }
                faded(&mut primitive.content, opacity);
                self.primitives.push(primitive);
            }
        }
        for (key, pixels) in &picture.textures {
            self.textures.entry(*key).or_insert_with(|| pixels.clone());
        }
    }
}

fn overlaps(primitive: &Primitive) -> bool {
    let bounds = match &primitive.content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => {
            let reach = shape.stroke_width / 2.0;
            match shape.turn.turns() {
                true => return true,
                false => [
                    shape.rect[0] - reach,
                    shape.rect[1] - reach,
                    shape.rect[2] + reach,
                    shape.rect[3] + reach,
                ],
            }
        }
        Content::Glyph(glyph) => match glyph.turn.turns() {
            true => return true,
            false => glyph.rect,
        },
        Content::Mesh(triangles) => triangles.iter().flat_map(|triangle| triangle.corners).fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |at, corner| {
                [
                    at[0].min(corner.pos[0]),
                    at[1].min(corner.pos[1]),
                    at[2].max(corner.pos[0]),
                    at[3].max(corner.pos[1]),
                ]
            },
        ),
        Content::Callback(_) => return false,
    };
    let clip = primitive.clip;
    bounds[0] < clip[2] && bounds[2] > clip[0] && bounds[1] < clip[3] && bounds[3] > clip[1]
}

fn faded(content: &mut Content, opacity: f32) {
    if opacity >= 1.0 {
        return;
    }
    let fade = |color: &mut [u8; 4]| color[3] = (f32::from(color[3]) * opacity).round() as u8;
    match content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => fade(&mut shape.color),
        Content::Glyph(glyph) => fade(&mut glyph.color),
        Content::Mesh(triangles) => {
            for triangle in triangles {
                for corner in &mut triangle.corners {
                    fade(&mut corner.color);
                }
            }
        }
        Content::Callback(_) => {}
    }
}
