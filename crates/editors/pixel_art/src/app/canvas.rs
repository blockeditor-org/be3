use std::rc::Rc;

use block_editor_plugin::be_block::pixel_art::{PixelArtOperation, PixelColor, PixelUpdate};
use block_editor_plugin::beui::icons::ICON_ARROW_FORWARD;
use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, CanvasView, ClickCatcher, Focusable, ForEach, Frame, ItemSize, List, Memo,
    NodeRef, Picture, ReadSignal, WriteSignal, clone, component, component_rect, create_effect,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{Code, use_theme};
use block_editor_plugin::beui::{
    Color32, CursorIcon, ImageFit, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, Vec2,
};

use crate::canvas::ZOOM_STEP;
use crate::canvas::{canvas_rect, pixel_at};
use crate::color::format_hex_color;
use crate::drawing::{
    ActiveDrawing, MAX_BRUSH_SIZE, PixelTool, rasterize_drawing,
};

use super::pane::{Pane, Shown};
use super::state::Tools;

const GRID_MIN_CELL: f32 = 6.0;
const GRID_ALPHA: u8 = 60;
const BORDER_ALPHA: u8 = 160;
const HOVER_LABEL_PADDING: f32 = 6.0;
const HOVER_LABEL_HEIGHT: f32 = 24.0;

