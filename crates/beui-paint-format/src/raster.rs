use crate::{Content, Fade, Frame, Glyph, Pixels, Round, RoundedRect, TextureKey, Vertex};

pub fn rasterize<'a>(
    frame: &Frame,
    background: [u8; 4],
    textures: impl Fn(TextureKey) -> Option<&'a Pixels>,
) -> Result<Pixels, String> {
    let [width, height] = frame.size.map(|side| side.max(1));
    let mut canvas = Canvas {
        width,
        height,
        pixels: vec![premultiplied(background); (width * height) as usize],
        fade: Fade::NONE,
        rounds: Vec::new(),
    };
    let sampler = |key: TextureKey| {
        textures(key)
            .map(Sampler)
            .ok_or("the painting is missing a texture it uses")
    };
    let scale = frame.pixels_per_point;
    for primitive in &frame.primitives {
        canvas.fade = primitive.fade.scaled(scale);
        canvas.rounds = primitive
            .rounds
            .iter()
            .map(|round| round.scaled(scale))
            .collect();
        let clip = canvas.scaled(primitive.clip, scale);
        match &primitive.content {
            Content::Mesh(triangles) => {
                for triangle in triangles {
                    canvas.fill(clip, triangle.corners, scale, &sampler(triangle.texture)?);
                }
            }
            Content::Callback(rect) => {
                let rect = canvas.scaled(*rect, scale);
                canvas.outline(rect);
            }
            Content::RoundedRect(shape) => {
                let corners = [shape.corner_radius; 4];
                canvas.rounded(exact(primitive.clip, scale), shape, corners, scale);
            }
            Content::CorneredRect(shape, corners) => {
                canvas.rounded(exact(primitive.clip, scale), shape, *corners, scale);
            }
            Content::Glyph(glyph) => {
                canvas.stamp(
                    exact(primitive.clip, scale),
                    glyph,
                    scale,
                    &sampler(glyph.texture)?,
                );
            }
        }
    }
    Ok(Pixels {
        size: [width, height],
        rgba: canvas.pixels.into_iter().map(straight).collect(),
    })
}

fn premultiplied(color: [u8; 4]) -> [u8; 4] {
    let alpha = color[3] as f32 / 255.0;
    [
        (color[0] as f32 * alpha).round() as u8,
        (color[1] as f32 * alpha).round() as u8,
        (color[2] as f32 * alpha).round() as u8,
        color[3],
    ]
}

fn straight(color: [u8; 4]) -> [u8; 4] {
    match color[3] {
        0 => [0; 4],
        255 => color,
        alpha => {
            let unmultiply =
                |channel: u8| (channel as f32 * 255.0 / alpha as f32).round().min(255.0) as u8;
            [
                unmultiply(color[0]),
                unmultiply(color[1]),
                unmultiply(color[2]),
                alpha,
            ]
        }
    }
}

fn exact(rect: [f32; 4], scale: f32) -> [f32; 4] {
    rect.map(|value| value * scale)
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 4]>,
    fade: Fade,
    rounds: Vec<Round>,
}

impl Canvas {
    fn pixel(&mut self, x: u32, y: u32) -> &mut [u8; 4] {
        &mut self.pixels[(y * self.width + x) as usize]
    }

    fn blend(&mut self, x: u32, y: u32, color: [f32; 3], alpha: f32) {
        let point = [x as f32 + 0.5, y as f32 + 0.5];
        let alpha = self
            .rounds
            .iter()
            .fold(alpha * self.fade.alpha(point), |alpha, round| {
                alpha * covered(round, point)
            });
        let target = self.pixel(x, y);
        for (target, source) in target.iter_mut().zip(color) {
            let blended = source * alpha + *target as f32 * (1.0 - alpha);
            *target = blended.round().clamp(0.0, 255.0) as u8;
        }
        let covered = 255.0 * alpha + target[3] as f32 * (1.0 - alpha);
        target[3] = covered.round().clamp(0.0, 255.0) as u8;
    }

    fn scaled(&self, rect: [f32; 4], scale: f32) -> [i64; 4] {
        [
            (rect[0] * scale).floor() as i64,
            (rect[1] * scale).floor() as i64,
            (rect[2] * scale).ceil().min(self.width as f32) as i64,
            (rect[3] * scale).ceil().min(self.height as f32) as i64,
        ]
    }

