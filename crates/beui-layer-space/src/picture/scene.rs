use beui_core::geometry::Rect;
use beui_core::painting::{Content, Fade, Painting, Placement, Primitive, Triangle, Vertex};
use beui_core::window::WindowId;
use glam::{Mat4, Vec3};

use super::{Out, WHITE, faded, overlaps};
use crate::head::NEAR;
use crate::panel::bent;
use crate::render::{Frame, PanelFrame};

const COLUMNS: usize = 24;
const ROWS: usize = 4;
const TIE: f32 = 1e-5;
const LEAST_OVERLAP: f32 = 1e-4;

struct Surface {
    picture: Painting,
    background: [u8; 4],
    opacity: f32,
}

struct Piece {
    surface: usize,
    window: WindowId,
    source: [f32; 4],
    corners: [[f32; 2]; 4],
    bounds: [f32; 4],
    origin: Vec3,
    normal: Vec3,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Order {
    Free,
    First,
    Second,
    Unknown,
}

pub struct Scene {
    rect: Rect,
    eye: Vec3,
    view_projection: Mat4,
    inverse: Mat4,
    surfaces: Vec<Surface>,
    pieces: Vec<Piece>,
}

impl Scene {
    pub fn new(frame: &Frame, rect: Rect) -> Self {
        Self {
            rect,
            eye: frame.eye,
            view_projection: frame.view_projection,
            inverse: frame.view_projection.inverse(),
            surfaces: Vec::new(),
            pieces: Vec::new(),
        }
    }

    pub fn surface(
        &mut self,
        panel: &PanelFrame,
        mut picture: Painting,
        span: [f32; 4],
        lift: f32,
        opacity: f32,
    ) {
        let rect = self.rect;
        let world = |u: f32, v: f32| -> Vec3 {
            let u = span[0] + (span[2] - span[0]) * u;
            let v = span[1] + (span[3] - span[1]) * v;
            let local = bent(
                panel.metres.x,
                panel.bend,
                (u - 0.5) * panel.metres.x,
                (0.5 - v) * panel.metres.y,
                lift,
            );
            panel.model.transform_point3(local)
        };
        let screen = |world: Vec3| -> Option<[f32; 2]> {
            let clip = self.view_projection * world.extend(1.0);
            if clip.w <= NEAR {
                return None;
            }
            let (x, y) = (clip.x / clip.w, clip.y / clip.w);
            Some([
                rect.min.x + (x + 1.0) / 2.0 * rect.width(),
                rect.min.y + (1.0 - y) / 2.0 * rect.height(),
            ])
        };
        let index = self.surfaces.len();
        let mut background = picture.frame.background;
        background[3] = (f32::from(background[3]) * opacity).round() as u8;
        picture.frame.background = [0; 4];
        for column in 0..COLUMNS {
            for row in 0..ROWS {
                let (left, right) = (
                    column as f32 / COLUMNS as f32,
                    (column + 1) as f32 / COLUMNS as f32,
                );
                let (top, bottom) = (row as f32 / ROWS as f32, (row + 1) as f32 / ROWS as f32);
                let points = [
                    world(left, top),
                    world(right, top),
                    world(right, bottom),
                    world(left, bottom),
                ];
                let (Some(c0), Some(c1), Some(c2), Some(c3)) = (
                    screen(points[0]),
                    screen(points[1]),
                    screen(points[2]),
                    screen(points[3]),
                ) else {
                    continue;
                };
                let facing = (c1[0] - c0[0]) * (c3[1] - c0[1]) - (c1[1] - c0[1]) * (c3[0] - c0[0]);
                if facing <= 0.0 {
                    continue;
                }
                let xs = [c0[0], c1[0], c2[0], c3[0]];
                let ys = [c0[1], c1[1], c2[1], c3[1]];
                let bounds = [
                    xs.into_iter().fold(f32::MAX, f32::min).max(rect.min.x),
                    ys.into_iter().fold(f32::MAX, f32::min).max(rect.min.y),
                    xs.into_iter().fold(f32::MIN, f32::max).min(rect.max.x),
                    ys.into_iter().fold(f32::MIN, f32::max).min(rect.max.y),
                ];
                let source = [left, top, right, bottom];
                let corners = [c0, c1, c2, c3];
                let shows = || {
                    background[3] > 0
                        || picture
                            .placed(Placement {
                                source,
                                corners,
                                clip: bounds,
                            })
                            .iter()
                            .any(overlaps)
                };
                if !empty(bounds) && shows() {
                    self.pieces.push(Piece {
                        surface: index,
                        window: panel.window,
                        source,
                        corners,
                        bounds,
                        origin: points[0],
                        normal: (points[1] - points[0]).cross(points[3] - points[0]),
                    });
                }
            }
        }
        self.surfaces.push(Surface {
            picture,
            background,
            opacity,
        });
    }