#[component]
pub(crate) fn ArtworkCanvas(
    tools: Rc<Tools>,
    pane: Rc<Pane>,
    shown: Memo<Shown>,
    size: Memo<(u16, u16)>,
    hovered: ReadSignal<Option<(u16, u16)>>,
    set_hovered: WriteSignal<Option<(u16, u16)>>,
) -> NodeId {
    let editor = tools.editor().clone();
    let view = editor.canvas();
    let scale = editor.scale();
    let placed = component_rect();
    let world = editor.world();
    let artwork = create_memo(clone!(size world placed -> move || {
        let (width, height) = size.get();
        if width == 0 || height == 0 {
            return Rect::ZERO;
        }
        let available = world.get().unwrap_or_else(|| placed.get().size());
        canvas_rect(
            Rect::from_min_size(Pos2::ZERO, available.max(Vec2::new(1.0, 1.0))),
            width,
            height,
        )
    }));

    let pressing = Rc::clone(&tools);
    let press_size = size.clone();
    let press_view = view.clone();
    let press_artwork = artwork.clone();
    let press_editor = editor.clone();
    let on_press = move |press: PointerPress| {
        let (width, height) = press_size.get_untracked();
        if width == 0 || pressing.busy() || !pressing.editable() {
            return;
        }
        pressing.set_constrained.set(press.modifiers.shift);
        let Some(pixel) = pixel_of(
            press.pos,
            press_view.get_untracked(),
            press_editor.content_rect(),
            press_artwork.get_untracked(),
            width,
            height,
        ) else {
            return;
        };
        begin(&pressing, pixel);
    };

    let dragging = Rc::clone(&tools);
    let drag_size = size.clone();
    let drag_view = view.clone();
    let drag_artwork = artwork.clone();
    let drag_editor = editor.clone();
    let on_drag = move |press: PointerPress| {
        let (width, height) = drag_size.get_untracked();
        if width == 0 || dragging.busy() || !dragging.editable() {
            return;
        }
        dragging.set_constrained.set(press.modifiers.shift);
        let Some(pixel) = pixel_of(
            press.pos,
            drag_view.get_untracked(),
            drag_editor.content_rect(),
            drag_artwork.get_untracked(),
            width,
            height,
        ) else {
            return;
        };
        dragging.draw(|drawing| {
            if let Some(drawing) = drawing.as_mut() {
                drawing.extend(pixel);
            }
        });
    };

    let releasing = Rc::clone(&tools);
    let release_size = size.clone();
    let release_view = view.clone();
    let release_artwork = artwork.clone();
    let release_editor = editor.clone();
    let on_click_at = move |press: PointerPress| {
        let (width, height) = release_size.get_untracked();
        if width == 0 || releasing.busy() || !releasing.editable() {
            return;
        }
        releasing.set_constrained.set(press.modifiers.shift);
        if let Some(pixel) = pixel_of(
            press.pos,
            release_view.get_untracked(),
            release_editor.content_rect(),
            release_artwork.get_untracked(),
            width,
            height,
        ) {
            finish(&releasing, pixel, width, height);
        }
    };

    let sampling = Rc::clone(&tools);
    let sample_size = size.clone();
    let sample_view = view.clone();
    let sample_artwork = artwork.clone();
    let sample_editor = editor.clone();
    let on_secondary = move |press: PointerPress| {
        let (width, height) = sample_size.get_untracked();
        if width == 0 {
            return;
        }
        if let Some(pixel) = pixel_of(
            press.pos,
            sample_view.get_untracked(),
            sample_editor.content_rect(),
            sample_artwork.get_untracked(),
            width,
            height,
        ) {
            sample(&sampling, pixel);
        }
    };

    let moving = Rc::clone(&tools);
    let hover_size = size.clone();
    let hover_view = view.clone();
    let hover_artwork = artwork.clone();
    let hover_editor = editor.clone();
    let set_hover = set_hovered.clone();
    let on_hover_move = move |press: PointerPress| {
        let (width, height) = hover_size.get_untracked();
        if width == 0 {
            return;
        }
        moving.set_constrained.set(press.modifiers.shift);
        set_hover.set(pixel_of(
            press.pos,
            hover_view.get_untracked(),
            hover_editor.content_rect(),
            hover_artwork.get_untracked(),
            width,
            height,
        ));
    };
    let left = set_hovered.clone();
    let on_hover_change = move |hovered: bool| {
        if !hovered {
            left.set(None);
        }
    };

    let refreshing = Rc::clone(&tools);
    let refreshed = Rc::clone(&pane);
    let hover = hovered.clone();
    let frame_size = size.clone();
    let theme = use_theme();
    let background = theme.background.clone();
    create_effect(move || {
        let (width, height) = frame_size.get();
        if width == 0 {
            return;
        }
        let (pixels, color) = pending(&refreshing, hover.get(), width, height);
        let dark = is_dark(background.get());
        refreshed.refresh(refreshing.block(), dark, &pixels, color);
    });

    let keys = Rc::clone(&tools);
    let key_editor = editor.clone();
    let on_key = move |press: KeyPress| shortcut(&keys, &key_editor, press);

    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Frame @node_ref={&content} @test_id={"pixel-art.canvas"}>
            <Focusable on_key={on_key}>
                <ClickCatcher
                    cursor=CursorIcon::Crosshair
                    repeat_drag=true
                    on_press={on_press}
                    on_drag={on_drag}
                    on_click_at={on_click_at}
                    on_secondary_press={on_secondary}
                    on_hover_move={on_hover_move}
                    on_hover_change={on_hover_change}
                >
                    <List spacing=0.0>
                        <Artwork
                            @sizing=ItemSize::Percent(100.0)
                            shown={shown}
                            view={view}
                            scale={scale}
                            artwork={artwork}
                            show_grid={tools.show_grid.clone()}
                            hovered={hovered}
                        />
                    </List>
                </ClickCatcher>
            </Focusable>
        </Frame>
    }
}

fn shortcut(tools: &Rc<Tools>, editor: &block_editor_plugin::Editor, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt || !press.pressed {
        return !matches!(shortcut_kind(press.key), Shortcut::None);
    }
    match shortcut_kind(press.key) {
        Shortcut::Tool(tool) => tools.select_tool(tool),
        Shortcut::Cancel => {
            tools.draw(|drawing| drawing.take());
        }
        Shortcut::Larger => {
            let size = tools.brush_size.get_untracked();
            tools.set_brush_size.set((size + 1).min(MAX_BRUSH_SIZE));
        }
        Shortcut::Smaller => {
            let size = tools.brush_size.get_untracked();
            tools.set_brush_size.set(size.saturating_sub(1).max(1));
        }
        Shortcut::Fill => {
            if matches!(
                tools.tool.get_untracked(),
                PixelTool::Rectangle | PixelTool::Ellipse
            ) {
                let filled = tools.shapes_filled.get_untracked();
                tools.set_shapes_filled.set(!filled);
            }
        }
        Shortcut::MirrorHorizontal => {
            let on = tools.mirror_horizontal.get_untracked();
            tools.set_mirror_horizontal.set(!on);
        }
        Shortcut::MirrorVertical => {
            let on = tools.mirror_vertical.get_untracked();
            tools.set_mirror_vertical.set(!on);
        }
        Shortcut::ZoomIn => editor.zoom(ZOOM_STEP),
        Shortcut::ZoomOut => editor.zoom(1.0 / ZOOM_STEP),
        Shortcut::Fit => editor.fit(),
        Shortcut::None => return false,
    }
    true
}