    fn rounded(&mut self, clip: [f32; 4], shape: &RoundedRect, corners: [f32; 4], scale: f32) {
        let rect = exact(shape.rect, scale);
        let corners = corners.map(|radius| radius * scale);
        let width = shape.stroke_width * scale;
        let turn = shape.turn.scaled(scale);
        let center = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        let extent = [(rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5];
        let bled = turn.swept([rect[0] - 1.0, rect[1] - 1.0, rect[2] + 1.0, rect[3] + 1.0]);
        self.cover(clip, bled, shape.color, |point| {
            let point = turn.undo(point);
            let offset = [point[0] - center[0], point[1] - center[1]];
            let radius = match (offset[0] < 0.0, offset[1] < 0.0) {
                (true, true) => corners[0],
                (false, true) => corners[1],
                (false, false) => corners[2],
                (true, false) => corners[3],
            };
            let mut distance = rounded_distance(offset, extent, radius);
            if width > 0.0 {
                distance = (distance + width * 0.5).abs() - width * 0.5;
            }
            (0.5 - distance).clamp(0.0, 1.0)
        });
    }

    fn stamp(&mut self, clip: [f32; 4], glyph: &Glyph, scale: f32, sampler: &Sampler<'_>) {
        let rect = glyph.rect.map(|value| (value * scale).round());
        let size = [rect[2] - rect[0], rect[3] - rect[1]];
        let turn = glyph.turn.scaled(scale);
        self.cover(clip, turn.swept(rect), glyph.color, |point| {
            let point = turn.undo(point);
            let uv = [
                (point[0] - rect[0]) / size[0],
                (point[1] - rect[1]) / size[1],
            ];
            if !(0.0..=1.0).contains(&uv[0]) || !(0.0..=1.0).contains(&uv[1]) {
                return 0.0;
            }
            sampler.sample(uv)[3] / 255.0
        });
    }

    fn cover(
        &mut self,
        clip: [f32; 4],
        bounds: [f32; 4],
        color: [u8; 4],
        coverage: impl Fn([f32; 2]) -> f32,
    ) {
        let left = bounds[0].max(clip[0]).floor().max(0.0) as u32;
        let top = bounds[1].max(clip[1]).floor().max(0.0) as u32;
        let right = bounds[2]
            .min(clip[2])
            .ceil()
            .min(self.width as f32)
            .max(0.0) as u32;
        let bottom = bounds[3]
            .min(clip[3])
            .ceil()
            .min(self.height as f32)
            .max(0.0) as u32;
        let source = [color[0] as f32, color[1] as f32, color[2] as f32];
        for y in top..bottom {
            for x in left..right {
                let point = [x as f32 + 0.5, y as f32 + 0.5];
                if point[0] < clip[0]
                    || point[1] < clip[1]
                    || point[0] > clip[2]
                    || point[1] > clip[3]
                {
                    continue;
                }
                let alpha = color[3] as f32 / 255.0 * coverage(point);
                if alpha <= 0.0 {
                    continue;
                }
                self.blend(x, y, source, alpha);
            }
        }
    }

    fn fill(&mut self, clip: [i64; 4], corners: [Vertex; 3], scale: f32, sampler: &Sampler<'_>) {
        let mut points = corners.map(|vertex| [vertex.pos[0] * scale, vertex.pos[1] * scale]);
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
        let left = (min_x.floor() as i64).max(clip[0]);
        let right = (max_x.ceil() as i64).min(clip[2]).min(self.width as i64);
        let top = (min_y.floor() as i64).max(clip[1]);
        let bottom = (max_y.ceil() as i64).min(clip[3]).min(self.height as i64);

        let edges = [
            [points[1], points[2]],
            [points[2], points[0]],
            [points[0], points[1]],
        ];

        for y in top.max(0)..bottom {
            for x in left.max(0)..right {
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

    fn outline(&mut self, rect: [i64; 4]) {
        let (width, height) = (self.width as i64, self.height as i64);
        let colour = [255, 0, 255, 255];
        for x in rect[0].max(0)..rect[2].min(width) {
            for y in [rect[1].max(0), (rect[3] - 1).min(height - 1)] {
                if y >= 0 && y < height {
                    *self.pixel(x as u32, y as u32) = colour;
                }
            }
        }
        for y in rect[1].max(0)..rect[3].min(height) {
            for x in [rect[0].max(0), (rect[2] - 1).min(width - 1)] {
                if x >= 0 && x < width {
                    *self.pixel(x as u32, y as u32) = colour;
                }
            }
        }
    }
}

fn covered(round: &Round, point: [f32; 2]) -> f32 {
    let point = round.turn.undo(point);
    let rect = round.rect;
    let center = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
    let extent = [(rect[2] - rect[0]) * 0.5, (rect[3] - rect[1]) * 0.5];
    let offset = [point[0] - center[0], point[1] - center[1]];
    (0.5 - rounded_distance(offset, extent, round.radius)).clamp(0.0, 1.0)
}

fn rounded_distance(point: [f32; 2], extent: [f32; 2], radius: f32) -> f32 {
    let limit = radius.min(extent[0].min(extent[1]));
    let offset = [
        point[0].abs() - extent[0] + limit,
        point[1].abs() - extent[1] + limit,
    ];
    offset[0].max(offset[1]).min(0.0) + offset[0].max(0.0).hypot(offset[1].max(0.0)) - limit
}

struct Sampler<'a>(&'a Pixels);

impl Sampler<'_> {
    fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        let size = self.0.size;
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
                let texel = self.texel(left + offset_x, top + offset_y);
                for (total, texel) in total.iter_mut().zip(texel) {
                    *total += texel as f32 * weight;
                }
            }
        }
        total
    }

    fn texel(&self, x: f32, y: f32) -> [u8; 4] {
        let size = self.0.size;
        let x = (x.max(0.0) as u32).min(size[0].saturating_sub(1));
        let y = (y.max(0.0) as u32).min(size[1].saturating_sub(1));
        self.0
            .rgba
            .get((y * size[0] + x) as usize)
            .copied()
            .unwrap_or([0, 0, 0, 0])
    }
}

fn is_top_left(edge: [[f32; 2]; 2]) -> bool {
    let [start, end] = edge;
    (start[1] == end[1] && end[0] < start[0]) || end[1] < start[1]
}

fn edge(a: [f32; 2], b: [f32; 2], point: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0])
}
