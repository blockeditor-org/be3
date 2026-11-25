//! What a frame draws, as beui-vello's shapes written into a vello_gpu scene: the items each
//! display's list holds (placed in its space's pixels, as `beui_core::draw` places a shape),
//! and the textures they sample.

use std::rc::Rc;

use be_lib_stable_hash::HashMap;
use beui_core::draw::Turn;
use beui_types::{Image, ImageId};
use beui_vello::vello_common::kurbo::{Affine, BezPath, Rect, Stroke};
use beui_vello::vello_common::paint::{ImageSource, PaintType, TextureRegion, Tint};
use beui_vello::vello_common::peniko::{BlendMode, Fill};
use beui_vello::{Canvas, Fade};
use vello_gpu::{Scene, TextureBindings, TextureId, geometry::RectU16};

use crate::Space;

/// The coverage glyph atlas, the colour glyph atlas, and the first of the images.
const COVERAGE: TextureId = TextureId(1);
const COLOURS: TextureId = TextureId(2);
const PICTURES: u64 = 16;

/// A shape placed in its space's pixels: a display list's entry, kept while the display is.
#[derive(Clone)]
pub(crate) enum Item {
    Rect {
        rect: [f32; 4],
        clip: [f32; 4],
        color: [u8; 4],
        radii: [f32; 4],
        stroke: f32,
        turn: Turn,
    },
    Glyph {
        rect: [f32; 4],
        clip: [f32; 4],
        color: [u8; 4],
        /// Where it lies in its atlas, in texels: left, top, width, height.
        region: [u32; 4],
        colored: bool,
        turn: Turn,
    },
    Image {
        rect: [f32; 4],
        clip: [f32; 4],
        source: [f32; 4],
        image: Image,
        /// The level of the image's mip chain it samples.
        level: u32,
        tint: [u8; 4],
        radius: f32,
        smooth: bool,
        turn: Turn,
    },
    Line {
        clip: [f32; 4],
        segment: [f32; 4],
        width: f32,
        color: [u8; 4],
    },
    Punch {
        rect: [f32; 4],
        clip: [f32; 4],
        radius: f32,
        turn: Turn,
    },
    Path {
        path: Rc<BezPath>,
        style: Painted,
        /// From the path's points to the space's pixels.
        transform: Affine,
        clip: [f32; 4],
        color: [u8; 4],
        /// Whether the fill meets the fills beside it on seams (`PathPaint::seamed`): those
        /// that follow it, alike but for their path, are filled with it as one nonzero path, so
        /// the edges they share cancel rather than each blending its own antialiasing.
        seamed: bool,
    },
}

#[derive(Clone, PartialEq)]
pub(crate) enum Painted {
    Fill(Fill),
    Stroke(Stroke),
}

impl Item {
    pub(crate) fn clip(&self) -> [f32; 4] {
        match self {
            Self::Rect { clip, .. }
            | Self::Glyph { clip, .. }
            | Self::Image { clip, .. }
            | Self::Line { clip, .. }
            | Self::Punch { clip, .. }
            | Self::Path { clip, .. } => *clip,
        }
    }

    pub(crate) fn picture(&self) -> Option<(&Image, u32)> {
        match self {
            Self::Image { image, level, .. } => Some((image, *level)),
            _ => None,
        }
    }
}

/// The textures a frame's items sample, as they were when the frame was prepared: a renderer
/// draws with these even after the atlas they came from was renewed.
#[derive(Default)]
pub(crate) struct Textures {
    coverage: Option<wgpu::TextureView>,
    colours: Option<wgpu::TextureView>,
    /// Each image level drawn: its id, view and size.
    pictures: HashMap<(ImageId, u32), (TextureId, wgpu::TextureView, [u32; 2])>,
}

impl Textures {
    pub(crate) fn of(shared: &crate::resources::Shared, using: &[(ImageId, u32)]) -> Self {
        let mut pictures = HashMap::default();
        for (index, (id, level)) in using.iter().enumerate() {
            let Some(picture) = shared.pictures.get(id) else {
                continue;
            };
            let Some((view, size)) = picture
                .levels
                .get((*level as usize).min(picture.levels.len().saturating_sub(1)))
            else {
                continue;
            };
            pictures.insert(
                (*id, *level),
                (TextureId(PICTURES + index as u64), view.clone(), *size),
            );
        }
        Self {
            coverage: Some(shared.atlas.view.clone()),
            colours: shared.colors.as_ref().map(|colors| colors.view.clone()),
            pictures,
        }
    }

