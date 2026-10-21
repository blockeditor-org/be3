use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use block_editor_beui::be_block::canvas::{
    CanvasEntity, CanvasShape, CanvasTextAlign, CanvasTextVerticalAlign, CanvasTextWeight,
};
use block_editor_beui::be_block::{BlockContent, ImageContent};
use block_editor_beui::beui::painting::{Punches, painting, rasterize};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{
    Color32, Context, FontId, FreetypeFonts, Galley, Image, LineCap, LineJoin, NodeId, Painter,
    Path, PathElement, Pos2, RawInput, Rect, Stroke, TextAlign, TextLayout, Vec2, line_height,
    pos2,
};
use block_editor_beui::{FileSaver, SavedFile};
use uuid::Uuid;

use crate::geometry::{TEXT_INSET, entity_bounds};

use super::paint::{Camera, EntityPaint, Palette, label, resolve_color, with_opacity};
use super::sketch::{Ink, Mark, Sketch};
use super::state::CanvasState;
use super::text::{font_px, text_box};

pub(crate) const PADDING: f32 = 16.0;
pub(crate) const SCALES: [f32; 3] = [1.0, 2.0, 3.0];
const MAX_SIDE: f32 = 16_384.0;
const MAX_PIXELS: f32 = 64.0 * 1024.0 * 1024.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportFormat {
    Png,
    Svg,
}