enum Shortcut {
    Tool(PixelTool),
    Cancel,
    Larger,
    Smaller,
    Fill,
    MirrorHorizontal,
    MirrorVertical,
    ZoomIn,
    ZoomOut,
    Fit,
    None,
}

fn shortcut_kind(key: Key) -> Shortcut {
    match key {
        Key::B | Key::P => Shortcut::Tool(PixelTool::Pencil),
        Key::E => Shortcut::Tool(PixelTool::Eraser),
        Key::G => Shortcut::Tool(PixelTool::Fill),
        Key::I => Shortcut::Tool(PixelTool::Eyedropper),
        Key::C => Shortcut::Tool(PixelTool::ReplaceColor),
        Key::L => Shortcut::Tool(PixelTool::Line),
        Key::R => Shortcut::Tool(PixelTool::Rectangle),
        Key::O => Shortcut::Tool(PixelTool::Ellipse),
        Key::Escape => Shortcut::Cancel,
        Key::BracketRight => Shortcut::Larger,
        Key::BracketLeft => Shortcut::Smaller,
        Key::X => Shortcut::Fill,
        Key::H => Shortcut::MirrorHorizontal,
        Key::V => Shortcut::MirrorVertical,
        Key::Plus => Shortcut::ZoomIn,
        Key::Minus => Shortcut::ZoomOut,
        Key::Zero => Shortcut::Fit,
        _ => Shortcut::None,
    }
}

