//! A mesh as a GPU draws triangles: each pixel whose centre a triangle holds (by the top-left
//! rule, so triangles that share an edge never both cover a pixel on it nor leave it uncovered)
//! takes the colour and texture coordinates interpolated across the triangle, with no
//! antialiasing. The pieces a painting draws as meshes (a 3D scene's panels and backdrop from
//! beui-layer-space, colour glyphs, images and lines until paintings hold paths) are drawn
//! that way on the GPU, and vello's shapes cannot interpolate a colour across three corners.

use std::collections::BTreeMap;

use beui_paint_format::{Pixels, TextureKey, Triangle, Vertex};
use beui_vello::vello_common::peniko::ImageAlphaType;
use beui_vello::vello_common::pixmap::{PixelMetadata, Pixmap};

/// The triangles painted onto a transparent pixmap of the pixels they cover within `clip`
/// (whole pixels of the frame), with where its top left corner lies, or none when they cover
/// nothing there.
pub(crate) fn mesh<'a>(
    triangles: &[Triangle],
    clip: [f32; 4],
    scale: f32,
    textures: &dyn Fn(TextureKey) -> Option<&'a Pixels>,
    chains: &mut BTreeMap<TextureKey, Vec<Pixels>>,
) -> Result<Option<(Pixmap, [f32; 2])>, String> {
    let reach = triangles
        .iter()
        .flat_map(|triangle| triangle.corners)
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |reach, corner| {
            let [x, y] = corner.pos.map(|value| value * scale);
            [
                reach[0].min(x),
                reach[1].min(y),
                reach[2].max(x),
                reach[3].max(y),
            ]
        });
    let left = reach[0].floor().max(clip[0]).max(0.0);
    let top = reach[1].floor().max(clip[1]).max(0.0);
    let right = reach[2].ceil().min(clip[2]).min(f32::from(u16::MAX));
    let bottom = reach[3].ceil().min(clip[3]).min(f32::from(u16::MAX));
    if left >= right || top >= bottom {
        return Ok(None);
    }
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let mut canvas = Canvas {
        width,
        height,
        pixels: vec![[0; 4]; (width * height) as usize],
    };
    for triangle in triangles {
        let base = textures(triangle.texture)
            .ok_or_else(|| "the painting is missing a texture it uses".to_owned())?;
        let level = minification(&triangle.corners, scale, base.size);
        let smaller = match level > 0.0 {
            true => chains
                .entry(triangle.texture)
                .or_insert_with(|| chain(base))
                .as_slice(),
            false => &[],
        };
        let sampler = Sampler {
            base,
            smaller,
            level,
        };
        canvas.fill([left, top], triangle.corners, scale, &sampler);
    }
    let data = canvas.pixels.into_iter().flatten().collect();
    Ok(Some((
        Pixmap::from_parts(
            data,
            width as u16,
            height as u16,
            PixelMetadata {
                may_have_transparency: true,
                alpha_type: ImageAlphaType::AlphaPremultiplied,
            },
        ),
        [left, top],
    )))
}

/// Premultiplied pixels.
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 4]>,
}

impl Canvas {
    fn blend(&mut self, x: u32, y: u32, color: [f32; 3], alpha: f32) {
        let target = &mut self.pixels[(y * self.width + x) as usize];
        for (target, source) in target.iter_mut().zip(color) {
            let blended = source * alpha + *target as f32 * (1.0 - alpha);
            *target = blended.round().clamp(0.0, 255.0) as u8;
        }
        let covered = 255.0 * alpha + target[3] as f32 * (1.0 - alpha);
        target[3] = covered.round().clamp(0.0, 255.0) as u8;
    }

    /// Fills a triangle whose corners are in points, this canvas's top left corner lying at
    /// `origin` in the frame's pixels.
    fn fill(&mut self, origin: [f32; 2], corners: [Vertex; 3], scale: f32, sampler: &Sampler<'_>) {
        let mut points = corners.map(|vertex| {
            [
                vertex.pos[0] * scale - origin[0],
                vertex.pos[1] * scale - origin[1],
            ]
        });
        let mut corners = corners;
        if edge(points[0], points[1], points[2]) < 0.0 {
            points.swap(1, 2);
            corners.swap(1, 2);
        }
        let area = edge(points[0], points[1], points[2]);
        if area <= 0.0 {
            return;
        }
        let min_x = points.iter().map(|point| point[0]).fold(f32::MAX, f32::min);
        let max_x = points.iter().map(|point| point[0]).fold(f32::MIN, f32::max);
        let min_y = points.iter().map(|point| point[1]).fold(f32::MAX, f32::min);
        let max_y = points.iter().map(|point| point[1]).fold(f32::MIN, f32::max);
        let left = (min_x.floor() as i64).max(0);
        let right = (max_x.ceil() as i64).min(self.width as i64);
        let top = (min_y.floor() as i64).max(0);
        let bottom = (max_y.ceil() as i64).min(self.height as i64);
        let edges = [
            [points[1], points[2]],
            [points[2], points[0]],
            [points[0], points[1]],
        ];
        for y in top..bottom {
            for x in left..right {
                let point = [x as f32 + 0.5, y as f32 + 0.5];
                let weights = [
                    edge(points[1], points[2], point),
                    edge(points[2], points[0], point),
                    edge(points[0], points[1], point),
                ];
                if weights
                    .iter()
                    .zip(edges)
                    .any(|(weight, edge)| *weight < 0.0 || (*weight == 0.0 && !is_top_left(edge)))
                {
                    continue;
                }
                let weights = weights.map(|weight| weight / area);
                let mut uv = [0.0; 2];
                let mut color = [0.0; 4];
                for (weight, corner) in weights.iter().zip(corners) {
                    uv[0] += corner.uv[0] * weight;
                    uv[1] += corner.uv[1] * weight;
                    for (color, corner) in color.iter_mut().zip(corner.color) {
                        *color += corner as f32 * weight;
                    }
                }
                let texel = sampler.sample(uv);
                let source = [
                    texel[0] * color[0] / 255.0,
                    texel[1] * color[1] / 255.0,
                    texel[2] * color[2] / 255.0,
                ];
                let alpha = texel[3] * color[3] / 255.0 / 255.0;
                self.blend(x as u32, y as u32, source, alpha);
            }
        }
    }
}

