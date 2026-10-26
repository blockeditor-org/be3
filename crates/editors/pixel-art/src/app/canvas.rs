use block_editor_beui::beui::icons::ICON_ARROW_RIGHT;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    Color32, CursorIcon, ImageFit, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, Vec2,
};
use pixel_art_types::{PixelArtOperation, PixelColor, PixelUpdate};

use crate::canvas::ZOOM_STEP;
use crate::canvas::{canvas_rect, pixel_at};
use crate::color::format_hex_color;
use crate::drawing::{ActiveDrawing, MAX_BRUSH_SIZE, PixelTool, rasterize_drawing};

use super::pane::{Pane, Shown};
use super::state::Tools;
use beui::ItemSize;

const GRID_MIN_CELL: f32 = 6.0;
const GRID_ALPHA: u8 = 60;
const BORDER_ALPHA: u8 = 160;
const HOVER_LABEL_PADDING: Space = Space::Sm;
const HOVER_LABEL_HEIGHT: Size = Size::Md;

#[component]
pub(crate) fn ArtworkCanvas(
    tools: Tools,
    pane: Pane,
    shown: ReadSignal<Shown>,
    size: Memo<(u16, u16)>,
    hovered: Signal<Option<(u16, u16)>>,
) -> NodeId {
    let editor = tools.editor();
    let view = editor.canvas();
    let scale = editor.scale();
    let placed_held = create_signal(Rect::ZERO);
    let placed = placed_held.read_only();
    let world = editor.world();
    let artwork = create_memo(move || {
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
    });

    let held = held_modifiers();
    create_effect(move || {
        let shift = held.get().shift;
        untrack(|| tools.constrained.set(shift));
    });

    let on_press = move |press: PointerPress| {
        let (width, height) = size.get_untracked();
        if width == 0 || tools.busy() || !tools.editable() {
            return;
        }
        tools.constrained.set(press.modifiers.shift);
        let Some(pixel) = pixel_of(
            press.pos,
            view.get_untracked(),
            editor.content_rect(),
            artwork.get_untracked(),
            width,
            height,
        ) else {
            return;
        };
        begin(tools, pixel);
    };

    let on_drag = move |press: PointerPress| {
        let (width, height) = size.get_untracked();
        if width == 0 || tools.busy() || !tools.editable() {
            return;
        }
        tools.constrained.set(press.modifiers.shift);
        let Some(pixel) = pixel_of(
            press.pos,
            view.get_untracked(),
            editor.content_rect(),
            artwork.get_untracked(),
            width,
            height,
        ) else {
            return;
        };
        tools.draw(|drawing| {
            if let Some(drawing) = drawing.as_mut() {
                drawing.extend(pixel);
            }
        });
    };

    let on_click_at = move |press: PointerPress| {
        let (width, height) = size.get_untracked();
        if width == 0 || tools.busy() || !tools.editable() {
            return;
        }
        tools.constrained.set(press.modifiers.shift);
        if let Some(pixel) = pixel_of(
            press.pos,
            view.get_untracked(),
            editor.content_rect(),
            artwork.get_untracked(),
            width,
            height,
        ) {
            finish(tools, pixel, width, height);
        }
    };

    let on_secondary = move |press: PointerPress| {
        let (width, height) = size.get_untracked();
        if width == 0 {
            return;
        }
        if let Some(pixel) = pixel_of(
            press.pos,
            view.get_untracked(),
            editor.content_rect(),
            artwork.get_untracked(),
            width,
            height,
        ) {
            sample(tools, pixel);
        }
    };

    let on_hover_move = move |press: PointerPress| {
        let (width, height) = size.get_untracked();
        if width == 0 {
            return;
        }
        tools.constrained.set(press.modifiers.shift);
        hovered.set(pixel_of(
            press.pos,
            view.get_untracked(),
            editor.content_rect(),
            artwork.get_untracked(),
            width,
            height,
        ));
    };
    let on_hover_change = move |inside: bool| {
        if !inside {
            hovered.set(None);
        }
    };

    let theme = use_theme();
    let background = theme.background;
    create_effect(move || {
        let (width, height) = size.get();
        if width == 0 {
            return;
        }
        let (pixels, color) = pending(tools, hovered.get(), width, height);
        let dark = is_dark(background.get());
        pane.refresh(&tools.block(), dark, &pixels, color);
    });

    let on_key = move |press: KeyPress| shortcut(tools, editor, press);

    let content = NodeRef::new();
    editor.content(&content);
    let built = view! {
        <Frame @node_ref={&content} @test_id={"pixel-art.canvas"}>
            <Interactive
                focusable=true
                on_key={on_key}
                cursor=CursorIcon::Crosshair
                on_press={on_press}
                on_drag={on_drag}
                on_click_at={on_click_at}
                on_secondary_press={on_secondary}
                on_hover_move={on_hover_move}
                on_hover_change={on_hover_change}
            >
                <List spacing=Space::None>
                    <Artwork
                        @sizing=ItemSize::Percent(100.0)
                        shown={shown}
                        view={view}
                        scale={scale}
                        artwork={artwork}
                        show_grid={tools.show_grid.read_only()}
                        hovered={hovered.read_only()}
                    />
                </List>
            </Interactive>
        </Frame>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(rect) = placed_measured.get() {
            placed_held.set(rect);
        }
    });
    built
}

