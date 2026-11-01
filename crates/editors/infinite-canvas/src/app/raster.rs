use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::icons::{
    Glyph, ICON_ARROW_DOWN, ICON_ARROW_DOWN_LEFT, ICON_ARROW_DOWN_RIGHT, ICON_ARROW_LEFT,
    ICON_ARROW_RIGHT, ICON_ARROW_UP, ICON_ARROW_UP_LEFT, ICON_ARROW_UP_RIGHT, ICON_CIRCLE,
    ICON_PLUS, ICON_SCALING, ICON_SQUARE, ICON_TRASH,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, Image, NodeId, Pos2, Rect, pos2, reactive::Painter};
use infinite_canvas_types::{
    Canvas, CanvasEntity, CanvasPixel, CanvasPixelUpdate, CanvasPoint, CanvasProfile, CanvasRaster,
    CanvasRasterAnchor, CanvasShape, CanvasTransform, DEFAULT_RASTER_SIZE, InfiniteCanvasOperation,
    MAX_RASTER_PALETTE_COLORS, MAX_RASTER_SIZE, default_raster_palette,
};
use uuid::Uuid;

use crate::geometry::entity_bounds;
use crate::raster::{
    ActiveDrawing, Brush, BrushShape, MAX_BRUSH_SIZE, PixelTool, format_hex_color, parse_hex_color,
    rasterize_drawing, stamp,
};

use super::actions::CanvasActions;
use super::export::DIALOG_SPACING;
use super::layers::LayersSection;
use super::sidebar::{OpacityRow, ShortcutsSection};
use super::state::{CanvasState, Gesture, Tool};
use beui::{Align, Direction, ItemSize};

const SPACING: Space = Space::Md;
const GRID_MIN_CELL: f32 = 12.0;
const GRID_OPACITY: f32 = 0.5;