struct Sampler<'a> {
    base: &'a Pixels,
    smaller: &'a [Pixels],
    level: f32,
}

impl Sampler<'_> {
    fn level(&self, at: usize) -> &Pixels {
        match at {
            0 => self.base,
            at => &self.smaller[(at - 1).min(self.smaller.len() - 1)],
        }
    }

    fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        if self.level <= 0.0 || self.smaller.is_empty() {
            return bilinear(self.base, uv);
        }
        let low = self.level.floor() as usize;
        let fraction = self.level - low as f32;
        let near = bilinear(self.level(low), uv);
        if fraction <= 0.0 || low >= self.smaller.len() {
            return near;
        }
        let far = bilinear(self.level(low + 1), uv);
        std::array::from_fn(|channel| near[channel] + (far[channel] - near[channel]) * fraction)
    }
}

fn minification(corners: &[Vertex; 3], scale: f32, size: [u32; 2]) -> f32 {
    let range = |values: [f32; 3]| {
        values.iter().copied().fold(f32::MIN, f32::max)
            - values.iter().copied().fold(f32::MAX, f32::min)
    };
    let spread = |axis: usize| {
        let texels = range(corners.map(|corner| corner.uv[axis])) * size[axis] as f32;
        texels / (range(corners.map(|corner| corner.pos[axis])) * scale).max(0.001)
    };
    spread(0).max(spread(1)).log2().max(0.0)
}

fn chain(base: &Pixels) -> Vec<Pixels> {
    let mut levels: Vec<Pixels> = Vec::new();
    loop {
        let from = levels.last().unwrap_or(base);
        let [width, height] = from.size;
        if width <= 1 && height <= 1 {
            return levels;
        }
        let next = halved(from);
        levels.push(next);
    }
}

fn halved(from: &Pixels) -> Pixels {
    let [width, height] = from.size;
    let size = [(width / 2).max(1), (height / 2).max(1)];
    let texel = |x: u32, y: u32| {
        from.rgba
            .get((y.min(height - 1) * width + x.min(width - 1)) as usize)
            .copied()
            .unwrap_or([0, 0, 0, 0])
    };
    let mut rgba = Vec::with_capacity((size[0] * size[1]) as usize);
    for y in 0..size[1] {
        for x in 0..size[0] {
            let mut sums = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let texel = texel(x * 2 + dx, y * 2 + dy);
                let alpha = u32::from(texel[3]);
                for channel in 0..3 {
                    sums[channel] += u32::from(texel[channel]) * alpha;
                }
                sums[3] += alpha;
            }
            let colour = |sum: u32| match sums[3] {
                0 => 0,
                alpha => ((sum + alpha / 2) / alpha) as u8,
            };
            rgba.push([
                colour(sums[0]),
                colour(sums[1]),
                colour(sums[2]),
                ((sums[3] + 2) / 4) as u8,
            ]);
        }
    }
    Pixels { size, rgba }
}

fn bilinear(pixels: &Pixels, uv: [f32; 2]) -> [f32; 4] {
    let size = pixels.size;
    let x = uv[0] * size[0] as f32 - 0.5;
    let y = uv[1] * size[1] as f32 - 0.5;
    let left = x.floor();
    let top = y.floor();
    let fraction_x = x - left;
    let fraction_y = y - top;
    let mut total = [0.0; 4];
    for (offset_y, weight_y) in [(0.0, 1.0 - fraction_y), (1.0, fraction_y)] {
        for (offset_x, weight_x) in [(0.0, 1.0 - fraction_x), (1.0, fraction_x)] {
            let weight = weight_x * weight_y;
            if weight == 0.0 {
                continue;
            }
            let texel = texel(pixels, left + offset_x, top + offset_y);
            for (total, texel) in total.iter_mut().zip(texel) {
                *total += texel as f32 * weight;
            }
        }
    }
    total
}

fn texel(pixels: &Pixels, x: f32, y: f32) -> [u8; 4] {
    let size = pixels.size;
    let x = (x.max(0.0) as u32).min(size[0].saturating_sub(1));
    let y = (y.max(0.0) as u32).min(size[1].saturating_sub(1));
    pixels
        .rgba
        .get((y * size[0] + x) as usize)
        .copied()
        .unwrap_or([0, 0, 0, 0])
}

fn is_top_left(edge: [[f32; 2]; 2]) -> bool {
    let [start, end] = edge;
    (start[1] == end[1] && end[0] < start[0]) || end[1] < start[1]
}

fn edge(a: [f32; 2], b: [f32; 2], point: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0])
}
