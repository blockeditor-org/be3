use crate::{Content, Fade, Frame, Primitive, Turn};

/// Whole pixels, `[left, top, right, bottom)`.
type Pixels = [i64; 4];

const NOTHING: Pixels = [0, 0, 0, 0];

impl Frame {
    /// Rewrites the frame into the plainest one that `rasterize` paints the same pixels from,
    /// over the frame's own background, so that two frames that look the same are more often
    /// equal too:
    ///
    /// - a turn by no angle has no pivot, and a fade keeps only the edges it ramps;
    /// - a clip is cut to the frame, and one that cuts nothing becomes the whole frame;
    /// - a primitive is dropped when it touches no pixel, is transparent, lies under a later
    ///   opaque rect, or paints its colour over pixels already that colour.
    pub fn tidy(&mut self) {
        let scale = self.pixels_per_point;
        if !(scale > 0.0) {
            return;
        }
        let size = self.size.map(|side| i64::from(side.max(1)));
        let whole = [0.0, 0.0, size[0] as f32 / scale, size[1] as f32 / scale];
        for primitive in &mut self.primitives {
            plain(primitive);
            primitive.clip = intersect(primitive.clip, whole);
        }

        let touched: Vec<Pixels> = self
            .primitives
            .iter()
            .map(|primitive| touched(&primitive.content, Some(primitive.clip), scale, size))
            .collect();
        let covers: Vec<Option<[Pixels; 2]>> = self
            .primitives
            .iter()
            .map(|primitive| cover(primitive, scale))
            .collect();
        let mut kept: Vec<bool> = self
            .primitives
            .iter()
            .zip(&touched)
            .map(|(primitive, touched)| {
                matches!(primitive.content, Content::Callback(_))
                    || (!is_empty(*touched) && !transparent(primitive))
            })
            .collect();
        for index in 0..self.primitives.len() {
            if !kept[index] || matches!(self.primitives[index].content, Content::Callback(_)) {
                continue;
            }
            let area = touched[index];
            let hidden = covers[index + 1..]
                .iter()
                .flatten()
                .any(|cover| cover.iter().any(|cover| inside(area, *cover)));
            if hidden || self.over_its_colour(index, &touched, &covers, &kept) {
                kept[index] = false;
            }
        }

        let mut kept = kept.into_iter();
        self.primitives.retain(|_| kept.next() == Some(true));
        for primitive in &mut self.primitives {
            if !clips(primitive, scale, size) {
                primitive.clip = whole;
            }
        }
    }

    /// Whether the primitive at `index` paints only over pixels already its colour: the
    /// nearest primitive below that touches any of them is an opaque rect of that colour
    /// covering them all, or there is none and the background is that colour.
    fn over_its_colour(
        &self,
        index: usize,
        touched: &[Pixels],
        covers: &[Option<[Pixels; 2]>],
        kept: &[bool],
    ) -> bool {
        let Some(paint) = colour(&self.primitives[index].content) else {
            return false;
        };
        let area = touched[index];
        let below = (0..index)
            .rev()
            .find(|&below| kept[below] && !is_empty(intersect_pixels(touched[below], area)));
        match below {
            Some(below) => covers[below].is_some_and(|cover| {
                colour(&self.primitives[below].content)
                    .is_some_and(|below| below[..3] == paint[..3])
                    && cover.iter().any(|cover| inside(area, *cover))
            }),
            None => self.background[3] == 255 && self.background[..3] == paint[..3],
        }
    }
}

fn plain(primitive: &mut Primitive) {
    let still = |turn: &mut Turn| {
        if !turn.turns() {
            *turn = Turn::NONE;
        }
    };
    match &mut primitive.content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => still(&mut shape.turn),
        Content::Glyph(glyph) => still(&mut glyph.turn),
        Content::Mesh(_) | Content::Callback(_) => {}
    }
    for round in &mut primitive.rounds {
        still(&mut round.turn);
    }
    let fade = &mut primitive.fade;
    if fade.is_none() {
        *fade = Fade::NONE;
        return;
    }
    for side in 0..4 {
        if fade.widths[side] <= 0.0 {
            fade.widths[side] = 0.0;
            fade.rect[side] = 0.0;
        }
    }
}

/// The pixels `rasterize` may change for the content inside `clip`, or every pixel it may
/// change without one.
fn touched(content: &Content, clip: Option<[f32; 4]>, scale: f32, size: [i64; 2]) -> Pixels {
    let frame = [0, 0, size[0], size[1]];
    let clip = clip.map_or([f32::MIN, f32::MIN, f32::MAX, f32::MAX], |clip| {
        clip.map(|value| value * scale)
    });
    let covered = |bounds: [f32; 4]| {
        intersect_pixels(
            [
                bounds[0].max(clip[0]).floor() as i64,
                bounds[1].max(clip[1]).floor() as i64,
                bounds[2].min(clip[2]).ceil() as i64,
                bounds[3].min(clip[3]).ceil() as i64,
            ],
            frame,
        )
    };
    match content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => {
            let rect = shape.rect.map(|value| value * scale);
            let turn = shape.turn.scaled(scale);
            match turn.turns() {
                // A pixel is covered at all only when its centre is less than half a pixel
                // outside the rect (or its stroke, which lies inside it).
                false => covered(rect),
                true => covered(turn.swept([
                    rect[0] - 1.0,
                    rect[1] - 1.0,
                    rect[2] + 1.0,
                    rect[3] + 1.0,
                ])),
            }
        }
        Content::Glyph(glyph) => {
            let rect = glyph.rect.map(|value| (value * scale).round());
            covered(glyph.turn.scaled(scale).swept(rect))
        }
        Content::Mesh(triangles) => {
            let clip = whole_pixels(clip);
            triangles
                .iter()
                .map(|triangle| {
                    let points = triangle
                        .corners
                        .map(|corner| [corner.pos[0] * scale, corner.pos[1] * scale]);
                    let low = |axis: usize| {
                        points
                            .iter()
                            .map(|point| point[axis])
                            .fold(f32::MAX, f32::min)
                    };
                    let high = |axis: usize| {
                        points
                            .iter()
                            .map(|point| point[axis])
                            .fold(f32::MIN, f32::max)
                    };
                    let bounds = whole_pixels([low(0), low(1), high(0), high(1)]);
                    intersect_pixels(intersect_pixels(bounds, clip), frame)
                })
                .fold(NOTHING, union)
        }
        // A callback's outline ignores its clip.
        Content::Callback(rect) => {
            intersect_pixels(whole_pixels(rect.map(|value| value * scale)), frame)
        }
    }
}