const ANCHORS: [(CanvasRasterAnchor, Glyph, &str); 9] = [
    (CanvasRasterAnchor::TopLeft, ICON_ARROW_UP_LEFT, "Top left"),
    (CanvasRasterAnchor::Top, ICON_ARROW_UP, "Top"),
    (
        CanvasRasterAnchor::TopRight,
        ICON_ARROW_UP_RIGHT,
        "Top right",
    ),
    (CanvasRasterAnchor::Left, ICON_ARROW_LEFT, "Left"),
    (CanvasRasterAnchor::Center, ICON_CIRCLE, "Centre"),
    (CanvasRasterAnchor::Right, ICON_ARROW_RIGHT, "Right"),
    (
        CanvasRasterAnchor::BottomLeft,
        ICON_ARROW_DOWN_LEFT,
        "Bottom left",
    ),
    (CanvasRasterAnchor::Bottom, ICON_ARROW_DOWN, "Bottom"),
    (
        CanvasRasterAnchor::BottomRight,
        ICON_ARROW_DOWN_RIGHT,
        "Bottom right",
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RasterDialog {
    #[default]
    None,
    Resize(Uuid),
    Clear(Uuid),
}

#[derive(Clone, Copy)]
pub(crate) struct RasterTools {
    pub(crate) color: Signal<CanvasPixel>,
    pub(crate) color_text: Signal<String>,
    pub(crate) brush_size: Signal<u16>,
    pub(crate) brush_shape: Signal<BrushShape>,
    pub(crate) filled: Signal<bool>,
    pub(crate) mirror_horizontal: Signal<bool>,
    pub(crate) mirror_vertical: Signal<bool>,
    pub(crate) grid: Signal<bool>,
    pub(crate) dialog: Signal<RasterDialog>,
    pub(crate) resize_width: Signal<u16>,
    pub(crate) resize_height: Signal<u16>,
    pub(crate) resize_anchor: Signal<CanvasRasterAnchor>,
    pub(crate) resize_button: NodeRef,
}

impl RasterTools {
    pub(crate) fn new() -> Self {
        let black = CanvasPixel::new(0, 0, 0, 255);
        Self {
            color: create_signal(black),
            color_text: create_signal(format_hex_color(black)),
            brush_size: create_signal(1),
            brush_shape: create_signal(BrushShape::Square),
            filled: create_signal(false),
            mirror_horizontal: create_signal(false),
            mirror_vertical: create_signal(false),
            grid: create_signal(true),
            dialog: create_signal(RasterDialog::None),
            resize_width: create_signal(DEFAULT_RASTER_SIZE),
            resize_height: create_signal(DEFAULT_RASTER_SIZE),
            resize_anchor: create_signal(CanvasRasterAnchor::Center),
            resize_button: NodeRef::new(),
        }
    }

    pub(crate) fn set_color(&self, color: CanvasPixel) {
        self.color.set(color);
        self.color_text.set(format_hex_color(color));
    }

    fn brush(&self, constrained: bool) -> Brush {
        Brush {
            size: self.brush_size.get_untracked(),
            shape: self.brush_shape.get_untracked(),
            filled: self.filled.get_untracked(),
            mirror_horizontal: self.mirror_horizontal.get_untracked(),
            mirror_vertical: self.mirror_vertical.get_untracked(),
            constrained,
        }
    }

    pub(crate) fn resize_brush(&self, by: i32) {
        let size = i32::from(self.brush_size.get_untracked()) + by;
        self.brush_size
            .set(size.clamp(1, i32::from(MAX_BRUSH_SIZE)) as u16);
    }
}

pub(crate) struct Stroke {
    layer: Uuid,
    created: Option<CanvasEntity>,
    transform: CanvasTransform,
    width: u16,
    height: u16,
    held: Option<CanvasRaster>,
    drawing: ActiveDrawing,
    brush: Brush,
    color: CanvasPixel,
    stamped: usize,
    written: HashMap<(u16, u16), CanvasPixel>,
}

impl Stroke {
    fn original(&self, pixel: (u16, u16)) -> CanvasPixel {
        self.held
            .as_ref()
            .and_then(|held| held.pixel(pixel.0, pixel.1))
            .unwrap_or(CanvasPixel::TRANSPARENT)
    }

    fn paint(&mut self, pixel: (u16, u16), updates: &mut Vec<CanvasPixelUpdate>) {
        if self.written.contains_key(&pixel) {
            return;
        }
        let original = self.original(pixel);
        if original != self.color {
            self.written.insert(pixel, original);
            updates.push(CanvasPixelUpdate {
                x: pixel.0,
                y: pixel.1,
                color: self.color,
            });
        }
    }

    fn pixels(&mut self) -> Vec<CanvasPixelUpdate> {
        let mut updates = Vec::new();
        if self.drawing.traces() {
            let from = self.stamped.min(self.drawing.path.len());
            let fresh = stamp(
                &self.drawing.path[from..],
                self.width,
                self.height,
                self.brush,
                false,
            );
            self.stamped = self.drawing.path.len();
            for pixel in fresh {
                self.paint(pixel, &mut updates);
            }
            return updates;
        }
        let wanted: HashSet<(u16, u16)> =
            rasterize_drawing(&self.drawing, self.width, self.height, self.brush)
                .into_iter()
                .collect();
        self.written.retain(|pixel, original| {
            let kept = wanted.contains(pixel);
            if !kept {
                updates.push(CanvasPixelUpdate {
                    x: pixel.0,
                    y: pixel.1,
                    color: *original,
                });
            }
            kept
        });
        let mut wanted: Vec<(u16, u16)> = wanted.into_iter().collect();
        wanted.sort_unstable();
        for pixel in wanted {
            self.paint(pixel, &mut updates);
        }
        updates
    }
}

impl CanvasState {
    pub(crate) fn palette(&self) -> Memo<Vec<CanvasPixel>> {
        let held = self.content().field(ObjectId::ROOT, Canvas::PALETTE);
        create_memo(move || held.get().unwrap_or_else(default_raster_palette))
    }

    pub(crate) fn raster_of(&self, id: Uuid) -> Option<CanvasRaster> {
        self.content()
            .read(|canvas| Canvas::raster(canvas, id))
            .ok()
            .flatten()
    }

    pub(crate) fn raster_at(&self, world: CanvasPoint, editable: bool) -> Option<CanvasEntity> {
        self.entities
            .all_untracked()
            .into_iter()
            .rev()
            .filter(|entity| entity.shape == CanvasShape::Raster && !entity.hidden)
            .find(|entity| entity_bounds(entity).contains(world))
            .filter(|entity| !editable || !entity.locked)
    }

    fn pixel_of(
        entity: &CanvasEntity,
        raster: &CanvasRaster,
        world: CanvasPoint,
    ) -> Option<(u16, u16)> {
        let (x, y) = entity
            .transform
            .pixel_at(raster.width(), raster.height(), world);
        (x >= 0 && y >= 0 && x < i32::from(raster.width()) && y < i32::from(raster.height()))
            .then_some((x as u16, y as u16))
    }

    pub(crate) fn press_raster(&self, tool: PixelTool, world: CanvasPoint, constrained: bool) {
        match tool {
            PixelTool::Fill | PixelTool::ReplaceColor => {
                self.begin_gesture(None);
                let Some(layer) = self.raster_at(world, true) else {
                    return;
                };
                let Some(raster) = self.raster_of(layer.id) else {
                    return;
                };
                let Some((x, y)) = Self::pixel_of(&layer, &raster, world) else {
                    return;
                };
                let color = self.raster.color.get_untracked();
                match tool {
                    PixelTool::Fill => self.record(InfiniteCanvasOperation::Fill {
                        id: layer.id,
                        x,
                        y,
                        color,
                    }),
                    _ => {
                        if let Some(from) = raster.pixel(x, y) {
                            self.record(InfiniteCanvasOperation::ReplaceColor {
                                id: layer.id,
                                from,
                                to: color,
                            });
                        }
                    }
                }
            }
            PixelTool::Eyedropper => {
                self.begin_gesture(None);
                self.pick_color_at(world);
            }
            PixelTool::Pencil
            | PixelTool::Eraser
            | PixelTool::Line
            | PixelTool::Rectangle
            | PixelTool::Ellipse => self.begin_stroke(tool, world, constrained),
        }
    }

    pub(crate) fn pick_color_at(&self, world: CanvasPoint) -> bool {
        let Some(layer) = self.raster_at(world, false) else {
            return false;
        };
        let Some(raster) = self.raster_of(layer.id) else {
            return false;
        };
        let Some(color) =
            Self::pixel_of(&layer, &raster, world).and_then(|(x, y)| raster.pixel(x, y))
        else {
            return false;
        };
        self.raster.set_color(color);
        if self.tool.get_untracked() == Tool::Pixel(PixelTool::Eyedropper) {
            self.set_tool(Tool::Pixel(self.last_drawing_tool()));
        }
        true
    }

    fn begin_stroke(&self, tool: PixelTool, world: CanvasPoint, constrained: bool) {
        let (layer, created, held) = match self.raster_at(world, false) {
            Some(layer) if layer.locked => {
                self.begin_gesture(None);
                return;
            }
            None if tool == PixelTool::Eraser => {
                self.begin_gesture(None);
                return;
            }
            Some(layer) => {
                let held = self.raster_of(layer.id);
                (layer, None, held)
            }
            None => {
                let entity = CanvasEntity::raster(world, DEFAULT_RASTER_SIZE, DEFAULT_RASTER_SIZE);
                (entity.clone(), Some(entity), None)
            }
        };
        let (width, height) = match &held {
            Some(held) => (held.width(), held.height()),
            None => (DEFAULT_RASTER_SIZE, DEFAULT_RASTER_SIZE),
        };
        let pixel = layer.transform.pixel_at(width, height, world);
        let color = match tool {
            PixelTool::Eraser => CanvasPixel::TRANSPARENT,
            _ => self.raster.color.get_untracked(),
        };
        let stroke = Stroke {
            layer: layer.id,
            created,
            transform: layer.transform,
            width,
            height,
            held,
            drawing: ActiveDrawing::new(tool, pixel),
            brush: self.raster.brush(constrained),
            color,
            stamped: 0,
            written: HashMap::new(),
        };
        self.hold_stroke(Some(stroke));
        self.begin_gesture(Some(Gesture::Raster {
            layer: layer.id,
            at: pixel,
            constrained,
        }));
    }

    pub(crate) fn drag_raster(&self, world: CanvasPoint, constrained: bool) {
        let Some(Gesture::Raster {
            at: held,
            constrained: was,
            ..
        }) = self.gesture.get_untracked()
        else {
            return;
        };
        let Some(pixel) = self.with_stroke(|stroke| {
            let stroke = stroke.as_mut()?;
            let pixel = stroke
                .transform
                .pixel_at(stroke.width, stroke.height, world);
            stroke.drawing.extend(pixel);
            stroke.brush.constrained = constrained;
            Some(pixel)
        }) else {
            return;
        };
        if pixel == held && constrained == was {
            return;
        }
        self.update_gesture(|gesture| {
            if let Gesture::Raster {
                at,
                constrained: shown,
                ..
            } = gesture
            {
                *at = pixel;
                *shown = constrained;
            }
        });
    }

    pub(crate) fn write_stroke(&self) {
        let written = self.with_stroke(|stroke| {
            let stroke = stroke.as_mut()?;
            Some((stroke.layer, stroke.created.take(), stroke.pixels()))
        });
        let Some((layer, created, pixels)) = written else {
            return;
        };
        if let Some(entity) = created {
            self.open_writes();
            self.record(InfiniteCanvasOperation::Add {
                entity: Box::new(entity),
                pixels: None,
            });
        }
        if !pixels.is_empty() {
            self.open_writes();
            self.record(InfiniteCanvasOperation::Paint { id: layer, pixels });
        }
    }

    pub(crate) fn selected_raster(&self) -> Option<CanvasEntity> {
        match self.selected_entities().as_slice() {
            [entity] if entity.shape == CanvasShape::Raster => Some(entity.clone()),
            _ => None,
        }
    }

    pub(crate) fn ask_to_resize_raster(&self) {
        let Some(layer) = untrack(|| self.selected_raster()) else {
            return;
        };
        let Some(raster) = self.raster_of(layer.id) else {
            return;
        };
        let tools = self.raster;
        tools.resize_width.set(raster.width());
        tools.resize_height.set(raster.height());
        tools.resize_anchor.set(CanvasRasterAnchor::Center);
        tools.dialog.set(RasterDialog::Resize(layer.id));
    }

    pub(crate) fn ask_to_clear_raster(&self) {
        if let Some(layer) = untrack(|| self.selected_raster()) {
            self.raster.dialog.set(RasterDialog::Clear(layer.id));
        }
    }

    pub(crate) fn add_to_palette(&self, color: CanvasPixel) {
        let mut colors = untrack(|| self.palette().get());
        if colors.contains(&color) || colors.len() >= MAX_RASTER_PALETTE_COLORS {
            return;
        }
        colors.push(color);
        self.record(InfiniteCanvasOperation::SetPalette { colors });
    }

    pub(crate) fn remove_from_palette(&self, color: CanvasPixel) {
        let colors: Vec<CanvasPixel> = untrack(|| self.palette().get())
            .into_iter()
            .filter(|held| *held != color)
            .collect();
        self.record(InfiniteCanvasOperation::SetPalette { colors });
    }
}

pub(crate) fn raster_image(raster: &CanvasRaster) -> Image {
    Image::from_rgba(
        u32::from(raster.width()),
        u32::from(raster.height()),
        raster.rgba_bytes().to_vec(),
    )
}

pub(crate) fn pixel_color(color: CanvasPixel) -> Color32 {
    Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha)
}

#[derive(Clone, PartialEq)]
pub(crate) struct RasterGuide {
    pub(crate) layer: Rect,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) scale: f32,
    pub(crate) grid: bool,
    pub(crate) color: Color32,
}

