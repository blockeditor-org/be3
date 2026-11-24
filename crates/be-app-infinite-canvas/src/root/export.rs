use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use be_sdk_beui::beui::icons::Glyph;
use be_sdk_beui::beui::native::{core_painter, wrap_painter};
use std::collections::BTreeMap;
use std::rc::Rc;

use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::SpanStyle;
use be_sdk_beui::beui::native::painting::{Punches, painting, rasterize};
use be_sdk_beui::beui::native::rich::{RichLayout, SpanKind};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{
    Color32, FontFamily, FontId, Image, LineCap, LineJoin, NodeId, Path, PathElement, Pos2, Rect,
    Stroke, TextAlign, TextLayout, Vec2, native::line_height, pos2, reactive::Painter,
};
use be_sdk_beui::be_block_content::blob::Blob;
use be_sdk_beui::{Editor, ExportSettings, ExporterOf, Made};
use be_type_workspace::{FileContent, FileHeader};
use serde::{Deserialize, Serialize};
use be_type_infinite_canvas::{
    CanvasEntity, CanvasPoint, CanvasRaster, CanvasShape, CanvasTextWeight,
};
use uuid::Uuid;

use crate::geometry::{TEXT_INSET, entity_bounds, point};
use be_sdk_beui::beui::CanvasView;

use super::paint::{EntityPaint, Palette, resolve_color, with_opacity};
use super::sketch::{Ink, Mark, Sketch};
use super::entities::Entities;
use be_sdk_beui::{BlockInfo, BlockQuery, ContentProjection};
use be_type_infinite_canvas::{Canvas, CanvasContent};
use super::text::{
    font_px, prose, rich_state, text_box, text_region, text_style, text_turn, writes,
};
use crate::raster::nearest_upscaled;
use be_type_image::{ImageContent, ImageHeader};
use beui::{Align, Direction};

pub(crate) const PADDING: f32 = 16.0;
const MAX_SIDE: f32 = 16_384.0;
const MAX_PIXELS: f32 = 64.0 * 1024.0 * 1024.0;

/// What the canvas's PNG and SVG exporters draw, and how (guides/apps/editor.md, "Exporting").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ImageExport {
    /// Pixels per canvas unit, for a PNG.
    pub(crate) scale: f32,
    /// The canvas's background is drawn behind the items.
    pub(crate) background: bool,
    /// The items drawn: every item for none, or what one frame frames.
    pub(crate) items: Vec<Uuid>,
}