    pub fn paint(&self, out: &mut Out, pixels_per_point: f32) {
        let rect = self.rect;
        let pixels = [
            (rect.min.x * pixels_per_point).floor() as i64,
            (rect.min.y * pixels_per_point).floor() as i64,
            (rect.max.x * pixels_per_point).ceil() as i64,
            (rect.max.y * pixels_per_point).ceil() as i64,
        ];
        let all: Vec<usize> = (0..self.pieces.len()).collect();
        self.resolve(out, pixels, pixels_per_point, &all);
        for surface in &self.surfaces {
            for (key, texture) in &surface.picture.textures {
                out.textures.entry(*key).or_insert_with(|| texture.clone());
            }
        }
    }

    fn resolve(&self, out: &mut Out, pixels: [i64; 4], scale: f32, candidates: &[usize]) {
        let tile = pixels.map(|value| value as f32 / scale);
        let here: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|index| !empty(intersect(self.pieces[*index].bounds, tile)))
            .collect();
        if here.is_empty() {
            return;
        }
        let order = self.sorted(&here, tile);
        let (width, height) = (pixels[2] - pixels[0], pixels[3] - pixels[1]);
        match order {
            Some(order) => self.emit(out, tile, &order),
            None if width <= 1 && height <= 1 => {
                let order = self.nearest_last(&here, tile);
                self.emit(out, tile, &order);
            }
            None => {
                let middle_x = pixels[0] + width / 2;
                let middle_y = pixels[1] + height / 2;
                let xs: &[[i64; 2]] = match width > 1 {
                    true => &[[pixels[0], middle_x], [middle_x, pixels[2]]],
                    false => &[[pixels[0], pixels[2]]],
                };
                let ys: &[[i64; 2]] = match height > 1 {
                    true => &[[pixels[1], middle_y], [middle_y, pixels[3]]],
                    false => &[[pixels[1], pixels[3]]],
                };
                for [top, bottom] in ys {
                    for [left, right] in xs {
                        self.resolve(out, [*left, *top, *right, *bottom], scale, &here);
                    }
                }
            }
        }
    }

    fn sorted(&self, here: &[usize], tile: [f32; 4]) -> Option<Vec<usize>> {
        let count = here.len();
        let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut waiting = vec![0usize; count];
        for a in 0..count {
            for b in a + 1..count {
                let (first, second) = match self.order(here[a], here[b], tile) {
                    Order::Free => continue,
                    Order::Unknown => return None,
                    Order::First => (a, b),
                    Order::Second => (b, a),
                };
                after[first].push(second);
                waiting[second] += 1;
            }
        }
        let mut done = vec![false; count];
        let mut order = Vec::with_capacity(count);
        for _ in 0..count {
            let next = (0..count).find(|at| !done[*at] && waiting[*at] == 0)?;
            done[next] = true;
            order.push(here[next]);
            for later in &after[next] {
                waiting[*later] -= 1;
            }
        }
        Some(order)
    }

    fn order(&self, a: usize, b: usize, tile: [f32; 4]) -> Order {
        let (first, second) = (&self.pieces[a], &self.pieces[b]);
        if first.surface == second.surface
            || empty(intersect(intersect(first.bounds, second.bounds), tile))
        {
            return Order::Free;
        }
        let polygon = overlap(&first.corners, &second.corners, tile);
        if area(&polygon) < LEAST_OVERLAP {
            return Order::Free;
        }
        let in_turn = match first.surface < second.surface {
            true => Order::First,
            false => Order::Second,
        };
        if first.window == second.window {
            return in_turn;
        }
        let mut farther = 0;
        for at in polygon {
            let (near, far) = (self.depth(first, at), self.depth(second, at));
            if !(near.is_finite() && far.is_finite() && near > 0.0 && far > 0.0) {
                return Order::Unknown;
            }
            let difference = near - far;
            let tie = TIE * (near + far);
            let side = match difference {
                _ if difference > tie => 1,
                _ if difference < -tie => -1,
                _ => 0,
            };
            if side != 0 && farther != 0 && side != farther {
                return Order::Unknown;
            }
            if side != 0 {
                farther = side;
            }
        }
        match farther {
            1 => Order::First,
            -1 => Order::Second,
            _ => in_turn,
        }
    }

    fn nearest_last(&self, here: &[usize], tile: [f32; 4]) -> Vec<usize> {
        let centre = [(tile[0] + tile[2]) / 2.0, (tile[1] + tile[3]) / 2.0];
        let depth = |index: usize| {
            let depth = self.depth(&self.pieces[index], centre);
            match depth.is_finite() && depth > 0.0 {
                true => depth,
                false => f32::INFINITY,
            }
        };
        let mut order = here.to_vec();
        order.sort_by(|a, b| {
            let (first, second) = (&self.pieces[*a], &self.pieces[*b]);
            match first.window == second.window {
                true => first.surface.cmp(&second.surface),
                false => depth(*b).total_cmp(&depth(*a)),
            }
            .then(a.cmp(b))
        });
        order
    }