impl RasterGuide {
    pub(crate) fn draw(&self, painter: &Painter) {
        let hairline = 1.0 / self.scale.max(f32::EPSILON);
        let rect = self.layer;
        painter.rect_stroke(rect, 0.0, hairline, self.color);
        let column_width = rect.width() / f32::from(self.width.max(1));
        let row_height = rect.height() / f32::from(self.height.max(1));
        if !self.grid || column_width.min(row_height) * self.scale < GRID_MIN_CELL {
            return;
        }
        let faint = super::paint::with_opacity(self.color, GRID_OPACITY);
        for column in 1..self.width {
            let x = rect.left() + f32::from(column) * column_width;
            painter.line(pos2(x, rect.top()), pos2(x, rect.bottom()), hairline, faint);
        }
        for row in 1..self.height {
            let y = rect.top() + f32::from(row) * row_height;
            painter.line(pos2(rect.left(), y), pos2(rect.right(), y), hairline, faint);
        }
    }
}

#[component]
pub(crate) fn RasterGuideDrawing(
    state: CanvasState,
    entity: Memo<Option<CanvasEntity>>,
    raster: Memo<Option<CanvasRaster>>,
    corner: Memo<Pos2>,
) -> NodeId {
    let theme = use_theme();
    let scale = state.editor().scale();
    let grid = state.raster.grid;
    let guide = create_memo(move || {
        let entity = entity.get()?;
        let (width, height) = raster.with(|raster| {
            raster
                .as_ref()
                .map(|raster| (raster.width(), raster.height()))
        })?;
        let corner = corner.get();
        let bounds = entity_bounds(&entity);
        Some(RasterGuide {
            layer: Rect::from_min_max(
                pos2(bounds.min.x - corner.x, bounds.min.y - corner.y),
                pos2(bounds.max.x - corner.x, bounds.max.y - corner.y),
            ),
            width,
            height,
            scale: scale.get(),
            grid: grid.get(),
            color: beui::reactive::resolve_color(theme.border.get()),
        })
    });
    let draw = Prop::dynamic(move || -> Draw {
        let guide = guide.get();
        Rc::new(move |painter: &Painter, _rect: Rect| {
            if let Some(guide) = guide.as_ref() {
                guide.draw(painter);
            }
        })
    });
    view! {
        <Drawing draw={draw} />
    }
}