    pub(crate) fn bindings(&self) -> TextureBindings {
        let mut bindings = TextureBindings::new();
        if let Some(view) = &self.coverage {
            bindings.insert(COVERAGE, view.clone());
        }
        if let Some(view) = &self.colours {
            bindings.insert(COLOURS, view.clone());
        }
        for (id, view, _) in self.pictures.values() {
            bindings.insert(*id, view.clone());
        }
        bindings
    }
}

/// vello_gpu's scene as the canvas beui's shapes are drawn on.
pub(crate) struct Gpu<'a>(pub(crate) &'a mut Scene);

impl Canvas for Gpu<'_> {
    fn set_transform(&mut self, transform: Affine) {
        self.0.set_transform(transform);
    }
    fn set_paint(&mut self, paint: PaintType) {
        self.0.set_paint(paint);
    }
    fn set_paint_transform(&mut self, transform: Affine) {
        self.0.set_paint_transform(transform);
    }
    fn set_tint(&mut self, tint: Option<Tint>) {
        self.0.set_tint(tint);
    }
    fn set_fill_rule(&mut self, rule: Fill) {
        self.0.set_fill_rule(rule);
    }
    fn set_stroke(&mut self, stroke: Stroke) {
        self.0.set_stroke(stroke);
    }
    fn fill_path(&mut self, path: &BezPath) {
        self.0.fill_path(path);
    }
    fn stroke_path(&mut self, path: &BezPath) {
        self.0.stroke_path(path);
    }
    fn fill_rect(&mut self, rect: &Rect) {
        self.0.fill_rect(rect);
    }
    fn push_clip_rect(&mut self, rect: &Rect) {
        self.0.push_clip_rect(rect);
    }
    fn push_clip_path(&mut self, path: &BezPath) {
        self.0.push_clip_path(path);
    }
    fn pop_clip(&mut self) {
        self.0.pop_clip();
    }
    fn push_layer(&mut self, blend: Option<BlendMode>, opacity: Option<f32>) {
        self.0.push_layer(None, blend, opacity, None, None);
    }
    fn pop_layer(&mut self) {
        self.0.pop_layer();
    }
}

fn shifted(rect: [f32; 4], [x, y]: [f32; 2]) -> [f32; 4] {
    [rect[0] + x, rect[1] + y, rect[2] + x, rect[3] + y]
}

fn turned(base: Affine, turn: Turn) -> Affine {
    base * beui_vello::turn(turn.pivot, turn.cos, turn.sin)
}

/// Draws `items`, placed in `space`, onto the scene, whose pixels are the target's: the
/// document's pixel `origin` lies at its top left corner.
pub(crate) fn run(
    canvas: &mut Gpu<'_>,
    space: &Space,
    items: &[Item],
    origin: [f32; 2],
    textures: &Textures,
) {
    if items.is_empty() {
        return;
    }
    let shift = [
        space.translation[0] - origin[0],
        space.translation[1] - origin[1],
    ];
    let base = Affine::translate((f64::from(shift[0]), f64::from(shift[1])));
    let bound = shifted(space.clip, [-origin[0], -origin[1]]);
    let fade = Fade {
        rect: shifted(space.fade, [-origin[0], -origin[1]]),
        widths: space.widths,
    };
    canvas.set_transform(Affine::IDENTITY);
    canvas.push_clip_rect(&beui_vello::rect(bound));
    if fade.fades() {
        beui_vello::begin_fade(canvas);
    }
    let mut clipped = None;
    let mut at = 0;
    while at < items.len() {
        let item = &items[at];
        let clip = shifted(item.clip(), shift);
        if clipped != Some(clip) {
            if clipped.is_some() {
                canvas.pop_clip();
            }
            canvas.set_transform(Affine::IDENTITY);
            canvas.push_clip_rect(&beui_vello::rect(clip));
            clipped = Some(clip);
        }
        let joined = items[at + 1..]
            .iter()
            .take_while(|next| seamed_together(item, next))
            .count();
        match joined {
            0 => draw(canvas, item, base, space.opacity, textures),
            _ => {
                let mut path = BezPath::new();
                for joining in &items[at..=at + joined] {
                    if let Item::Path { path: piece, .. } = joining {
                        path.extend(piece.iter());
                    }
                }
                if let Item::Path {
                    transform, color, ..
                } = item
                {
                    beui_vello::filled_path(
                        canvas,
                        &path,
                        Fill::NonZero,
                        *color,
                        base * *transform,
                    );
                }
            }
        }
        at += 1 + joined;
    }
    if clipped.is_some() {
        canvas.pop_clip();
    }
    if fade.fades() {
        beui_vello::end_fade(canvas, fade, bound);
    }
    canvas.pop_clip();
}