impl ExportFormat {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Svg => "SVG",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }

    fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportTo {
    Clipboard,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExportOptions {
    pub(crate) scale: f32,
    pub(crate) background: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            scale: 2.0,
            background: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Embed {
    pub(crate) title: String,
    pub(crate) glyph: Option<String>,
    pub(crate) image: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Scene {
    pub(crate) entities: Vec<CanvasEntity>,
    pub(crate) area: Rect,
    pub(crate) clipped: bool,
    pub(crate) background: Color32,
    pub(crate) palette: Palette,
    pub(crate) embeds: BTreeMap<Uuid, Embed>,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImageFile {
    pub(crate) media_type: String,
    pub(crate) data: Vec<u8>,
}

pub(crate) type Images = BTreeMap<Uuid, ImageFile>;

impl Scene {
    pub(crate) fn of(state: &CanvasState, palette: Palette, background: Color32) -> Option<Self> {
        let all: Vec<CanvasEntity> = state
            .entities
            .all_untracked()
            .into_iter()
            .filter(|entity| !entity.hidden)
            .collect();
        let selected = untrack(|| state.selected_entities());
        let (entities, area, clipped, background, name) = match selected.as_slice() {
            [frame] if frame.is_frame() => {
                let area = world_rect(entity_bounds(frame));
                let inside = all
                    .into_iter()
                    .filter(|entity| !entity.is_frame() && reach(entity).intersects(area))
                    .collect();
                let background = frame
                    .style
                    .filled()
                    .map_or(background, |fill| resolve_color(fill, background));
                let name = match frame.text.trim().is_empty() {
                    true => "Frame".to_owned(),
                    false => frame.text.trim().to_owned(),
                };
                (inside, area, true, background, name)
            }
            _ => {
                let chosen: Vec<CanvasEntity> = match selected.is_empty() {
                    true => all,
                    false => selected
                        .into_iter()
                        .filter(|entity| !entity.hidden)
                        .collect(),
                };
                let chosen: Vec<CanvasEntity> = chosen
                    .into_iter()
                    .filter(|entity| !entity.is_frame())
                    .collect();
                let area = chosen
                    .iter()
                    .map(reach)
                    .reduce(|all, next| all.union(next))?;
                let name = state
                    .editor()
                    .blocks()
                    .info(state.editor().block_id())
                    .and_then(|info| info.name())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "Canvas".to_owned());
                (chosen, area.expand(PADDING), false, background, name)
            }
        };
        if entities.is_empty() {
            return None;
        }
        let embeds = entities
            .iter()
            .filter(|entity| entity.shape.embeds_block())
            .filter_map(|entity| {
                let block = entity.block?;
                let label = state.label_of(block);
                Some((
                    block,
                    Embed {
                        title: label
                            .as_ref()
                            .map(|label| label.name.clone())
                            .unwrap_or_else(|| state.unlabeled()),
                        glyph: label
                            .as_ref()
                            .and_then(block_editor_beui::label_glyph)
                            .map(str::to_owned),
                        image: state.block_type_of(block) == Some(ImageContent::CONTENT_TYPE),
                    },
                ))
            })
            .collect();
        Some(Self {
            entities,
            area,
            clipped,
            background,
            palette,
            embeds,
            name,
        })
    }

    fn images(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.embeds
            .iter()
            .filter(|(_, embed)| embed.image)
            .map(|(block, _)| *block)
    }

    fn file_name(&self, format: ExportFormat) -> String {
        let stem: String = self
            .name
            .chars()
            .map(|character| match character {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                character => character,
            })
            .collect();
        format!("{stem}.{}", format.extension())
    }
}

fn world_rect(bounds: crate::geometry::WorldRect) -> Rect {
    Rect::from_min_max(
        pos2(bounds.min.x, bounds.min.y),
        pos2(bounds.max.x, bounds.max.y),
    )
}

fn reach(entity: &CanvasEntity) -> Rect {
    match entity.shape {
        CanvasShape::Text => text_box(entity),
        _ => super::canvas::item_bounds(entity),
    }
}

#[derive(Clone, PartialEq)]
struct Job {
    scene: Scene,
    format: ExportFormat,
    to: ExportTo,
    options: ExportOptions,
}

#[derive(Clone, Copy)]
pub(crate) struct Exporter {
    state: CanvasState,
    pub(crate) options: Signal<ExportOptions>,
    pub(crate) dialog: Signal<Option<ExportFormat>>,
    job: Signal<Option<Job>>,
    palette: Memo<Palette>,
    background: Memo<Color32>,
}

impl Exporter {
    pub(crate) fn new(
        state: CanvasState,
        palette: Memo<Palette>,
        background: Memo<Color32>,
    ) -> Self {
        let exporter = Self {
            state,
            options: create_signal(ExportOptions::default()),
            dialog: create_signal(None),
            job: create_signal(None),
            palette,
            background,
        };
        let saver = Rc::new(RefCell::new(FileSaver::default()));
        let saving = Rc::clone(&saver);
        let editor = state.editor();
        let job = exporter.job;
        create_effect(move || {
            let Some(wanted) = job.get() else {
                return;
            };
            let mut images = Images::new();
            for block in wanted.scene.images() {
                let read = editor
                    .content_of::<ImageContent>(block)
                    .read(|image| ImageFile {
                        media_type: image.header().media_type.clone(),
                        data: image.data().to_vec(),
                    });
                match read {
                    Ok(file) => {
                        images.insert(block, file);
                    }
                    Err(Pending::Loading) => return,
                    Err(Pending::Failed(_)) => {}
                }
            }
            untrack(|| {
                job.set(None);
                let made = match wanted.format {
                    ExportFormat::Png => png(&wanted.scene, wanted.options, &images),
                    ExportFormat::Svg => {
                        Ok(svg(&wanted.scene, wanted.options, &images).into_bytes())
                    }
                };
                let data = match made {
                    Ok(data) => data,
                    Err(error) => {
                        state
                            .import_error
                            .set(Some(format!("Could not export: {error}")));
                        return;
                    }
                };
                match (wanted.to, wanted.format) {
                    (ExportTo::Clipboard, ExportFormat::Png) => copy_image(data),
                    (ExportTo::Clipboard, ExportFormat::Svg) => {
                        copy_text(String::from_utf8_lossy(&data).into_owned())
                    }
                    (ExportTo::File, format) => saving.borrow_mut().save(
                        &editor.host(),
                        SavedFile {
                            name: wanted.scene.file_name(format),
                            mime_type: format.mime_type().to_owned(),
                            data,
                        },
                    ),
                }
            });
        });
        let host = editor.host();
        editor.on_reply(move || {
            if let Some(Err(failure)) = saver.borrow_mut().poll(&host) {
                state
                    .import_error
                    .set(Some(format!("Could not export: {failure}")));
            }
        });
        exporter
    }

    pub(crate) fn run(&self, format: ExportFormat, to: ExportTo) {
        let scene = Scene::of(
            &self.state,
            self.palette.get_untracked(),
            self.background.get_untracked(),
        );
        let Some(scene) = scene else {
            return;
        };
        self.state.import_error.set(None);
        self.job.set(Some(Job {
            scene,
            format,
            to,
            options: self.options.get_untracked(),
        }));
    }

    pub(crate) fn ask(&self, format: ExportFormat) {
        self.dialog.set(Some(format));
    }
}

pub(crate) fn png(
    scene: &Scene,
    options: ExportOptions,
    images: &Images,
) -> Result<Vec<u8>, String> {
    let scale = options.scale.max(0.1);
    let size = scene.area.size();
    let pixels = size * scale;
    if pixels.x > MAX_SIDE || pixels.y > MAX_SIDE || pixels.x * pixels.y > MAX_PIXELS {
        return Err(format!(
            "{} by {} pixels is too large an image",
            pixels.x.ceil(),
            pixels.y.ceil()
        ));
    }
    let decoded: BTreeMap<Uuid, Image> = images
        .iter()
        .filter_map(|(block, file)| Some((*block, decode(&file.data)?)))
        .collect();
    let context = Context::new(FreetypeFonts::new(block_editor_beui::fonts::fonts()));
    context.set_pixels_per_point(scale);
    let output = context.run(RawInput::default(), |context| {
        let whole = Rect::from_min_size(Pos2::ZERO, size);
        let painter = context.painter();
        let painter = match scene.clipped {
            true => painter.with_clip_rect(whole),
            false => painter,
        };
        if options.background {
            painter.rect_filled(whole, 0.0, scene.background);
        }
        let camera = Camera::item(scene.area.min);
        for entity in &scene.entities {
            draw(&painter, scene, entity, camera, &decoded);
        }
    });
    let painted = painting(&output, size, scale, Color32::TRANSPARENT, Punches::Shown)?;
    let raster = rasterize(&painted.frame, [0; 4], |key| painted.textures.get(&key))?;
    encode_png(raster.size, raster.rgba.into_iter().flatten().collect())
}

fn decode(data: &[u8]) -> Option<Image> {
    let image = image::load_from_memory(data).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(Image::from_rgba(width, height, image.into_raw()))
}

fn encode_png(size: [u32; 2], rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_vec(size[0], size[1], rgba)
        .ok_or("the rendered image does not match its size")?;
    let mut data = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut data),
            image::ImageFormat::Png,
        )
        .map_err(|error| error.to_string())?;
    Ok(data)
}

fn draw(
    painter: &Painter,
    scene: &Scene,
    entity: &CanvasEntity,
    camera: Camera,
    images: &BTreeMap<Uuid, Image>,
) {
    let opacity = entity.style.opacity.clamp(0.0, 1.0);
    if entity.shape == CanvasShape::Text {
        let color = with_opacity(
            resolve_color(entity.style.stroke_color, scene.palette.auto),
            opacity,
        );
        let turned = painter.rotated(
            camera.at(entity.transform.center),
            entity.transform.rotation,
        );
        for (at, galley) in text_lines(painter, entity, camera) {
            turned.galley(at, galley, color);
        }
        return;
    }
    let embed = entity.block.and_then(|block| scene.embeds.get(&block));
    if let Some(image) = entity.block.and_then(|block| images.get(&block)) {
        let rect = camera.rect(crate::geometry::WorldRect::from_points(
            entity_corner(entity, -0.5),
            entity_corner(entity, 0.5),
        ));
        painter
            .rotated(rect.center(), entity.transform.rotation)
            .image(
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                image,
                with_opacity(Color32::WHITE, opacity),
                0.0,
                true,
            );
        return;
    }
    EntityPaint {
        entity: entity.clone(),
        camera,
        palette: scene.palette.clone(),
        title: embed.map(|embed| embed.title.clone()).unwrap_or_default(),
        glyph: embed.and_then(|embed| embed.glyph.clone()),
        automatic: false,
        covered: false,
        placeholder: None,
        sketch: Rc::new(Sketch::of(entity).translated(Vec2::new(camera.origin.x, camera.origin.y))),
    }
    .draw(painter);
}

fn entity_corner(
    entity: &CanvasEntity,
    at: f32,
) -> block_editor_beui::be_block::canvas::CanvasPoint {
    let transform = entity.transform;
    block_editor_beui::be_block::canvas::CanvasPoint::new(
        transform.center.x + transform.size.x * at,
        transform.center.y + transform.size.y * at,
    )
}

fn text_lines(painter: &Painter, entity: &CanvasEntity, camera: Camera) -> Vec<(Pos2, Galley)> {
    let text = match entity.text.is_empty() {
        true => return Vec::new(),
        false => &entity.text,
    };
    let style = entity.text_style;
    let px = font_px(entity, 1.0) * camera.scale;
    let rect = camera.rect(crate::geometry::WorldRect::from_points(
        entity_corner(entity, -0.5),
        entity_corner(entity, 0.5),
    ));
    let inset = TEXT_INSET * camera.scale;
    let inner = rect.shrink(inset);
    let galley = painter.layout_text(
        text,
        FontId::proportional(px).bold(style.weight == CanvasTextWeight::Bold),
        TextLayout {
            wrap_width: match style.wrap {
                true => inner.width().max(1.0),
                false => f32::INFINITY,
            },
            align: match style.alignment {
                CanvasTextAlign::Left => TextAlign::Start,
                CanvasTextAlign::Center => TextAlign::Center,
                CanvasTextAlign::Right => TextAlign::End,
            },
            line_height: Some(line_height(px) * style.line_height.max(0.5)),
            ..TextLayout::DEFAULT
        },
    );
    let size = galley.size();
    let x = match style.alignment {
        CanvasTextAlign::Left => inner.left(),
        CanvasTextAlign::Center => inner.center().x - size.x / 2.0,
        CanvasTextAlign::Right => inner.right() - size.x,
    };
    let y = match style.vertical_alignment {
        CanvasTextVerticalAlign::Top => inner.top(),
        CanvasTextVerticalAlign::Middle => inner.center().y - size.y / 2.0,
        CanvasTextVerticalAlign::Bottom => inner.bottom() - size.y,
    };
    vec![(pos2(x, y), galley)]
}

pub(crate) fn svg(scene: &Scene, options: ExportOptions, images: &Images) -> String {
    let area = scene.area;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\">\n",
        x = number(area.min.x),
        y = number(area.min.y),
        w = number(area.width()),
        h = number(area.height()),
    );
    let whole = format!(
        "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        number(area.min.x),
        number(area.min.y),
        number(area.width()),
        number(area.height())
    );
    if scene.clipped {
        out.push_str(&format!(
            "<defs><clipPath id=\"area\"><rect {whole}/></clipPath></defs>\n<g clip-path=\"url(#area)\">\n"
        ));
    }
    if options.background {
        out.push_str(&format!(
            "<rect {whole}{}/>\n",
            paint("fill", scene.background)
        ));
    }
    let context = Context::new(FreetypeFonts::new(block_editor_beui::fonts::fonts()));
    context.run(RawInput::default(), |context| {
        let painter = context.painter();
        for entity in &scene.entities {
            svg_entity(&mut out, &painter, scene, entity, images);
        }
    });
    if scene.clipped {
        out.push_str("</g>\n");
    }
    out.push_str("</svg>\n");
    out
}