pub(crate) fn rastering(state: CanvasState) -> Memo<bool> {
    create_memo(move || state.profile.get() == CanvasProfile::Raster)
}

#[component]
pub(crate) fn RasterPanel(state: CanvasState) -> NodeId {
    let layer = create_memo(move || state.selected_raster().is_some());
    let narrow = block_editor_beui::narrow_chrome();
    let roomy = create_memo(move || !narrow.get());
    view! {
        <List spacing=SPACING @test_id={"infinite-canvas.raster"}>
            <BrushSection state={state} />
            <ColorSection state={state} />
            <Show condition={layer}>
                <LayerSection state={state} />
            </Show>
            <PixelSection />
            <LayersSection state={state} />
            <Show condition={roomy}>
                <List spacing=SPACING>
                    <Separator />
                    <ShortcutsSection raster=true />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn BrushSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let tools = state.raster;
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let size = tools.brush_size;
    let size_value = create_memo(move || f64::from(size.get()));
    let shape = tools.brush_shape;
    let square = create_memo(move || shape.get() == BrushShape::Square);
    let circle = create_memo(move || shape.get() == BrushShape::Circle);
    view! {
        <Accordion title="Brush" open={open}>
            <List spacing=Space::Sm>
                <NumberInput
                    value={Binding::new(size_value, move |value: f64| {
                        size.set(value.round().clamp(1.0, f64::from(MAX_BRUSH_SIZE)) as u16)
                    })}
                    min=1.0
                    max={f64::from(MAX_BRUSH_SIZE)}
                    decimals={Some(0)}
                    label="Brush size"
                    @test_id={"infinite-canvas.raster.brush-size"}
                />
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <ToggleButton
                        glyph=ICON_SQUARE
                        label="Square brush"
                        icon_only=true
                        pressed={Binding::new(square, move |_| shape.set(BrushShape::Square))}
                        @test_id={"infinite-canvas.raster.square"}
                    />
                    <ToggleButton
                        glyph=ICON_CIRCLE
                        label="Round brush"
                        icon_only=true
                        pressed={Binding::new(circle, move |_| shape.set(BrushShape::Circle))}
                        @test_id={"infinite-canvas.raster.round"}
                    />
                    <ToggleButton
                        icon_only=true
                        action={actions.raster.filled.clone()}
                        @test_id={"infinite-canvas.raster.filled"}
                    />
                    <ToggleButton
                        icon_only=true
                        action={actions.raster.mirror_horizontal.clone()}
                        @test_id={"infinite-canvas.raster.mirror-horizontal"}
                    />
                    <ToggleButton
                        icon_only=true
                        action={actions.raster.mirror_vertical.clone()}
                        @test_id={"infinite-canvas.raster.mirror-vertical"}
                    />
                </List>
            </List>
        </Accordion>
    }
}

#[component]
fn ColorSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let tools = state.raster;
    let color = tools.color;
    let text = tools.color_text;
    let palette = state.palette();
    let typed = move |typed: String| match parse_hex_color(&typed) {
        Some(parsed) => {
            color.set(parsed);
            text.set(typed);
        }
        None => text.set(typed),
    };
    let current = create_memo(move || pixel_color(color.get()));
    let current_label = create_memo(move || format_hex_color(color.get()));
    let cannot_add = create_memo(move || {
        palette.with(|palette| {
            palette.len() >= MAX_RASTER_PALETTE_COLORS || palette.contains(&color.get())
        })
    });
    let cannot_remove =
        create_memo(move || palette.with(|palette| !palette.contains(&color.get())));
    let swatches = create_memo(move || (0..palette.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <Accordion title="Colour" open={open}>
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                    <SwatchButton
                        color={current}
                        label={current_label}
                        radius=Radius::Xs
                        on_click={move || {}}
                    />
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={Binding::new(text, typed)}
                        label="Colour"
                        placeholder="#RRGGBBAA"
                        @test_id={"infinite-canvas.raster.hex"}
                    />
                </List>
                <List direction=Direction::Horizontal wrap=true spacing=Space::Xs>
                    <ForEach keys={swatches}>|index: usize| {
                        let swatch = create_memo(move || {
                            palette.with(|palette| palette.get(index).copied().unwrap_or_default())
                        });
                        let fill = create_memo(move || pixel_color(swatch.get()));
                        let label = create_memo(move || format_hex_color(swatch.get()));
                        let chosen = create_memo(move || swatch.get() == color.get());
                        view! {
                            <SwatchButton
                                color={fill}
                                label={label}
                                selected={chosen}
                                radius=Radius::Xs
                                @test_id={format!("infinite-canvas.raster.swatch.{index}")}
                                on_click={move || tools.set_color(swatch.get_untracked())}
                            />
                        }
                    }</ForEach>
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <Button
                        icon_only=true
                        glyph=ICON_PLUS
                        label="Add the colour to the palette"
                        disabled={cannot_add}
                        @test_id={"infinite-canvas.raster.palette-add"}
                        on_click={move || state.add_to_palette(color.get_untracked())}
                    />
                    <Button
                        icon_only=true
                        glyph=ICON_TRASH
                        label="Remove the colour from the palette"
                        disabled={cannot_remove}
                        @test_id={"infinite-canvas.raster.palette-remove"}
                        on_click={move || state.remove_from_palette(color.get_untracked())}
                    />
                </List>
            </List>
        </Accordion>
    }
}

#[component]
fn LayerSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let tools = state.raster;
    let layer = create_memo(move || state.selected_raster());
    let size = create_memo(move || {
        let layer = layer.get()?;
        state.entities.raster(layer.id).with(|raster| {
            raster
                .as_ref()
                .map(|raster| (raster.width(), raster.height()))
        })
    });
    let dimensions = create_memo(move || match size.get() {
        Some((width, height)) => format!("{width} × {height} pixels"),
        None => String::new(),
    });
    let locked = create_memo(move || layer.get().is_none_or(|layer| layer.locked));
    view! {
        <Accordion title="Raster layer" open={open}>
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::CAPTION>
                        {dimensions}
                    </Text>
                    <Button
                        icon_only=true
                        glyph=ICON_SCALING
                        label="Resize…"
                        disabled={locked}
                        @node_ref={&tools.resize_button}
                        @test_id={"infinite-canvas.raster.resize"}
                        on_click={move || state.ask_to_resize_raster()}
                    />
                    <Button
                        icon_only=true
                        glyph=ICON_TRASH
                        label="Clear…"
                        disabled={locked}
                        @test_id={"infinite-canvas.raster.clear"}
                        on_click={move || state.ask_to_clear_raster()}
                    />
                </List>
                <OpacityRow state={state} />
            </List>
        </Accordion>
    }
}