/// Whether `next` is a seamed fill alike to the seamed fill `first` but for its path.
fn seamed_together(first: &Item, next: &Item) -> bool {
    match (first, next) {
        (
            Item::Path {
                style,
                transform,
                clip,
                color,
                seamed: true,
                ..
            },
            Item::Path {
                style: next_style,
                transform: next_transform,
                clip: next_clip,
                color: next_color,
                seamed: true,
                ..
            },
        ) => {
            *style == Painted::Fill(Fill::NonZero)
                && style == next_style
                && transform == next_transform
                && clip == next_clip
                && color == next_color
        }
        _ => false,
    }
}

fn draw(canvas: &mut Gpu<'_>, item: &Item, base: Affine, strength: f32, textures: &Textures) {
    match item {
        Item::Rect {
            rect,
            color,
            radii,
            stroke,
            turn,
            ..
        } => beui_vello::filled_rect(canvas, *rect, *radii, *stroke, *color, turned(base, *turn)),
        Item::Glyph {
            rect,
            color,
            region: [left, top, width, height],
            colored,
            turn,
            ..
        } => {
            let id = match colored {
                true => COLOURS,
                false => COVERAGE,
            };
            let region = RectU16::new(
                *left as u16,
                *top as u16,
                (left + width) as u16,
                (top + height) as u16,
            );
            beui_vello::glyph(
                canvas,
                *rect,
                ImageSource::external_texture(id, TextureRegion::Rect(region), true),
                [*width as f32, *height as f32],
                *colored,
                *color,
                turned(base, *turn),
            );
        }
        Item::Image {
            rect,
            source,
            image,
            level,
            tint,
            radius,
            smooth,
            turn,
            ..
        } => {
            let Some((id, _, [width, height])) = textures.pictures.get(&(image.id(), *level))
            else {
                return;
            };
            let (width, height) = (*width as f32, *height as f32);
            let texels = [
                (source[0] * width).round(),
                (source[1] * height).round(),
                (source[2] * width).round(),
                (source[3] * height).round(),
            ];
            let (region, size) = match texels == [0.0, 0.0, width, height] {
                true => (TextureRegion::Full, [width, height]),
                false if texels[2] > texels[0] && texels[3] > texels[1] => (
                    TextureRegion::Rect(RectU16::new(
                        texels[0] as u16,
                        texels[1] as u16,
                        texels[2] as u16,
                        texels[3] as u16,
                    )),
                    [texels[2] - texels[0], texels[3] - texels[1]],
                ),
                false => return,
            };
            beui_vello::image(
                canvas,
                *rect,
                ImageSource::external_texture(*id, region, true),
                size,
                *tint,
                *radius,
                *smooth,
                turned(base, *turn),
            );
        }
        Item::Line {
            segment,
            width,
            color,
            ..
        } => beui_vello::segment(canvas, *segment, *width, *color, base),
        Item::Punch {
            rect, radius, turn, ..
        } => beui_vello::punch(canvas, *rect, *radius, strength, turned(base, *turn)),
        Item::Path {
            path,
            style,
            transform,
            color,
            ..
        } => match style {
            Painted::Fill(rule) => {
                beui_vello::filled_path(canvas, path, *rule, *color, base * *transform)
            }
            Painted::Stroke(stroke) => {
                beui_vello::stroked_path(canvas, path, stroke.clone(), *color, base * *transform)
            }
        },
    }
}
