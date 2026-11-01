use std::collections::BTreeMap;

use beui_core::context::WindowOutput;
use beui_core::painting::{
    self, Content, Fade, Frame as Paint, Painting, Pixels, Primitive, Punches, RoundedRect,
    Triangle, Turn, Vertex,
};
use beui_types::Color32;
use beui_types::Rect;
use glam::Vec3;

use crate::chrome::LIFT;
use crate::floor;
use crate::head::NEAR;
use crate::panel::bent;
use crate::render::{
    BEAM_WIDTH, Frame, OUTLINE, OUTLINE_FILL, OUTLINE_LINE, OUTLINE_WIDTH, Outline, PanelFrame,
    SEGMENTS,
};
use crate::space::SKY;

mod scene;

use scene::Scene;

const WHITE: u64 = u64::MAX;
const BACKDROP: usize = 16;
const GRID_REACH: f32 = 20.0;
const GRID_SEGMENT: usize = 4;
const DOT_SIDES: usize = 16;

pub fn painting(frame: &Frame, rect: Rect) -> Option<Painting> {
    let scale = frame.pixels_per_point;
    let whole = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    let mut out = Out {
        primitives: vec![Primitive {
            clip: whole,
            fade: Fade::NONE,
            rounds: Vec::new(),
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
    out.backdrop(frame, rect);
    out.grid(frame, rect);
    let mut scene = Scene::new(frame, rect);
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
            scene.surface(panel, picture, [0.0, 0.0, 1.0, 1.0], 0.0, 1.0);
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
            scene.surface(panel, picture, chrome.span, LIFT, chrome.opacity);
        }
    }
    scene.paint(&mut out, scale);
    out.beams(frame, rect);
    for outline in &frame.outlines {
        out.outline(frame, rect, outline);
    }
    if let (Some(cursor), Some(output)) = (&frame.cursor, &frame.sprite) {
        let picture = painting::painting(
            output,
            output.size,
            scale,
            Color32::TRANSPARENT,
            Punches::Shown,
        )
        .ok()?;
        let mut over = Scene::new(frame, rect);
        over.surface(cursor, picture, [0.0, 0.0, 1.0, 1.0], 0.0, 1.0);
        over.paint(&mut out, scale);
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
    fn white(&mut self) {
        self.textures.entry(WHITE).or_insert_with(|| Pixels {
            size: [1, 1],
            rgba: vec![[255; 4]],
        });
    }

    fn mesh(&mut self, rect: Rect, triangles: Vec<Triangle>) {
        if triangles.is_empty() {
            return;
        }
        self.white();
        self.primitives.push(Primitive {
            clip: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
            fade: Fade::NONE,
            rounds: Vec::new(),
            content: Content::Mesh(triangles),
        });
    }

    fn backdrop(&mut self, frame: &Frame, rect: Rect) {
        let inverse = frame.view_projection.inverse();
        let vertex = |column: usize, row: usize| {
            let (u, v) = (
                column as f32 / BACKDROP as f32,
                row as f32 / BACKDROP as f32,
            );
            let far = inverse.project_point3(Vec3::new(u * 2.0 - 1.0, 1.0 - v * 2.0, 0.5));
            Vertex {
                pos: [
                    rect.min.x + u * rect.width(),
                    rect.min.y + v * rect.height(),
                ],
                uv: [0.0, 0.0],
                color: floor::background(far - frame.eye).to_array(),
            }
        };
        let mut triangles = Vec::new();
        for column in 0..BACKDROP {
            for row in 0..BACKDROP {
                let corners = [
                    vertex(column, row),
                    vertex(column + 1, row),
                    vertex(column + 1, row + 1),
                    vertex(column, row + 1),
                ];
                triangles.push(Triangle {
                    texture: WHITE,
                    corners: [corners[0], corners[1], corners[2]],
                });
                triangles.push(Triangle {
                    texture: WHITE,
                    corners: [corners[0], corners[2], corners[3]],
                });
            }
        }
        self.mesh(rect, triangles);
    }

    fn grid(&mut self, frame: &Frame, rect: Rect) {
        let eye = frame.eye;
        let half = 0.5 / frame.pixels_per_point;
        let screen = |clip: glam::Vec4| {
            [
                rect.min.x + (clip.x / clip.w + 1.0) / 2.0 * rect.width(),
                rect.min.y + (1.0 - clip.y / clip.w) / 2.0 * rect.height(),
            ]
        };
        let alpha = |at: Vec3| {
            let distance = (at - eye).with_y(0.0).length();
            floor::fade(distance)
                * (1.0 - floor::smoothstep(GRID_REACH * 0.6, GRID_REACH, distance))
        };
        let (centre_x, centre_z) = (eye.x.round() as i32, eye.z.round() as i32);
        let reach = GRID_REACH as i32;
        let mut triangles = Vec::new();
        for line in -reach..=reach {
            for along in (-reach..reach).step_by(GRID_SEGMENT) {
                for crosses_x in [true, false] {
                    let (fixed, from, to) = match crosses_x {
                        true => (
                            centre_x + line,
                            centre_z + along,
                            centre_z + along + GRID_SEGMENT as i32,
                        ),
                        false => (
                            centre_z + line,
                            centre_x + along,
                            centre_x + along + GRID_SEGMENT as i32,
                        ),
                    };
                    let point = |at: i32| match crosses_x {
                        true => Vec3::new(fixed as f32, 0.0, at as f32),
                        false => Vec3::new(at as f32, 0.0, fixed as f32),
                    };
                    let (mut a, mut b) = (point(from), point(to));
                    let (alpha_a, alpha_b) = (alpha(a), alpha(b));
                    if alpha_a.max(alpha_b) <= 0.0 {
                        continue;
                    }
                    let mut clip_a = frame.view_projection * a.extend(1.0);
                    let mut clip_b = frame.view_projection * b.extend(1.0);
                    if clip_a.w <= NEAR && clip_b.w <= NEAR {
                        continue;
                    }
                    if clip_a.w <= NEAR || clip_b.w <= NEAR {
                        let t = (NEAR * 1.01 - clip_a.w) / (clip_b.w - clip_a.w);
                        let cut = a.lerp(b, t);
                        match clip_a.w <= NEAR {
                            true => a = cut,
                            false => b = cut,
                        }
                        clip_a = frame.view_projection * a.extend(1.0);
                        clip_b = frame.view_projection * b.extend(1.0);
                    }
                    let (start, end) = (screen(clip_a), screen(clip_b));
                    let direction = glam::Vec2::from(end) - glam::Vec2::from(start);
                    if direction.length() < 1e-3 {
                        continue;
                    }
                    let side = direction.normalize().perp() * half;
                    let color = match fixed == 0 {
                        true => floor::AXIS,
                        false => floor::GRID,
                    }
                    .to_array();
                    let vertex = |at: [f32; 2], offset: glam::Vec2, alpha: f32| Vertex {
                        pos: [at[0] + offset.x, at[1] + offset.y],
                        uv: [0.0, 0.0],
                        color: [
                            color[0],
                            color[1],
                            color[2],
                            (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
                        ],
                    };
                    let (alpha_a, alpha_b) = (alpha(a), alpha(b));
                    let corners = [
                        vertex(start, side, alpha_a),
                        vertex(end, side, alpha_b),
                        vertex(end, -side, alpha_b),
                        vertex(start, -side, alpha_a),
                    ];
                    triangles.push(Triangle {
                        texture: WHITE,
                        corners: [corners[0], corners[1], corners[2]],
                    });
                    triangles.push(Triangle {
                        texture: WHITE,
                        corners: [corners[0], corners[2], corners[3]],
                    });
                }
            }
        }
        self.mesh(rect, triangles);
    }

    fn outline(&mut self, frame: &Frame, rect: Rect, outline: &Outline) {
        let screen = |world: Vec3| -> Option<glam::Vec2> {
            let clip = frame.view_projection * world.extend(1.0);
            (clip.w > NEAR).then(|| {
                glam::Vec2::new(
                    rect.min.x + (clip.x / clip.w + 1.0) / 2.0 * rect.width(),
                    rect.min.y + (1.0 - clip.y / clip.w) / 2.0 * rect.height(),
                )
            })
        };
        let world = |u: f32, v: f32| {
            let local = bent(
                outline.metres.x,
                outline.bend,
                (u - 0.5) * outline.metres.x,
                (0.5 - v) * outline.metres.y,
                0.0,
            );
            outline.model.transform_point3(local)
        };
        let color = OUTLINE.to_array();
        let vertex = |at: glam::Vec2, alpha: f32| Vertex {
            pos: at.to_array(),
            uv: [0.0, 0.0],
            color: [
                color[0],
                color[1],
                color[2],
                (alpha * outline.strength * 255.0).round().clamp(0.0, 255.0) as u8,
            ],
        };
        let quad = |triangles: &mut Vec<Triangle>, corners: [Vertex; 4]| {
            triangles.push(Triangle {
                texture: WHITE,
                corners: [corners[0], corners[1], corners[2]],
            });
            triangles.push(Triangle {
                texture: WHITE,
                corners: [corners[0], corners[2], corners[3]],
            });
        };
        let along = |index: u32| index as f32 / SEGMENTS as f32;
        let mut triangles = Vec::new();
        for index in 0..SEGMENTS {
            let (u, next) = (along(index), along(index + 1));
            let corners = [
                world(u, 0.0),
                world(next, 0.0),
                world(next, 1.0),
                world(u, 1.0),
            ];
            if let [Some(a), Some(b), Some(c), Some(d)] = corners.map(screen) {
                quad(
                    &mut triangles,
                    [a, b, c, d].map(|at| vertex(at, OUTLINE_FILL)),
                );
            }
        }
        let mut edge = Vec::new();
        for index in 0..=SEGMENTS {
            edge.push(world(along(index), 0.0));
        }
        edge.push(world(1.0, 1.0));
        for index in (0..=SEGMENTS).rev() {
            edge.push(world(along(index), 1.0));
        }
        edge.push(world(0.0, 0.0));
        for pair in edge.windows(2) {
            let (Some(from), Some(to)) = (screen(pair[0]), screen(pair[1])) else {
                continue;
            };
            let Some(side) =
                screen(pair[0] + frame.right * OUTLINE_WIDTH * pair[0].distance(frame.eye))
            else {
                continue;
            };
            let width = from.distance(side).max(0.5);
            let direction = to - from;
            if direction.length() < 1e-3 {
                continue;
            }
            let across = direction.normalize().perp() * width / 2.0;
            quad(
                &mut triangles,
                [from + across, to + across, to - across, from - across]
                    .map(|at| vertex(at, OUTLINE_LINE)),
            );
        }
        self.mesh(rect, triangles);
    }

    fn beams(&mut self, frame: &Frame, rect: Rect) {
        let screen = |world: Vec3| -> Option<(glam::Vec2, f32)> {
            let clip = frame.view_projection * world.extend(1.0);
            (clip.w > NEAR).then(|| {
                (
                    glam::Vec2::new(
                        rect.min.x + (clip.x / clip.w + 1.0) / 2.0 * rect.width(),
                        rect.min.y + (1.0 - clip.y / clip.w) / 2.0 * rect.height(),
                    ),
                    clip.w,
                )
            })
        };
        let across = |world: Vec3, metres: f32| -> Option<f32> {
            let (at, _) = screen(world)?;
            let (side, _) = screen(world + frame.right * metres)?;
            Some(at.distance(side))
        };
        let mut triangles = Vec::new();
        for beam in &frame.beams {
            let color = beam.color().to_array();
            let tinted = |alpha: f32| {
                [
                    color[0],
                    color[1],
                    color[2],
                    (f32::from(color[3]) * alpha).round() as u8,
                ]
            };
            let (mut start, end) = (beam.start, beam.end);
            let (start_w, end_w) = (
                (frame.view_projection * start.extend(1.0)).w,
                (frame.view_projection * end.extend(1.0)).w,
            );
            if end_w <= NEAR && start_w <= NEAR {
                continue;
            }
            let mut faint = 0.15;
            if start_w <= NEAR {
                let t = (NEAR * 1.01 - start_w) / (end_w - start_w);
                start = start.lerp(end, t);
                faint += (1.0 - faint) * t;
            }
            let (Some((from, _)), Some((to, _)), Some(near), Some(far)) = (
                screen(start),
                screen(end),
                across(start, BEAM_WIDTH / 2.0),
                across(end, BEAM_WIDTH / 2.0),
            ) else {
                continue;
            };
            let direction = to - from;
            if direction.length() > 1e-3 {
                let side = direction.normalize().perp();
                let vertex = |at: glam::Vec2, alpha: f32| Vertex {
                    pos: at.to_array(),
                    uv: [0.0, 0.0],
                    color: tinted(alpha),
                };
                let corners = [
                    vertex(from + side * near.max(0.5), faint),
                    vertex(to + side * far.max(0.5), 1.0),
                    vertex(to - side * far.max(0.5), 1.0),
                    vertex(from - side * near.max(0.5), faint),
                ];
                triangles.push(Triangle {
                    texture: WHITE,
                    corners: [corners[0], corners[1], corners[2]],
                });
                triangles.push(Triangle {
                    texture: WHITE,
                    corners: [corners[0], corners[2], corners[3]],
                });
            }
            let radius = beam.dot(frame.eye);
            if radius > 0.0
                && let Some(size) = across(end, radius)
            {
                let vertex = |at: glam::Vec2| Vertex {
                    pos: at.to_array(),
                    uv: [0.0, 0.0],
                    color: tinted(1.0),
                };
                let rim = |index: usize| {
                    let angle = std::f32::consts::TAU * index as f32 / DOT_SIDES as f32;
                    to + glam::Vec2::from_angle(angle) * size
                };
                for index in 0..DOT_SIDES {
                    triangles.push(Triangle {
                        texture: WHITE,
                        corners: [vertex(to), vertex(rim(index)), vertex(rim(index + 1))],
                    });
                }
            }
        }
        self.mesh(rect, triangles);
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