/// The pixels an opaque, unturned, unfaded rect paints wholly in its colour, as two boxes:
/// the rect without the columns its corners round, and without the rows.
fn cover(primitive: &Primitive, scale: f32) -> Option<[Pixels; 2]> {
    let (shape, corners) = match &primitive.content {
        Content::RoundedRect(shape) => (shape, [shape.corner_radius; 4]),
        Content::CorneredRect(shape, corners) => (shape, *corners),
        _ => return None,
    };
    let opaque = shape.color[3] == 255
        && shape.stroke_width <= 0.0
        && !shape.turn.turns()
        && primitive.fade == Fade::NONE
        && primitive.rounds.is_empty();
    if !opaque {
        return None;
    }
    // A pixel is wholly covered when its centre is at least half a pixel inside the rect, and
    // drawn when its centre is inside the clip; a hundredth of a pixel keeps rounding out.
    const SPARE: f32 = 0.01;
    let rect = shape.rect.map(|value| value * scale);
    let clip = primitive.clip.map(|value| value * scale);
    let radius = corners.iter().copied().fold(0.0f32, f32::max).max(0.0) * scale;
    let inner = |inset: [f32; 2]| -> Pixels {
        let near = |axis: usize| {
            (rect[axis] + inset[axis] + SPARE)
                .ceil()
                .max((clip[axis] - 0.5 + SPARE).ceil()) as i64
        };
        let far = |axis: usize| {
            (rect[axis + 2] - inset[axis] - SPARE)
                .floor()
                .min((clip[axis + 2] + 0.5 - SPARE).floor()) as i64
        };
        [near(0), near(1), far(0), far(1)]
    };
    Some([inner([radius, 0.0]), inner([0.0, radius])])
}

/// Whether the clip leaves out any pixel the content would change without it.
fn clips(primitive: &Primitive, scale: f32, size: [i64; 2]) -> bool {
    let area = touched(&primitive.content, None, scale, size);
    let clip = primitive.clip.map(|value| value * scale);
    match &primitive.content {
        Content::Callback(_) => false,
        Content::Mesh(_) => !inside(area, whole_pixels(clip)),
        // The others keep the pixels whose centres lie in the clip.
        _ => !inside(
            area,
            [
                (clip[0] - 0.5).ceil() as i64,
                (clip[1] - 0.5).ceil() as i64,
                (clip[2] - 0.5).floor() as i64 + 1,
                (clip[3] - 0.5).floor() as i64 + 1,
            ],
        ),
    }
}

fn transparent(primitive: &Primitive) -> bool {
    if primitive.fade.opacity <= 0.0 {
        return true;
    }
    match &primitive.content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => shape.color[3] == 0,
        Content::Glyph(glyph) => glyph.color[3] == 0,
        Content::Mesh(triangles) => triangles
            .iter()
            .all(|triangle| triangle.corners.iter().all(|corner| corner.color[3] == 0)),
        Content::Callback(_) => false,
    }
}

/// The one colour a primitive paints in, at whatever coverage.
fn colour(content: &Content) -> Option<[u8; 4]> {
    match content {
        Content::RoundedRect(shape) | Content::CorneredRect(shape, _) => Some(shape.color),
        Content::Glyph(glyph) => Some(glyph.color),
        Content::Mesh(_) | Content::Callback(_) => None,
    }
}

fn whole_pixels(rect: [f32; 4]) -> Pixels {
    [
        rect[0].floor() as i64,
        rect[1].floor() as i64,
        rect[2].ceil() as i64,
        rect[3].ceil() as i64,
    ]
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

fn intersect_pixels(a: Pixels, b: Pixels) -> Pixels {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

fn union(a: Pixels, b: Pixels) -> Pixels {
    match (is_empty(a), is_empty(b)) {
        (true, _) => b,
        (_, true) => a,
        _ => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    }
}

fn is_empty(pixels: Pixels) -> bool {
    pixels[2] <= pixels[0] || pixels[3] <= pixels[1]
}

fn inside(inner: Pixels, outer: Pixels) -> bool {
    is_empty(inner)
        || (inner[0] >= outer[0]
            && inner[1] >= outer[1]
            && inner[2] <= outer[2]
            && inner[3] <= outer[3])
}