#[component]
fn PixelSection() -> NodeId {
    let open = create_signal(true);
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    view! {
        <Accordion title="Pixels" open={open}>
            <List spacing=Space::Sm>
                <ToggleButton
                    action={actions.raster.grid.clone()}
                    @test_id={"infinite-canvas.raster.grid"}
                />
                <ToggleButton
                    action={actions.snap.clone()}
                    @test_id={"infinite-canvas.raster.snap"}
                />
            </List>
        </Accordion>
    }
}

#[component]
pub(crate) fn RasterDialogs(state: CanvasState) -> NodeId {
    let tools = state.raster;
    let dialog = tools.dialog;
    let resizing = create_memo(move || matches!(dialog.get(), RasterDialog::Resize(_)));
    let clearing = create_memo(move || matches!(dialog.get(), RasterDialog::Clear(_)));
    let width = tools.resize_width;
    let height = tools.resize_height;
    let width_value = create_memo(move || f64::from(width.get()));
    let height_value = create_memo(move || f64::from(height.get()));
    let held = create_memo(move || match dialog.get() {
        RasterDialog::Resize(id) | RasterDialog::Clear(id) => {
            state.entities.raster(id).with(|raster| {
                raster
                    .as_ref()
                    .map(|raster| (raster.width(), raster.height()))
            })
        }
        RasterDialog::None => None,
    });
    let cropping = create_memo(move || {
        held.get().is_some_and(|(held_width, held_height)| {
            width.get() < held_width || height.get() < held_height
        })
    });
    let rows = (0..3).collect::<Vec<usize>>();
    let apply = move || {
        if let RasterDialog::Resize(id) = dialog.get_untracked() {
            state.record(InfiniteCanvasOperation::Resize {
                id,
                width: width.get_untracked(),
                height: height.get_untracked(),
                anchor: tools.resize_anchor.get_untracked(),
            });
        }
        dialog.set(RasterDialog::None);
    };
    let clear = move || {
        if let RasterDialog::Clear(id) = dialog.get_untracked() {
            state.record(InfiniteCanvasOperation::Clear { id });
        }
        dialog.set(RasterDialog::None);
    };
    let dismiss = move || dialog.set(RasterDialog::None);
    let dimension = |signal: Signal<u16>| {
        move |value: f64| signal.set(value.round().clamp(1.0, f64::from(MAX_RASTER_SIZE)) as u16)
    };
    view! {
        <List spacing=Space::None>
            <Dialog
                open={resizing}
                title="Resize raster layer"
                anchor={DialogAnchor::Control(tools.resize_button)}
                id="infinite-canvas.raster.resizing"
                actions={vec![
                    DialogAction::cancel("Cancel"),
                    DialogAction::primary("Resize", apply),
                ]}
                on_dismiss={dismiss}
            >
                <List spacing=DIALOG_SPACING>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=DIALOG_SPACING
                    >
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            value={Binding::new(width_value, dimension(width))}
                            min=1.0
                            max={f64::from(MAX_RASTER_SIZE)}
                            decimals={Some(0)}
                            label="Width"
                            @test_id={"infinite-canvas.raster.resize-width"}
                        />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            value={Binding::new(height_value, dimension(height))}
                            min=1.0
                            max={f64::from(MAX_RASTER_SIZE)}
                            decimals={Some(0)}
                            label="Height"
                            @test_id={"infinite-canvas.raster.resize-height"}
                        />
                    </List>
                    <Text style=TextStyle::CAPTION>"Anchor"</Text>
                    <ForEach keys={rows}>|row: usize|
                        <AnchorRow tools={tools} row={row} />
                    </ForEach>
                    <Show condition={cropping}>
                        <Text style=TextStyle::CAPTION color=TextColor::Warning>
                            "Shrinking crops the pixels outside the anchored region."
                        </Text>
                    </Show>
                </List>
            </Dialog>
            <Dialog
                open={clearing}
                kind=DialogKind::Alert
                title="Clear the raster layer?"
                id="infinite-canvas.raster.clearing"
                actions={vec![
                    DialogAction::destructive("Clear", clear),
                    DialogAction::cancel("Cancel"),
                ]}
                on_dismiss={dismiss}
            >
                <Text style=TextStyle::PARAGRAPH>
                    "This makes every pixel of the layer transparent."
                </Text>
            </Dialog>
        </List>
    }
}

#[component]
fn AnchorRow(tools: RasterTools, row: usize) -> NodeId {
    let entries = (0..3).map(|column| row * 3 + column).collect::<Vec<_>>();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
            <ForEach keys={entries}>|index: usize| {
                let (anchor, glyph, label) = ANCHORS[index];
                let chosen = tools.resize_anchor;
                let pressed = create_memo(move || chosen.get() == anchor);
                view! {
                    <ToggleButton
                        glyph={glyph}
                        label={label}
                        icon_only=true
                        pressed={Binding::new(pressed, move |_| chosen.set(anchor))}
                    />
                }
            }</ForEach>
        </List>
    }
}