impl Default for ImageExport {
    fn default() -> Self {
        Self {
            scale: 2.0,
            background: true,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Embed {
    pub(crate) title: String,
    pub(crate) glyph: Option<Glyph>,
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
    pub(crate) rasters: BTreeMap<Uuid, CanvasRaster>,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImageFile {
    pub(crate) media_type: String,
    pub(crate) data: Vec<u8>,
}

pub(crate) type Images = BTreeMap<Uuid, ImageFile>;

impl Scene {
    /// What `items` (every item for none) of the canvas `state` shows draw, as it is now and
    /// tracking it.
    pub(crate) fn of(
        state: &CanvasReader,
        palette: Palette,
        background: Color32,
        items: &[Uuid],
    ) -> Ready<Self> {
        // Nothing to draw is only known once the canvas has arrived.
        state.content.read(|_| ())?;
        let references = state.references.get()?;
        let all: Vec<CanvasEntity> = state
            .entities
            .all()
            .into_iter()
            .filter(|entity| !entity.hidden)
            .collect();
        let selected: Vec<CanvasEntity> = all
            .iter()
            .filter(|entity| items.contains(&entity.id))
            .cloned()
            .collect();
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
                let name = frame.text.to_str_lossy();
                let name = match name.trim().is_empty() {
                    true => "Frame".to_owned(),
                    false => name.trim().to_owned(),
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
                let Some(area) = chosen.iter().map(reach).reduce(|all, next| all.union(next))
                else {
                    return Err(Pending::Failed("There is nothing to export.".to_owned()));
                };
                let name = state
                    .editor
                    .blocks()
                    .info(state.editor.block_id())
                    .and_then(|info| info.name())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| "Canvas".to_owned());
                (chosen, area.expand(PADDING), false, background, name)
            }
        };
        if entities.is_empty() {
            return Err(Pending::Failed("There is nothing to export.".to_owned()));
        }
        let catalog = state.editor.block_types();
        let embeds = entities
            .iter()
            .filter(|entity| entity.shape.embeds_block())
            .filter_map(|entity| {
                let block = entity.block?;
                let info = references.iter().find(|info| info.id == block);
                let label = info.map(|info| info.label(catalog.as_ref()));
                Some((
                    block,
                    Embed {
                        title: label
                            .as_ref()
                            .map(|label| label.name.clone())
                            .unwrap_or_default(),
                        glyph: label.as_ref().and_then(be_sdk_beui::label_glyph),
                        image: info.is_some_and(|info| {
                            info.block_type == ImageContent::CONTENT_TYPE
                        }),
                    },
                ))
            })
            .collect();
        let rasters = entities
            .iter()
            .filter(|entity| entity.shape == CanvasShape::Raster)
            .filter_map(|entity| {
                let raster = state
                    .content
                    .read(|canvas| Canvas::raster(canvas, entity.id))
                    .ok()
                    .flatten()?;
                Some((entity.id, raster))
            })
            .collect();
        Ok(Self {
            entities,
            area,
            clipped,
            background,
            palette,
            embeds,
            rasters,
            name,
        })
    }

    fn images(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.embeds
            .iter()
            .filter(|(_, embed)| embed.image)
            .map(|(block, _)| *block)
    }

    fn file_name(&self, extension: &str) -> String {
        let stem: String = self
            .name
            .chars()
            .map(|character| match character {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
                character => character,
            })
            .collect();
        format!("{stem}.{extension}")
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

/// The images the scene shows, from the blocks it embeds: loading until each has arrived.
fn images_of(editor: Editor, scene: &Scene) -> Ready<Images> {
    let mut images = Images::new();
    for block in scene.images() {
        let image = editor.content_of::<ImageContent>(block);
        let read = image
            .read(|image| image.header().media_type.clone())
            .and_then(|media_type| {
                let data = image.payload().get()?;
                Ok(ImageFile {
                    media_type,
                    data: data.to_vec(),
                })
            });
        match read {
            Ok(file) => {
                images.insert(block, file);
            }
            Err(Pending::Loading) => return Err(Pending::Loading),
            Err(Pending::Failed(_)) => {}
        }
    }
    Ok(images)
}

/// The canvas as an export reads it: its items, and what the graph says of the blocks it
/// embeds. Unlike `CanvasState` it needs no document, so an artifact instance, which shows
/// nothing, makes one too.
#[derive(Clone, Copy)]
pub(crate) struct CanvasReader {
    editor: Editor,
    content: ContentProjection<CanvasContent>,
    entities: Entities,
    references: Memo<Ready<Vec<BlockInfo>>>,
}

impl CanvasReader {
    fn new(editor: Editor) -> Self {
        let content = editor.block_content::<CanvasContent>();
        let listed = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        Self {
            editor,
            content,
            entities: Entities::new(content),
            references: create_memo(move || listed.read()),
        }
    }
}

/// The colours an export draws in: always the same, so what it makes does not change with the
/// theme of whoever makes it (an artifact is made again on every device that shows it).
fn export_palette() -> (Palette, Color32) {
    let theme = Theme::DARK;
    (
        Palette {
            auto: theme.text,
            surface: theme.surface,
            control: theme.control,
            border: theme.border,
            muted: theme.text_muted,
        },
        theme.background,
    )
}

/// The scene `settings` draws of the canvas `editor` reads, with the images it shows.
fn scenes(editor: Editor) -> impl Fn(&ImageExport) -> Ready<(Scene, Images)> {
    let reader = CanvasReader::new(editor);
    move |settings: &ImageExport| {
        let (palette, background) = export_palette();
        let scene = Scene::of(&reader, palette, background, &settings.items)?;
        let images = images_of(editor, &scene)?;
        Ok((scene, images))
    }
}

/// The settings an export from the open canvas starts from: what is selected.
fn selected(editor: Editor) -> ImageExport {
    ImageExport {
        items: super::state::selection_of(editor.block_id()),
        ..ImageExport::default()
    }
}

fn describe(settings: &ImageExport) -> String {
    match settings.items.is_empty() {
        true => "The whole canvas".to_owned(),
        false => "Part of the canvas".to_owned(),
    }
}

/// The canvas as a PNG image block.
pub(crate) fn png_exporter() -> ExporterOf<ImageContent, ImageExport> {
    ExporterOf::new("png", |editor: Editor| {
        let scenes = scenes(editor);
        move |settings: &ImageExport| {
            let (scene, images) = scenes(settings)?;
            let (data, [width, height]) = png(&scene, settings, &images).map_err(Pending::Failed)?;
            let name = scene.file_name("png");
            let header = ImageHeader {
                source_name: name.clone(),
                media_type: "image/png".to_owned(),
                width,
                height,
                ..ImageHeader::default()
            };
            Ok(Made::named(Blob::new(header, data), name))
        }
    })
    .settings(|settings| Build::new(move || view! { <ImageSettings settings png=true /> }))
    .describe(describe)
    .initial(selected)
}

/// The canvas as an SVG file block.
pub(crate) fn svg_exporter() -> ExporterOf<FileContent, ImageExport> {
    ExporterOf::new("svg", |editor: Editor| {
        let scenes = scenes(editor);
        move |settings: &ImageExport| {
            let (scene, images) = scenes(settings)?;
            let name = scene.file_name("svg");
            let header = FileHeader {
                source_name: name.clone(),
                media_type: "image/svg+xml".to_owned(),
            };
            let data = svg(&scene, settings, &images).into_bytes();
            Ok(Made::named(Blob::new(header, data), name))
        }
    })
    .settings(|settings| Build::new(move || view! { <ImageSettings settings png=false /> }))
    .describe(describe)
    .initial(selected)
}

/// `scene` drawn as a PNG, and its size in pixels.
pub(crate) fn png(
    scene: &Scene,
    options: &ImageExport,
    images: &Images,
) -> Result<(Vec<u8>, [u32; 2]), String> {
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
    let context = be_sdk_beui::context();
    context.set_pixels_per_point(scale);
    let output = context.run(|context| {
        let whole = Rect::from_min_size(Pos2::ZERO, size);
        let painter = wrap_painter(&context.painter());
        let painter = match scene.clipped {
            true => painter.with_clip_rect(whole),
            false => painter,
        };
        if options.background {
            painter.rect_filled(whole, 0.0, scene.background);
        }
        let camera = CanvasView::from_corner(scene.area.min, 1.0);
        for entity in &scene.entities {
            match scene.rasters.get(&entity.id) {
                Some(raster) => draw_raster(&painter, entity, raster, camera, scale),
                None => draw(&painter, scene, entity, camera, &decoded),
            }
        }
    });
    let painted = painting(
        output.main(),
        size,
        scale,
        Color32::TRANSPARENT,
        Punches::Shown,
    )?;
    let raster = rasterize(&painted.frame, [0; 4], |key| painted.textures.get(&key))?;
    let size = raster.size;
    Ok((
        encode_png(size, raster.rgba.into_iter().flatten().collect())?,
        size,
    ))
}

const MAX_RASTER_UPSCALE: u32 = 64;
const MAX_UPSCALED_PIXELS: u32 = 16 * 1024 * 1024;

fn draw_raster(
    painter: &Painter,
    entity: &CanvasEntity,
    raster: &CanvasRaster,
    camera: CanvasView,
    scale: f32,
) {
    if entity.hidden {
        return;
    }
    let rect = camera.rect_to_screen(entity_bounds(entity).rect());
    let (width, height) = (u32::from(raster.width()), u32::from(raster.height()));
    let wanted = (scale
        * (rect.width() / width.max(1) as f32).max(rect.height() / height.max(1) as f32))
    .ceil()
    .max(1.0) as u32;
    let room = (MAX_UPSCALED_PIXELS / (width * height).max(1))
        .isqrt()
        .max(1);
    let factor = wanted.min(MAX_RASTER_UPSCALE).min(room);
    let image = Image::from_rgba(
        width * factor,
        height * factor,
        nearest_upscaled(raster.width(), raster.height(), raster.rgba_bytes(), factor),
    );
    painter.image(
        rect,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        &image,
        with_opacity(Color32::WHITE, entity.style.opacity.clamp(0.0, 1.0)),
        0.0,
        false,
    );
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
    camera: CanvasView,
    images: &BTreeMap<Uuid, Image>,
) {
    let opacity = entity.style.opacity.clamp(0.0, 1.0);
    let ink = with_opacity(
        resolve_color(entity.style.stroke_color, scene.palette.auto),
        opacity,
    );
    let turned = painter.rotated(
        camera.to_screen(point(entity.transform.center)),
        text_turn(entity),
    );
    if entity.shape == CanvasShape::Text {
        paint_written(&turned, entity, ink, camera);
        return;
    }
    let embed = entity.block.and_then(|block| scene.embeds.get(&block));
    if let Some(image) = entity.block.and_then(|block| images.get(&block)) {
        let rect = camera.rect_to_screen(
            crate::geometry::WorldRect::from_points(
                entity_corner(entity, -0.5),
                entity_corner(entity, 0.5),
            )
            .rect(),
        );
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
        glyph: embed.and_then(|embed| embed.glyph),
        automatic: false,
        covered: false,
        placeholder: None,
        raster: None,
        sketch: Rc::new(
            Sketch::of(entity, false).translated(Vec2::new(camera.origin.x, camera.origin.y)),
        ),
    }
    .draw(painter);
    paint_written(&turned, entity, ink, camera);
}

fn paint_written(painter: &Painter, entity: &CanvasEntity, ink: Color32, camera: CanvasView) {
    for Written {
        origin,
        layout,
        missing,
    } in written(painter, entity, ink, camera)
    {
        for rect in missing {
            painter.rect_filled(rect, 0.0, MISSING);
        }
        layout.paint_runs(core_painter(painter), origin, |run| {
            (
                run.style.color.resolve(|_| None),
                run.style.underline,
                run.style.strikethrough,
            )
        });
    }
}

fn entity_corner(entity: &CanvasEntity, at: f32) -> be_type_infinite_canvas::CanvasPoint {
    let transform = entity.transform;
    be_type_infinite_canvas::CanvasPoint::new(
        transform.center.x + transform.size.x * at,
        transform.center.y + transform.size.y * at,
    )
}

struct Written {
    origin: Vec2,
    layout: RichLayout,
    missing: Vec<Rect>,
}

fn written(
    painter: &Painter,
    entity: &CanvasEntity,
    ink: Color32,
    camera: CanvasView,
) -> Vec<Written> {
    if entity.text.is_empty() || !writes(entity) {
        return Vec::new();
    }
    let px = font_px(entity, 1.0) * camera.scale;
    let prose = prose(text_style(entity), px);
    let region = text_region(entity);
    let inner = camera
        .rect_to_screen(
            crate::geometry::WorldRect::from_points(
                CanvasPoint::new(region.min.x, region.min.y),
                CanvasPoint::new(region.max.x, region.max.y),
            )
            .rect(),
        )
        .shrink(TEXT_INSET * camera.scale);
    let colors = TextAreaColors {
        syntax: SyntaxColors::uniform(ink.into()),
        ..text_area_colors(&ThemeTokens::literal(&Theme::DARK))
    };
    let square = line_height(px);
    let Some(lines) = rich_state(entity).lay_out_prose(
        px,
        prose,
        "",
        inner.width(),
        &colors,
        text_area_geometry(),
        square,
        |pieces, options| Some(core_painter(painter).rich_layout(pieces, options)),
    ) else {
        return Vec::new();
    };
    let height: f32 = lines.iter().map(|line| line.layout.size.y).sum();
    let mut top = match prose.vertical_align {
        TextAlign::Start => inner.top(),
        TextAlign::Center => inner.center().y - height / 2.0,
        TextAlign::End => inner.bottom() - height,
    };
    lines
        .into_iter()
        .map(|ProseLine { mut layout, quote }| {
            let indent = quote as f32 * square;
            layout.align(inner.width() - indent, prose.align, |x| x);
            let origin = Vec2::new(inner.left() + indent, top);
            let mut missing: Vec<Rect> = layout
                .inline_rects()
                .into_iter()
                .map(|(_, rect)| rect.translate(origin))
                .collect();
            if quote > 0 {
                missing.push(Rect::from_min_size(
                    pos2(inner.left(), top),
                    Vec2::new(indent, layout.size.y),
                ));
            }
            top += layout.size.y;
            Written {
                origin,
                layout,
                missing,
            }
        })
        .collect()
}

pub(crate) fn svg(scene: &Scene, options: &ImageExport, images: &Images) -> String {
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
    let context = be_sdk_beui::context();
    context.run(|context| {
        let painter = wrap_painter(&context.painter());
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
    let world = CanvasView::from_corner(Pos2::ZERO, 1.0);
    let turn = turn(entity);
    if let Some(raster) = scene.rasters.get(&entity.id) {
        let rect = world.rect_to_screen(entity_bounds(entity).rect());
        out.push_str(&format!(
            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" image-rendering=\"pixelated\" style=\"image-rendering:pixelated\"{} href=\"data:image/png;base64,{}\"/>\n",
            number(rect.min.x),
            number(rect.min.y),
            number(rect.width()),
            number(rect.height()),
            opacity_of(opacity),
            encode_raster(raster).unwrap_or_default()
        ));
        return;
    }
    if entity.shape == CanvasShape::Text {
        svg_written(out, painter, entity, ink, &turn);
        return;
    }
    if entity.shape.embeds_block() {
        let rect = world.rect_to_screen(
            crate::geometry::WorldRect::from_points(
                entity_corner(entity, -0.5),
                entity_corner(entity, 0.5),
            )
            .rect(),
        );
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
                BASE64.encode(&image.data)
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
                let title = painter.lay_out_text(
                    &title,
                    FontId::proportional(18.0),
                    TextLayout::wrapped(rect.width()),
                );
                let size = title.size();
                let at = pos2(
                    rect.center().x - size.x / 2.0,
                    rect.center().y - size.y / 2.0,
                );
                svg_text(out, at, &title, ink, &turn);
            }
        }
        return;
    }
    let fill = with_opacity(resolve_color(style.fill_color, scene.palette.auto), opacity);
    for part in &Sketch::of(entity, false).parts {
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
            Mark::Painted(painted, offset) => out.push_str(&format!(
                "<path d=\"{}\"{}/>\n",
                path_data(&painted.path().translated(*offset)),
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
    svg_written(
        out,
        painter,
        entity,
        ink,
        &turn_by(entity, text_turn(entity)),
    );
}

const MISSING: Color32 = Color32::from_rgb(255, 105, 180);

fn svg_written(
    out: &mut String,
    painter: &Painter,
    entity: &CanvasEntity,
    ink: Color32,
    turn: &str,
) {
    let lines = written(
        painter,
        entity,
        ink,
        CanvasView::from_corner(Pos2::ZERO, 1.0),
    );
    if lines.is_empty() {
        return;
    }
    for rect in lines.iter().flat_map(|line| &line.missing) {
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{turn}/>\n",
            number(rect.min.x),
            number(rect.min.y),
            number(rect.width()),
            number(rect.height()),
            paint("fill", MISSING)
        ));
    }
    let body = FontId::proportional(font_px(entity, 1.0))
        .bold(text_style(entity).weight == CanvasTextWeight::Bold);
    out.push_str(&format!(
        "<text font-family=\"sans-serif\" font-size=\"{}\"{}{}{turn}>",
        number(body.size),
        weight_of(body.bold),
        paint("fill", ink)
    ));
    for Written { origin, layout, .. } in &lines {
        for line in &layout.lines {
            for run in &line.runs {
                let Some(galley) = run.galley.as_ref().filter(|_| run.kind == SpanKind::Text)
                else {
                    continue;
                };
                let shown = galley.text().trim_end_matches(['\n', '\r']);
                if shown.trim().is_empty() {
                    continue;
                }
                out.push_str(&format!(
                    "<tspan x=\"{}\" y=\"{}\"{}>{}</tspan>",
                    number(origin.x + line.run_left(run)),
                    number(origin.y + line.top + line.baseline),
                    span_attributes(run.style, body, ink),
                    escape(shown)
                ));
            }
        }
    }
    out.push_str("</text>\n");
}

fn span_attributes(style: SpanStyle, body: FontId, ink: Color32) -> String {
    let font = style.font;
    let mut attributes = String::new();
    if font.family == FontFamily::Monospace {
        attributes.push_str(" font-family=\"monospace\"");
    }
    if font.size != body.size {
        attributes.push_str(&format!(" font-size=\"{}\"", number(font.size)));
    }
    if font.bold != body.bold {
        attributes.push_str(match font.bold {
            true => " font-weight=\"bold\"",
            false => " font-weight=\"normal\"",
        });
    }
    if font.italic {
        attributes.push_str(" font-style=\"italic\"");
    }
    let decorations: Vec<&str> = [
        (style.underline, "underline"),
        (style.strikethrough, "line-through"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if !decorations.is_empty() {
        attributes.push_str(&format!(" text-decoration=\"{}\"", decorations.join(" ")));
    }
    if style.color != ink.into() {
        attributes.push_str(&paint("fill", style.color.resolve(|_| None)));
    }
    attributes
}

fn weight_of(bold: bool) -> &'static str {
    match bold {
        true => " font-weight=\"bold\"",
        false => "",
    }
}

fn turn(entity: &CanvasEntity) -> String {
    turn_by(entity, entity.transform.rotation)
}

fn turn_by(entity: &CanvasEntity, rotation: f32) -> String {
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

fn svg_text(out: &mut String, at: Pos2, block: &TextBlock, color: Color32, turn: &str) {
    let text = block.text();
    let font = block.font();
    out.push_str(&format!(
        "<text font-family=\"sans-serif\" font-size=\"{}\"{}{}{turn}>",
        number(font.size),
        weight_of(font.bold),
        paint("fill", color)
    ));
    for line in block.lines() {
        let shown = text
            .get(line.bytes.clone())
            .unwrap_or_default()
            .trim_end_matches(['\n', '\r']);
        if shown.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
            number(at.x + line.rect.min.x),
            number(at.y + line.baseline),
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

pub(crate) fn encode_raster(raster: &CanvasRaster) -> Option<String> {
    encode_png(
        [u32::from(raster.width()), u32::from(raster.height())],
        raster.rgba_bytes().to_vec(),
    )
    .ok()
    .map(|png| BASE64.encode(png))
}

pub(crate) const DIALOG_SPACING: Space = Space::Lg;

/// The settings of the canvas's PNG (`png`) or SVG export.
#[component]
fn ImageSettings(settings: ExportSettings<ImageExport>, png: bool) -> NodeId {
    let value = settings.value();
    let scale = create_memo(move || Some(value.get().scale));
    let choose_scale = move |chosen: Option<f32>| {
        if let Some(scale) = chosen {
            settings.update(|settings| settings.scale = scale);
        }
    };
    let background = create_memo(move || value.get().background);
    let set_background = move |on: bool| settings.update(|settings| settings.background = on);
    view! {
        <List spacing=DIALOG_SPACING>
            <Show value={move || png}>
                <List direction=Direction::Horizontal align=Align::Center spacing=DIALOG_SPACING>
                    <ListItem grow=1.0>
                        <Text>"Scale"</Text>
                    </ListItem>
                    <Select
                        options={view! {
                            <ChoiceOption value=1.0f32 label="1x" />
                            <ChoiceOption value=2.0f32 label="2x" />
                            <ChoiceOption value=3.0f32 label="3x" />
                        }}
                        selected={Binding::new(scale, choose_scale)}
                        label="Scale"
                        @test_id={"infinite-canvas.export.scale"}
                    />
                </List>
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=DIALOG_SPACING>
                <ListItem grow=1.0>
                    <Text>"Background"</Text>
                </ListItem>
                <Switch
                    on={Binding::new(background, set_background)}
                    label="Background"
                    @test_id={"infinite-canvas.export.background"}
                />
            </List>
        </List>
    }
}