    fn depth(&self, piece: &Piece, at: [f32; 2]) -> f32 {
        let rect = self.rect;
        let u = (at[0] - rect.min.x) / rect.width();
        let v = (at[1] - rect.min.y) / rect.height();
        let direction = self
            .inverse
            .project_point3(Vec3::new(u * 2.0 - 1.0, 1.0 - v * 2.0, 0.5))
            - self.eye;
        piece.normal.dot(piece.origin - self.eye) / piece.normal.dot(direction)
    }

    fn covers(&self, index: usize, tile: [f32; 4]) -> bool {
        let piece = &self.pieces[index];
        if self.surfaces[piece.surface].background[3] < u8::MAX {
            return false;
        }
        let corners = piece.corners;
        let turn = cross(corners[0], corners[1], corners[2]).signum();
        [
            [tile[0], tile[1]],
            [tile[2], tile[1]],
            [tile[2], tile[3]],
            [tile[0], tile[3]],
        ]
        .into_iter()
        .all(|at| {
            (0..4).all(|side| turn * cross(corners[side], corners[(side + 1) % 4], at) >= 0.0)
        })
    }

    fn emit(&self, out: &mut Out, tile: [f32; 4], order: &[usize]) {
        let hidden = order
            .iter()
            .rposition(|index| self.covers(*index, tile))
            .unwrap_or(0);
        let order = &order[hidden..];
        let rect = self.rect;
        let whole = intersect(tile, [rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
        for run in order.chunk_by(|a, b| self.pieces[*a].surface == self.pieces[*b].surface) {
            let surface = &self.surfaces[self.pieces[run[0]].surface];
            if surface.background[3] > 0 {
                out.white();
                let vertex = |pos: [f32; 2]| Vertex {
                    pos,
                    uv: [0.0, 0.0],
                    color: surface.background,
                };
                let triangles = run
                    .iter()
                    .flat_map(|index| {
                        let [c0, c1, c2, c3] = self.pieces[*index].corners;
                        [[c0, c1, c2], [c0, c2, c3]].map(|corners| Triangle {
                            texture: WHITE,
                            corners: corners.map(vertex),
                        })
                    })
                    .collect();
                out.primitives.push(Primitive {
                    clip: whole,
                    fade: Fade::NONE,
                    content: Content::Mesh(triangles),
                });
            }
            for index in run {
                let piece = &self.pieces[*index];
                let clip = intersect(piece.bounds, tile);
                if empty(clip) {
                    continue;
                }
                let placed = surface.picture.placed(Placement {
                    source: piece.source,
                    corners: piece.corners,
                    clip,
                });
                for mut primitive in placed {
                    if !overlaps(&primitive) {
                        continue;
                    }
                    faded(&mut primitive.content, surface.opacity);
                    out.primitives.push(primitive);
                }
            }
        }
    }
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

fn empty(rect: [f32; 4]) -> bool {
    rect[2] <= rect[0] || rect[3] <= rect[1]
}

fn cross(origin: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - origin[0]) * (b[1] - origin[1]) - (a[1] - origin[1]) * (b[0] - origin[0])
}

fn area(polygon: &[[f32; 2]]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    (1..polygon.len() - 1)
        .map(|at| cross(polygon[0], polygon[at], polygon[at + 1]))
        .sum::<f32>()
        .abs()
        / 2.0
}

fn clipped(polygon: Vec<[f32; 2]>, side: impl Fn([f32; 2]) -> f32) -> Vec<[f32; 2]> {
    let mut kept = Vec::with_capacity(polygon.len() + 1);
    for (index, &at) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        let (here, there) = (side(at), side(next));
        if here >= 0.0 {
            kept.push(at);
        }
        if (here >= 0.0) != (there >= 0.0) {
            let t = here / (here - there);
            kept.push([at[0] + (next[0] - at[0]) * t, at[1] + (next[1] - at[1]) * t]);
        }
    }
    kept
}

fn overlap(a: &[[f32; 2]; 4], b: &[[f32; 2]; 4], tile: [f32; 4]) -> Vec<[f32; 2]> {
    let mut polygon = a.to_vec();
    let turn = cross(b[0], b[1], b[2]).signum();
    for index in 0..4 {
        let (from, to) = (b[index], b[(index + 1) % 4]);
        polygon = clipped(polygon, |at| turn * cross(from, to, at));
        if polygon.is_empty() {
            return polygon;
        }
    }
    let sides: [&dyn Fn([f32; 2]) -> f32; 4] = [
        &|at| at[0] - tile[0],
        &|at| tile[2] - at[0],
        &|at| at[1] - tile[1],
        &|at| tile[3] - at[1],
    ];
    for side in sides {
        polygon = clipped(polygon, side);
        if polygon.is_empty() {
            break;
        }
    }
    polygon
}