#[component]
fn Artwork(
    shown: Memo<Shown>,
    view: ReadSignal<Option<CanvasView>>,
    scale: ReadSignal<f32>,
    artwork: Memo<Rect>,
    show_grid: ReadSignal<bool>,
    hovered: ReadSignal<Option<(u16, u16)>>,
) -> NodeId {
    let cell = create_memo(clone!(shown artwork -> move || {
        let width = shown.get().width;
        match width {
            0 => 0.0,
            width => artwork.get().width() / f32::from(width),
        }
    }));
    let left = create_memo(clone!(artwork -> move || artwork.get().left()));
    let top = create_memo(clone!(artwork -> move || artwork.get().top()));
    let width = create_memo(clone!(artwork -> move || artwork.get().width()));
    let height = create_memo(clone!(artwork -> move || artwork.get().height()));
    let image = create_memo(clone!(shown -> move || shown.get().artwork));
    let preview = create_memo(clone!(shown -> move || shown.get().preview));
    let has_preview = create_memo(clone!(preview -> move || preview.get().is_some()));
    let preview_image = create_memo(clone!(preview -> move || {
        preview.get().map(|(_, _, _, _, image)| image)
    }));
    let preview_x = create_memo(clone!(preview left cell -> move || {
        left.get() + preview.get().map_or(0.0, |(edge, _, _, _, _)| f32::from(edge)) * cell.get()
    }));
    let preview_y = create_memo(clone!(preview top cell -> move || {
        top.get() + preview.get().map_or(0.0, |(_, edge, _, _, _)| f32::from(edge)) * cell.get()
    }));
    let preview_width = create_memo(clone!(preview cell -> move || {
        preview.get().map_or(0.0, |(edge, _, far, _, _)| f32::from(far - edge + 1)) * cell.get()
    }));
    let preview_height = create_memo(clone!(preview cell -> move || {
        preview.get().map_or(0.0, |(_, edge, _, far, _)| f32::from(far - edge + 1)) * cell.get()
    }));
    let thin = create_memo(clone!(scale -> move || 1.0 / scale.get().max(f32::EPSILON)));
    let border = thin.clone();
    let lines = create_memo(clone!(shown scale cell show_grid -> move || {
        let shown = shown.get();
        if !show_grid.get() || cell.get() * scale.get() < GRID_MIN_CELL {
            return Vec::new();
        }
        (1..shown.width)
            .map(Line::Vertical)
            .chain((1..shown.height).map(Line::Horizontal))
            .collect::<Vec<Line>>()
    }));
    let hover_x = create_memo(clone!(hovered left cell -> move || {
        left.get() + hovered.get().map_or(0.0, |(x, _)| f32::from(x)) * cell.get()
    }));
    let hover_y = create_memo(clone!(hovered top cell -> move || {
        top.get() + hovered.get().map_or(0.0, |(_, y)| f32::from(y)) * cell.get()
    }));
    let over = create_memo(clone!(hovered -> move || hovered.get().is_some()));
    let hover_size = create_memo(clone!(over cell -> move || match over.get() {
        true => cell.get(),
        false => 0.0,
    }));
    view! {
        <Canvas view={view}>
            <CanvasItem
                x={left.clone()}
                y={top.clone()}
                width={width.clone()}
                height={height.clone()}
                @test_id={"pixel-art.artwork"}
            >
                <Picture image={image} fit=ImageFit::Fill smooth=false />
            </CanvasItem>
            <CanvasItem
                x={left.clone()}
                y={top.clone()}
                width={width.clone()}
                height={height.clone()}
            >
                <Frame
                    outline={Color32::from_rgba_unmultiplied(0, 0, 0, BORDER_ALPHA)}
                    outline_width={border}
                    outline_visible=true
                />
            </CanvasItem>
            <CanvasItem x={preview_x} y={preview_y} width={preview_width} height={preview_height}>
                <Frame visible={has_preview}>
                    <Picture image={preview_image} fit=ImageFit::Fill smooth=false />
                </Frame>
            </CanvasItem>
            <ForEach keys={lines}>
                {move |line: Line| {
                    let thin = thin.clone();
                    let cell = cell.clone();
                    let (left, top) = (left.clone(), top.clone());
                    let (width, height) = (width.clone(), height.clone());
                    let (x, y, line_width, line_height) = match line {
                        Line::Vertical(at) => (
                            create_memo(clone!(left cell -> move || {
                                left.get() + f32::from(at) * cell.get()
                            })),
                            top,
                            thin,
                            height,
                        ),
                        Line::Horizontal(at) => (
                            left,
                            create_memo(clone!(top cell -> move || {
                                top.get() + f32::from(at) * cell.get()
                            })),
                            width,
                            thin,
                        ),
                    };
                    view! {
                        <CanvasItem x={x} y={y} width={line_width} height={line_height}>
                            <Frame color={Color32::from_rgba_unmultiplied(0, 0, 0, GRID_ALPHA)} />
                        </CanvasItem>
                    }
                }}
            </ForEach>
            <CanvasItem x={hover_x} y={hover_y} width={hover_size.clone()} height={hover_size}>
                <Frame
                    visible={over}
                    outline=Color32::WHITE
                    outline_width=1.0
                    outline_visible=true
                />
            </CanvasItem>
        </Canvas>
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Line {
    Vertical(u16),
    Horizontal(u16),
}

#[component]
pub(crate) fn HoverLabel(tools: Rc<Tools>, hovered: ReadSignal<Option<(u16, u16)>>) -> NodeId {
    let color = tools.color.clone();
    let tool = tools.tool.clone();
    let sampled = Rc::clone(&tools);
    let source = create_memo(clone!(hovered tool -> move || {
        if tool.get() != PixelTool::ReplaceColor {
            return None;
        }
        let (x, y) = hovered.get()?;
        sampled.pixel(x, y)
    }));
    let label = create_memo(clone!(hovered source color -> move || {
        let Some((x, y)) = hovered.get() else {
            return String::new();
        };
        match source.get() {
            Some(source) => format!(
                "{x}, {y} · {} {ICON_ARROW_FORWARD} {}",
                format_hex_color(source),
                format_hex_color(color.get())
            ),
            None => format!("{x}, {y}"),
        }
    }));
    view! {
        <Frame
            height=HOVER_LABEL_HEIGHT
            padding_horizontal=HOVER_LABEL_PADDING
            padding_vertical=HOVER_LABEL_PADDING
        >
            <Code @test_id={"pixel-art.hover"} content={label} />
        </Frame>
    }
}

fn pixel_of(
    at: Pos2,
    view: Option<CanvasView>,
    content: Rect,
    artwork: Rect,
    width: u16,
    height: u16,
) -> Option<(u16, u16)> {
    let world = match view {
        Some(view) => view.to_canvas(at),
        None => Pos2::new(at.x - content.left(), at.y - content.top()),
    };
    pixel_at(world, artwork, width, height)
}

fn is_dark(color: Color32) -> bool {
    let [red, green, blue, _] = color.to_array();
    u16::from(red) + u16::from(green) + u16::from(blue) < 384
}

fn begin(tools: &Rc<Tools>, pixel: (u16, u16)) {
    let tool = tools.tool.get_untracked();
    match tool {
        tool if tool.is_drawing() => {
            tools.draw(|drawing| *drawing = Some(ActiveDrawing::new(tool, pixel)));
        }
        _ => {}
    }
}

fn finish(tools: &Rc<Tools>, pixel: (u16, u16), width: u16, height: u16) {
    let tool = tools.tool.get_untracked();
    match tool {
        tool if tool.is_drawing() => commit(tools, width, height),
        PixelTool::Fill => {
            let color = tools.color.get_untracked();
            tools.remember_color(color);
            tools.operate(PixelArtOperation::Fill {
                x: pixel.0,
                y: pixel.1,
                color,
            });
        }
        PixelTool::Eyedropper => sample(tools, pixel),
        PixelTool::ReplaceColor => {
            let Some(from) = tools.pixel(pixel.0, pixel.1) else {
                return;
            };
            let to = tools.color.get_untracked();
            tools.remember_color(to);
            tools.operate(PixelArtOperation::ReplaceColor { from, to });
        }
        PixelTool::Pencil
        | PixelTool::Eraser
        | PixelTool::Line
        | PixelTool::Rectangle
        | PixelTool::Ellipse => {}
    }
}

fn commit(tools: &Rc<Tools>, width: u16, height: u16) {
    let Some(drawing) = tools.draw(|drawing| drawing.take()) else {
        return;
    };
    let pixels = rasterize_drawing(&drawing, width, height, tools.brush());
    if pixels.is_empty() {
        return;
    }
    let color = stroke_color(tools, drawing.tool);
    if drawing.tool != PixelTool::Eraser {
        tools.remember_color(tools.color.get_untracked());
    }
    tools.operate(PixelArtOperation::Paint {
        pixels: pixels
            .into_iter()
            .map(|(x, y)| PixelUpdate { x, y, color })
            .collect(),
    });
}

fn sample(tools: &Rc<Tools>, pixel: (u16, u16)) {
    let Some(color) = tools.pixel(pixel.0, pixel.1) else {
        return;
    };
    tools.set_active_color(color, true);
    if tools.tool.get_untracked() == PixelTool::Eyedropper {
        tools.select_tool(tools.previous_drawing_tool());
    }
}

fn stroke_color(tools: &Rc<Tools>, tool: PixelTool) -> PixelColor {
    match tool == PixelTool::Eraser {
        true => PixelColor::TRANSPARENT,
        false => tools.color.get_untracked(),
    }
}

fn pending(
    tools: &Rc<Tools>,
    hovered: Option<(u16, u16)>,
    width: u16,
    height: u16,
) -> (Vec<(u16, u16)>, PixelColor) {
    let color = tools.color.get();
    if !tools.editor().editable().get() || tools.busy() {
        return (Vec::new(), color);
    }
    let brush = tools.brush();
    let drawn = tools.drawing(|drawing| {
        drawing.map(|drawing| {
            (
                rasterize_drawing(drawing, width, height, brush),
                stroke_color(tools, drawing.tool),
            )
        })
    });
    if let Some(drawn) = drawn {
        return drawn;
    }
    let tool = tools.tool.get();
    match hovered.filter(|_| tool.is_drawing()) {
        Some(pixel) => {
            let drawing = ActiveDrawing::new(tool, pixel);
            (
                rasterize_drawing(&drawing, width, height, tools.brush()),
                stroke_color(tools, tool),
            )
        }
        None => (Vec::new(), color),
    }
}