fn shortcut(tools: Tools, editor: block_editor_beui::Editor, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt || !press.pressed {
        return !matches!(shortcut_kind(press.key), Shortcut::None | Shortcut::Cancel);
    }
    match shortcut_kind(press.key) {
        Shortcut::Tool(tool) => tools.select_tool(tool),
        Shortcut::Cancel => return tools.draw(|drawing| drawing.take()).is_some(),
        Shortcut::Larger => {
            let size = tools.brush_size.get_untracked();
            tools.brush_size.set((size + 1).min(MAX_BRUSH_SIZE));
        }
        Shortcut::Smaller => {
            let size = tools.brush_size.get_untracked();
            tools.brush_size.set(size.saturating_sub(1).max(1));
        }
        Shortcut::Fill => {
            if matches!(
                tools.tool.get_untracked(),
                PixelTool::Rectangle | PixelTool::Ellipse
            ) {
                let filled = tools.shapes_filled.get_untracked();
                tools.shapes_filled.set(!filled);
            }
        }
        Shortcut::MirrorHorizontal => {
            let on = tools.mirror_horizontal.get_untracked();
            tools.mirror_horizontal.set(!on);
        }
        Shortcut::MirrorVertical => {
            let on = tools.mirror_vertical.get_untracked();
            tools.mirror_vertical.set(!on);
        }
        Shortcut::ZoomIn => editor.zoom(ZOOM_STEP),
        Shortcut::ZoomOut => editor.zoom(1.0 / ZOOM_STEP),
        Shortcut::Fit => tools.fitting.fit(),
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
    shown: ReadSignal<Shown>,
    view: ReadSignal<Option<CanvasView>>,
    scale: ReadSignal<f32>,
    artwork: Memo<Rect>,
    show_grid: ReadSignal<bool>,
    hovered: ReadSignal<Option<(u16, u16)>>,
) -> NodeId {
    let cell = create_memo(move || {
        let width = shown.get().width;
        match width {
            0 => 0.0,
            width => artwork.get().width() / f32::from(width),
        }
    });
    let left = create_memo(move || artwork.get().left());
    let top = create_memo(move || artwork.get().top());
    let width = create_memo(move || artwork.get().width());
    let height = create_memo(move || artwork.get().height());
    let image = create_memo(move || shown.get().artwork);
    let preview = create_memo(move || shown.get().preview);
    let has_preview = create_memo(move || preview.get().is_some());
    let preview_image = create_memo(move || preview.get().map(|(_, _, _, _, image)| image));
    let preview_x = create_memo(move || {
        left.get()
            + preview
                .get()
                .map_or(0.0, |(edge, _, _, _, _)| f32::from(edge))
                * cell.get()
    });
    let preview_y = create_memo(move || {
        top.get()
            + preview
                .get()
                .map_or(0.0, |(_, edge, _, _, _)| f32::from(edge))
                * cell.get()
    });
    let preview_width = create_memo(move || {
        preview
            .get()
            .map_or(0.0, |(edge, _, far, _, _)| f32::from(far - edge + 1))
            * cell.get()
    });
    let preview_height = create_memo(move || {
        preview
            .get()
            .map_or(0.0, |(_, edge, _, far, _)| f32::from(far - edge + 1))
            * cell.get()
    });
    let thin = create_memo(move || 1.0 / scale.get().max(f32::EPSILON));
    let lines = create_memo(move || {
        let shown = shown.get();
        if !show_grid.get() || cell.get() * scale.get() < GRID_MIN_CELL {
            return Vec::new();
        }
        (1..shown.width)
            .map(Line::Vertical)
            .chain((1..shown.height).map(Line::Horizontal))
            .collect::<Vec<Line>>()
    });
    let hover_x = create_memo(move || {
        left.get() + hovered.get().map_or(0.0, |(x, _)| f32::from(x)) * cell.get()
    });
    let hover_y = create_memo(move || {
        top.get() + hovered.get().map_or(0.0, |(_, y)| f32::from(y)) * cell.get()
    });
    let over = create_memo(move || hovered.get().is_some());
    let hover_size = create_memo(move || match over.get() {
        true => cell.get(),
        false => 0.0,
    });
    view! {
        <Canvas view={view}>
            <CanvasItem
                x={left}
                y={top}
                width={width}
                height={height}
                @test_id={"pixel-art.artwork"}
            >
                <Picture image={image} fit=ImageFit::Fill smooth=false />
            </CanvasItem>
            <CanvasItem x={left} y={top} width={width} height={height}>
                <Frame
                    border={create_memo(move || Some(Border::new(ColorRole::Custom(Color32::from_rgba_unmultiplied(0, 0, 0, BORDER_ALPHA))).width(Stroke::Custom(thin.get()))))}
                />
            </CanvasItem>
            <Show condition={has_preview}>
                <CanvasItem
                    x={preview_x}
                    y={preview_y}
                    width={preview_width}
                    height={preview_height}
                >
                    <Picture image={preview_image} fit=ImageFit::Fill smooth=false />
                </CanvasItem>
            </Show>
            <ForEach keys={lines}>|line: Line| {
                let (x, y, line_width, line_height) = match line {
                    Line::Vertical(at) => (
                        create_memo(move || {
                            left.get() + f32::from(at) * cell.get()
                        }),
                        top,
                        thin,
                        height,
                    ),
                    Line::Horizontal(at) => (
                        left,
                        create_memo(move || {
                            top.get() + f32::from(at) * cell.get()
                        }),
                        width,
                        thin,
                    ),
                };
                view! {
                    <CanvasItem x={x} y={y} width={line_width} height={line_height}>
                        <Frame
                            fill={ColorRole::Custom(Color32::from_rgba_unmultiplied(0, 0, 0, GRID_ALPHA))}
                        />
                    </CanvasItem>
                }
            }</ForEach>
            <Show condition={over}>
                <CanvasItem x={hover_x} y={hover_y} width={hover_size} height={hover_size}>
                    <Frame
                        border={Border::new(ColorRole::Custom(Color32::WHITE)).width(Stroke::Hairline)}
                    />
                </CanvasItem>
            </Show>
        </Canvas>
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Line {
    Vertical(u16),
    Horizontal(u16),
}

#[component]
pub(crate) fn HoverLabel(tools: Tools, hovered: ReadSignal<Option<(u16, u16)>>) -> NodeId {
    let color = tools.color;
    let tool = tools.tool;
    let source = create_memo(move || {
        if tool.get() != PixelTool::ReplaceColor {
            return None;
        }
        let (x, y) = hovered.get()?;
        tools.pixel(x, y)
    });
    let label = create_memo(move || {
        let Some((x, y)) = hovered.get() else {
            return String::new();
        };
        match source.get() {
            Some(source) => format!(
                "{x}, {y} · {} {} {}",
                format_hex_color(source),
                ICON_ARROW_RIGHT.as_str(),
                format_hex_color(color.get())
            ),
            None => format!("{x}, {y}"),
        }
    });
    view! {
        <Frame height=HOVER_LABEL_HEIGHT padding=HOVER_LABEL_PADDING>
            <Text style=TextStyle::CODE @test_id={"pixel-art.hover"}>{label}</Text>
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

fn begin(tools: Tools, pixel: (u16, u16)) {
    let tool = tools.tool.get_untracked();
    match tool {
        tool if tool.is_drawing() => {
            tools.draw(|drawing| *drawing = Some(ActiveDrawing::new(tool, pixel)));
        }
        _ => {}
    }
}

fn finish(tools: Tools, pixel: (u16, u16), width: u16, height: u16) {
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

fn commit(tools: Tools, width: u16, height: u16) {
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

fn sample(tools: Tools, pixel: (u16, u16)) {
    let Some(color) = tools.pixel(pixel.0, pixel.1) else {
        return;
    };
    tools.set_active_color(color, true);
    if tools.tool.get_untracked() == PixelTool::Eyedropper {
        tools.select_tool(tools.previous_drawing_tool());
    }
}

fn stroke_color(tools: Tools, tool: PixelTool) -> PixelColor {
    match tool == PixelTool::Eraser {
        true => PixelColor::TRANSPARENT,
        false => tools.color.get_untracked(),
    }
}

fn pending(
    tools: Tools,
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