fn svg_entity(
    out: &mut String,
    painter: &Painter,
    scene: &Scene,
    entity: &CanvasEntity,
    images: &Images,
) {
    let style = entity.style;
    let opacity = style.opacity.clamp(0.0, 1.0);
    let ink = with_opacity(
        resolve_color(style.stroke_color, scene.palette.auto),
        opacity,
    );
    let world = Camera::item(Pos2::ZERO);
    let turn = turn(entity);
    if entity.shape == CanvasShape::Text {
        for (at, galley) in text_lines(painter, entity, world) {
            svg_text(out, at, &galley, ink, &turn);
        }
        return;
    }
    if entity.shape.embeds_block() {
        let rect = world.rect(crate::geometry::WorldRect::from_points(
            entity_corner(entity, -0.5),
            entity_corner(entity, 0.5),
        ));
        let place = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{turn}",
            number(rect.min.x),
            number(rect.min.y),
            number(rect.width()),
            number(rect.height())
        );
        match entity.block.and_then(|block| images.get(&block)) {
            Some(image) => out.push_str(&format!(
                "<image {place} preserveAspectRatio=\"none\"{} href=\"data:{};base64,{}\"/>\n",
                opacity_of(opacity),
                escape(&image.media_type),
                base64(&image.data)
            )),
            None => {
                let surface = with_opacity(scene.palette.surface, opacity);
                let border = with_opacity(scene.palette.border, opacity);
                out.push_str(&format!(
                    "<rect {place}{}{}/>\n",
                    paint("fill", surface),
                    paint("stroke", border)
                ));
                let title = entity
                    .block
                    .and_then(|block| scene.embeds.get(&block))
                    .map(|embed| embed.title.clone())
                    .unwrap_or_default();
                let galley = painter.layout(title, FontId::proportional(18.0), rect.width());
                let size = galley.size();
                let at = pos2(
                    rect.center().x - size.x / 2.0,
                    rect.center().y - size.y / 2.0,
                );
                svg_text(out, at, &galley, ink, &turn);
            }
        }
        return;
    }
    let fill = with_opacity(resolve_color(style.fill_color, scene.palette.auto), opacity);
    for part in &Sketch::of(entity).parts {
        let color = match part.ink {
            Ink::Stroke => ink,
            Ink::Fill => fill,
        };
        match &part.mark {
            Mark::Fill(path) => out.push_str(&format!(
                "<path d=\"{}\"{}/>\n",
                path_data(path),
                paint("fill", color)
            )),
            Mark::Stroke(path, stroke) => out.push_str(&format!(
                "<path d=\"{}\" fill=\"none\"{}{}/>\n",
                path_data(path),
                paint("stroke", color),
                stroke_attributes(stroke)
            )),
        }
    }
    if let Some((at, galley)) = label(painter, entity, world) {
        svg_text(out, at, &galley, ink, "");
    }
}

fn turn(entity: &CanvasEntity) -> String {
    let rotation = entity.transform.rotation;
    match rotation == 0.0 {
        true => String::new(),
        false => format!(
            " transform=\"rotate({} {} {})\"",
            number(rotation.to_degrees()),
            number(entity.transform.center.x),
            number(entity.transform.center.y)
        ),
    }
}

fn svg_text(out: &mut String, at: Pos2, galley: &Galley, color: Color32, turn: &str) {
    let text = galley.text();
    let font = galley.font();
    let weight = match font.bold {
        true => " font-weight=\"bold\"",
        false => "",
    };
    out.push_str(&format!(
        "<text font-family=\"sans-serif\" font-size=\"{}\"{weight}{}{turn}>",
        number(font.size),
        paint("fill", color)
    ));
    for line in galley.lines() {
        let shown = text
            .get(line.range.clone())
            .unwrap_or_default()
            .trim_end_matches(['\n', '\r']);
        if shown.is_empty() {
            continue;
        }
        let x = line.cursors.first().map_or(0.0, |cursor| cursor.1);
        out.push_str(&format!(
            "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
            number(at.x + x),
            number(at.y + line.top + galley.baseline()),
            escape(shown)
        ));
    }
    out.push_str("</text>\n");
}

fn path_data(path: &Path) -> String {
    let point = |at: Pos2| format!("{} {}", number(at.x), number(at.y));
    path.elements()
        .iter()
        .map(|element| match *element {
            PathElement::MoveTo(to) => format!("M{}", point(to)),
            PathElement::LineTo(to) => format!("L{}", point(to)),
            PathElement::QuadTo(control, to) => format!("Q{} {}", point(control), point(to)),
            PathElement::CubicTo(first, second, to) => {
                format!("C{} {} {}", point(first), point(second), point(to))
            }
            PathElement::Close => "Z".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn stroke_attributes(stroke: &Stroke) -> String {
    let mut attributes = format!(" stroke-width=\"{}\"", number(stroke.width));
    match stroke.join {
        LineJoin::Miter => {
            if stroke.miter_limit != 4.0 {
                attributes.push_str(&format!(
                    " stroke-miterlimit=\"{}\"",
                    number(stroke.miter_limit)
                ));
            }
        }
        LineJoin::Round => attributes.push_str(" stroke-linejoin=\"round\""),
        LineJoin::Bevel => attributes.push_str(" stroke-linejoin=\"bevel\""),
    }
    match stroke.cap {
        LineCap::Butt => {}
        LineCap::Round => attributes.push_str(" stroke-linecap=\"round\""),
        LineCap::Square => attributes.push_str(" stroke-linecap=\"square\""),
    }
    if stroke.is_dashed() {
        let dashes: Vec<String> = stroke.dashes.iter().map(|dash| number(*dash)).collect();
        attributes.push_str(&format!(" stroke-dasharray=\"{}\"", dashes.join(" ")));
        if stroke.dash_offset != 0.0 {
            attributes.push_str(&format!(
                " stroke-dashoffset=\"{}\"",
                number(stroke.dash_offset)
            ));
        }
    }
    attributes
}

fn paint(attribute: &str, color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_array();
    let mut painted = format!(" {attribute}=\"#{red:02x}{green:02x}{blue:02x}\"");
    if alpha < 255 {
        painted.push_str(&format!(
            " {attribute}-opacity=\"{}\"",
            number(alpha as f32 / 255.0)
        ));
    }
    painted
}

fn opacity_of(opacity: f32) -> String {
    match opacity < 1.0 {
        true => format!(" opacity=\"{}\"", number(opacity)),
        false => String::new(),
    }
}

fn number(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{:.2}", if rounded == 0.0 { 0.0 } else { rounded });
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let joined = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        for index in 0..4 {
            match index <= chunk.len() {
                true => out.push(ALPHABET[(joined >> (18 - 6 * index) & 63) as usize] as char),
                false => out.push('='),
            }
        }
    }
    out
}

const DIALOG_WIDTH: f32 = 320.0;
const DIALOG_SPACING: f32 = 12.0;

#[component]
pub(crate) fn ExportDialog(exporter: Exporter) -> NodeId {
    let dialog = exporter.dialog;
    let options = exporter.options;
    let open = create_memo(move || dialog.get().is_some());
    let raster = create_memo(move || dialog.get() == Some(ExportFormat::Png));
    let title = create_memo(move || {
        let format = dialog.get().unwrap_or(ExportFormat::Png);
        format!("Export as {}", format.name())
    });
    let scale = create_memo(move || {
        let scale = options.get().scale;
        SCALES.iter().position(|chosen| *chosen == scale)
    });
    let choose_scale = move |chosen: Option<usize>| {
        if let Some(scale) = chosen.and_then(|index| SCALES.get(index)) {
            options.update(|options| options.scale = *scale);
        }
    };
    let background = create_memo(move || options.get().background);
    let set_background = move |on: bool| options.update(|options| options.background = on);
    let finish = move |to: ExportTo| {
        if let Some(format) = dialog.get_untracked() {
            dialog.set(None);
            exporter.run(format, to);
        }
    };
    view! {
        <Dialog
            open={open}
            title={title}
            width=DIALOG_WIDTH
            id="infinite-canvas.export"
            confirm="Save…"
            cancel="Cancel"
            on_confirm={move || finish(ExportTo::File)}
            on_dismiss={move || dialog.set(None)}
            extra_actions={move || view! {
                <Button
                    label="Copy"
                    variant=ButtonVariant::Secondary
                    @test_id={"infinite-canvas.export.copy"}
                    on_click={move || finish(ExportTo::Clipboard)}
                />
            }}
        >
            <List spacing=DIALOG_SPACING>
                <Show condition={raster}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=DIALOG_SPACING
                    >
                        <Text @sizing=ItemSize::Percent(100.0)>"Scale"</Text>
                        <Select
                            options={view! {
                                <ChoiceOption label="1x" />
                                <ChoiceOption label="2x" />
                                <ChoiceOption label="3x" />
                            }}
                            selected={Binding::new(scale, choose_scale)}
                            label="Scale"
                            @test_id={"infinite-canvas.export.scale"}
                        />
                    </List>
                </Show>
                <List direction=Direction::Horizontal align=Align::Center spacing=DIALOG_SPACING>
                    <Text @sizing=ItemSize::Percent(100.0)>"Background"</Text>
                    <Switch
                        on={Binding::new(background, set_background)}
                        label="Background"
                        @test_id={"infinite-canvas.export.background"}
                    />
                </List>
            </List>
        </Dialog>
    }
}
