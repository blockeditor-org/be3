use block_editor_beui::Toolbar;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_DOWN_LEFT, ICON_ARROW_DOWN_RIGHT, ICON_ARROW_LEFT,
    ICON_ARROW_RIGHT, ICON_ARROW_UP, ICON_ARROW_UP_LEFT, ICON_ARROW_UP_RIGHT, ICON_CIRCLE,
    ICON_DOWNLOAD, ICON_ERASER, ICON_PAINT_BUCKET, ICON_PENCIL, ICON_PIPETTE, ICON_PLUS,
    ICON_REPLACE, ICON_SCALING, ICON_SCAN, ICON_SLASH, ICON_SQUARE, ICON_TRASH, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Dialog as Modal;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::Pressable;
use pixel_art_types::{
    MAX_PIXEL_ART_PALETTE_COLORS, MAX_PIXEL_ART_SIZE, PixelArtAnchor, PixelArtOperation, PixelColor,
};

use crate::canvas::ZOOM_STEP;
use crate::color::{parse_hex_color, swatch_color};
use crate::drawing::{BrushShape, MAX_BRUSH_SIZE, PixelTool};

use crate::app::state::{Dialog, Tools};

const SPACING: f32 = 8.0;
const SWATCH: f32 = 20.0;
const TOOL_WIDTH: f32 = 76.0;
const DIALOG_WIDTH: f32 = 300.0;

const TOOLS: [(PixelTool, Glyph, &str); 8] = [
    (PixelTool::Pencil, ICON_PENCIL, "B or P"),
    (PixelTool::Eraser, ICON_ERASER, "E"),
    (PixelTool::Fill, ICON_PAINT_BUCKET, "G"),
    (PixelTool::Eyedropper, ICON_PIPETTE, "I"),
    (PixelTool::Line, ICON_SLASH, "L"),
    (PixelTool::ReplaceColor, ICON_REPLACE, "C"),
    (PixelTool::Rectangle, ICON_SQUARE, "R"),
    (PixelTool::Ellipse, ICON_CIRCLE, "O"),
];

const ANCHORS: [(PixelArtAnchor, Glyph, &str); 9] = [
    (PixelArtAnchor::TopLeft, ICON_ARROW_UP_LEFT, "Top left"),
    (PixelArtAnchor::Top, ICON_ARROW_UP, "Top"),
    (PixelArtAnchor::TopRight, ICON_ARROW_UP_RIGHT, "Top right"),
    (PixelArtAnchor::Left, ICON_ARROW_LEFT, "Left"),
    (PixelArtAnchor::Center, ICON_CIRCLE, "Centre"),
    (PixelArtAnchor::Right, ICON_ARROW_RIGHT, "Right"),
    (
        PixelArtAnchor::BottomLeft,
        ICON_ARROW_DOWN_LEFT,
        "Bottom left",
    ),
    (PixelArtAnchor::Bottom, ICON_ARROW_DOWN, "Bottom"),
    (
        PixelArtAnchor::BottomRight,
        ICON_ARROW_DOWN_RIGHT,
        "Bottom right",
    ),
];

#[component]
pub(crate) fn TopBar(
    tools: Tools,
    size: Memo<(u16, u16)>,
    shown: ReadSignal<bool>,
) -> DynamicSegment<ListChild> {
    let editor = tools.editor();
    let tool = tools.tool;
    let label = create_memo(move || tool.get().label().to_owned());
    let dimensions = create_memo(move || {
        let (width, height) = size.get();
        format!("{width} × {height} px")
    });
    let scale = editor.scale();
    let percent = create_memo(move || format!("{:.0}%", scale.get() * 100.0));
    let read_only = editor.read_only();

    let zoom_out = move || editor.zoom(1.0 / ZOOM_STEP);
    let zoom_in = move || editor.zoom(ZOOM_STEP);
    let reset = move || {
        editor.zoom(1.0 / scale.get_untracked().max(f32::EPSILON));
    };
    let fit = move || tools.fitting.fit();
    let grid = tools.show_grid;
    let toggle_grid = move |on: bool| grid.set(on);

    let export = move || super::app::export(tools);
    let clear = move || tools.open_dialog(Dialog::Clear);
    let resize = move || {
        let (width, height) = size.get_untracked();
        tools.resize_width.set(width);
        tools.resize_height.set(height);
        tools.resize_anchor.set(PixelArtAnchor::Center);
        tools.open_dialog(Dialog::Resize);
    };

    let failure = tools.export_error;
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    view! {
        <Toolbar shown={shown}>
            <Text>{label}</Text>
            <Text style=TextStyle::CAPTION>{dimensions}</Text>
            <IconButton
                glyph=ICON_ZOOM_OUT
                label="Zoom out"
                @test_id={"pixel-art.zoom-out"}
                on_click={zoom_out}
            />
            <Button
                label={percent}
                variant=ButtonVariant::Secondary
                @test_id={"pixel-art.zoom-reset"}
                on_click={reset}
            />
            <IconButton
                glyph=ICON_ZOOM_IN
                label="Zoom in"
                @test_id={"pixel-art.zoom-in"}
                on_click={zoom_in}
            />
            <IconButton
                glyph=ICON_SCAN
                label="Fit the canvas to the viewport"
                @test_id={"pixel-art.fit"}
                on_click={fit}
            />
            <Checkbox label="Grid" checked={Binding::new(grid, toggle_grid)} />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <IconButton
                glyph=ICON_SCALING
                label="Resize"
                disabled={read_only}
                @test_id={"pixel-art.resize"}
                on_click={resize}
            />
            <IconButton
                glyph=ICON_TRASH
                label="Clear"
                disabled={read_only}
                @test_id={"pixel-art.clear"}
                on_click={clear}
            />
            <IconButton
                glyph=ICON_DOWNLOAD
                label="Export a PNG"
                disabled={read_only}
                @test_id={"pixel-art.export"}
                on_click={export}
            />
        </Toolbar>
    }
}

#[component]
pub(crate) fn ToolsPanel(tools: Tools) -> NodeId {
    let chosen = tools.tool;
    let rows = create_memo(|| (0..TOOLS.len() / 2).collect::<Vec<usize>>());
    let brush_size = tools.brush_size;
    let size_value = create_memo(move || f64::from(brush_size.get()));
    let shape = tools.brush_shape;
    let square = create_memo(move || shape.get() == BrushShape::Square);
    let circle = create_memo(move || shape.get() == BrushShape::Circle);
    let filled = tools.shapes_filled;
    let shapes =
        create_memo(move || !matches!(chosen.get(), PixelTool::Rectangle | PixelTool::Ellipse));
    let horizontal = tools.mirror_horizontal;
    let vertical = tools.mirror_vertical;
    view! {
        <List spacing=SPACING>
            <Text style=TextStyle::HEADING>"Tools"</Text>
            <ForEach keys={rows}>|row: usize| {
                let (left, right) = (TOOLS[row * 2], TOOLS[row * 2 + 1]);
                view! {
                    <List direction=Direction::Horizontal spacing=4.0>
                        <ToolButton
                            @sizing=ItemSize::Fixed(TOOL_WIDTH)
                            tools={tools}
                            entry={left}
                        />
                        <ToolButton
                            @sizing=ItemSize::Fixed(TOOL_WIDTH)
                            tools={tools}
                            entry={right}
                        />
                    </List>
                }
            }</ForEach>
            <Text>"Tool options"</Text>
            <NumberInput
                value={Binding::new(size_value, move |value: f64| brush_size.set(value as u16))}
                min=1.0
                max={f64::from(MAX_BRUSH_SIZE)}
                label="Brush size"
                @test_id={"pixel-art.brush-size"}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <ToggleButton
                    glyph=ICON_SQUARE
                    label="Square brush"
                    icon_only=true
                    pressed={Binding::new(square, move |_| shape.set(BrushShape::Square))}
                />
                <ToggleButton
                    glyph=ICON_CIRCLE
                    label="Circle brush"
                    icon_only=true
                    pressed={Binding::new(circle, move |_| shape.set(BrushShape::Circle))}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <ToggleButton
                glyph=ICON_PAINT_BUCKET
                label="Filled shapes"
                pressed={filled}
                disabled={shapes}
            />
            <Text>"Symmetry"</Text>
            <Checkbox label="Mirror horizontally" checked={horizontal} />
            <Checkbox label="Mirror vertically" checked={vertical} />
        </List>
    }
}

#[component]
fn ToolButton(tools: Tools, entry: (PixelTool, Glyph, &'static str)) -> NodeId {
    let (tool, glyph, shortcut) = entry;
    let chosen = tools.tool;
    let pressed = create_memo(move || chosen.get() == tool);
    let select = move |_| tools.select_tool(tool);
    view! {
        <ToggleButton
            glyph={glyph}
            label={format!("{} ({shortcut})", tool.label())}
            icon_only=true
            pressed={Binding::new(pressed, select)}
            @test_id={format!("pixel-art.tool.{}", tool.label())}
        />
    }
}

#[component]
pub(crate) fn ColorsPanel(tools: Tools, palette: Memo<Vec<PixelColor>>) -> NodeId {
    let color = tools.color;
    let hex = tools.color_hex;
    let read_only = tools.editor().read_only();
    let typing = move |typed: String| {
        if let Some(color) = parse_hex_color(&typed) {
            tools.set_active_color(color, false);
        } else {
            tools.set_color_text(typed);
        }
    };
    let channels = create_memo(|| ["Red", "Green", "Blue", "Alpha"].to_vec());
    let recent = tools.recent;
    let recent_colors = create_memo(move || recent.get());

    let can_add = create_memo(move || {
        read_only.get()
            || palette.with(|palette| {
                palette.len() >= MAX_PIXEL_ART_PALETTE_COLORS || palette.contains(&color.get())
            })
    });
    let can_remove = create_memo(move || {
        read_only.get() || palette.with(|palette| !palette.contains(&color.get()))
    });
    let add = move || {
        let mut colors = palette.get_untracked();
        colors.push(color.get_untracked());
        tools.operate(PixelArtOperation::SetPalette { colors });
    };
    let remove = move || {
        let removed = color.get_untracked();
        let colors = palette
            .get_untracked()
            .into_iter()
            .filter(|listed| *listed != removed)
            .collect();
        tools.operate(PixelArtOperation::SetPalette { colors });
    };
    view! {
        <List spacing=SPACING>
            <Text style=TextStyle::HEADING>"Color"</Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Swatch color={color} selected=false on_click={move || {}} />
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(hex, typing)}
                    label="Colour"
                    placeholder="#RRGGBBAA"
                    @test_id={"pixel-art.hex"}
                />
            </List>
            <ForEach keys={channels}>|channel: &'static str|
                <Channel tools={tools} channel={channel} />
            </ForEach>
            <Text>"Palette"</Text>
            <List direction=Direction::Horizontal wrap=true spacing=4.0>
                <ForEach keys={palette}>|swatch: PixelColor| {
                    let chosen = create_memo(move || color.get() == swatch);
                    let pick = move || tools.set_active_color(swatch, true);
                    view! {
                        <Swatch color={swatch} selected={chosen} on_click={pick} />
                    }
                }</ForEach>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <IconButton
                    glyph=ICON_PLUS
                    label="Add the active colour to the palette"
                    disabled={can_add}
                    @test_id={"pixel-art.palette-add"}
                    on_click={add}
                />
                <IconButton
                    glyph=ICON_TRASH
                    label="Remove the active colour from the palette"
                    disabled={can_remove}
                    @test_id={"pixel-art.palette-remove"}
                    on_click={remove}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Text>"Recent"</Text>
            <List direction=Direction::Horizontal wrap=true spacing=4.0>
                <ForEach keys={recent_colors}>|swatch: PixelColor|
                    <Swatch color={swatch} selected=false on_click={move || {}} />
                </ForEach>
            </List>
        </List>
    }
}

#[component]
fn Channel(tools: Tools, channel: &'static str) -> NodeId {
    let index = match channel {
        "Red" => 0,
        "Green" => 1,
        "Blue" => 2,
        _ => 3,
    };
    let color = tools.color;
    let value = create_memo(move || f64::from(color.get().rgba()[index]));
    let edit = move |typed: f64| {
        let mut rgba = color.get_untracked().rgba();
        rgba[index] = typed.clamp(0.0, 255.0) as u8;
        tools.set_active_color(PixelColor::new(rgba[0], rgba[1], rgba[2], rgba[3]), false);
    };
    view! {
        <NumberInput value={Binding::new(value, edit)} min=0.0 max=255.0 label={channel} />
    }
}

#[component]
fn Swatch(color: Prop<PixelColor>, selected: Prop<bool>, on_click: ClickCallback) -> NodeId {
    let fill = create_memo(move || swatch_color(color.get()));
    let chosen = create_memo(move || selected.get());
    let theme = use_theme();
    let outline = create_memo(move || match chosen.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    view! {
        <Pressable on_click={on_click}>
            <Frame
                width=SWATCH
                height=SWATCH
                fill={ColorRole::custom(fill)}
                border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get())).width(2.0)))}
                radius=Radius::Xs
            />
        </Pressable>
    }
}

#[component]
pub(crate) fn Dialogs(tools: Tools, size: Memo<(u16, u16)>) -> NodeId {
    let dialog = tools.dialog();
    let resizing = create_memo(move || dialog.get() == Dialog::Resize);
    let clearing = create_memo(move || dialog.get() == Dialog::Clear);
    let width = tools.resize_width;
    let height = tools.resize_height;
    let width_value = create_memo(move || f64::from(width.get()));
    let height_value = create_memo(move || f64::from(height.get()));
    let cropping = create_memo(move || {
        let (current_width, current_height) = size.get();
        width.get() < current_width || height.get() < current_height
    });
    let anchors = create_memo(|| (0..3).collect::<Vec<usize>>());

    let apply = move || {
        tools.operate(PixelArtOperation::Resize {
            width: width.get_untracked(),
            height: height.get_untracked(),
            anchor: tools.resize_anchor.get_untracked(),
        });
        tools.fitting.fit();
        tools.close_dialog();
    };
    let clear = move || {
        tools.operate(PixelArtOperation::Clear);
        tools.close_dialog();
    };
    let cancel_resize = move || tools.close_dialog();
    let dismiss_resize = move || tools.close_dialog();
    let cancel_clear = move || tools.close_dialog();
    let dismiss_clear = move || tools.close_dialog();
    view! {
        <List spacing=0.0>
            <Modal
                open={resizing}
                title="Resize pixel art"
                width=DIALOG_WIDTH
                on_dismiss={dismiss_resize}
            >
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            value={Binding::new(width_value, move |value: f64| width.set(value as u16))}
                            min=1.0
                            max={f64::from(MAX_PIXEL_ART_SIZE)}
                            label="Width"
                            @test_id={"pixel-art.resize-width"}
                        />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            value={Binding::new(height_value, move |value: f64| height.set(value as u16))}
                            min=1.0
                            max={f64::from(MAX_PIXEL_ART_SIZE)}
                            label="Height"
                            @test_id={"pixel-art.resize-height"}
                        />
                    </List>
                    <Text style=TextStyle::CAPTION>"Anchor"</Text>
                    <ForEach keys={anchors}>|row: usize|
                        <AnchorRow tools={tools} row={row} />
                    </ForEach>
                    <Show condition={cropping}>
                        <Text style=TextStyle::CAPTION color=TextColor::Warning>
                            "Shrinking crops pixels outside the anchored region."
                        </Text>
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            label="Resize"
                            variant=ButtonVariant::Primary
                            @test_id={"pixel-art.resize-apply"}
                            on_click={apply}
                        />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            @test_id={"pixel-art.resize-cancel"}
                            on_click={cancel_resize}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                </List>
            </Modal>
            <Modal
                open={clearing}
                title="Clear pixel art?"
                width=DIALOG_WIDTH
                on_dismiss={dismiss_clear}
            >
                <List spacing=SPACING>
                    <Text style=TextStyle::CAPTION>"This makes every pixel transparent."</Text>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            label="Clear"
                            variant=ButtonVariant::Primary
                            @test_id={"pixel-art.clear-apply"}
                            on_click={clear}
                        />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            @test_id={"pixel-art.clear-cancel"}
                            on_click={cancel_clear}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                </List>
            </Modal>
        </List>
    }
}

#[component]
fn AnchorRow(tools: Tools, row: usize) -> NodeId {
    let entries = create_memo(move || (0..3).map(|column| row * 3 + column).collect::<Vec<_>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ForEach keys={entries}>|index: usize| {
                let (anchor, glyph, label) = ANCHORS[index];
                let chosen = tools.resize_anchor;
                let pressed = create_memo(move || chosen.get() == anchor);
                let set = tools.resize_anchor;
                view! {
                    <ToggleButton
                        glyph={glyph}
                        label={label}
                        icon_only=true
                        pressed={Binding::new(pressed, move |_| set.set(anchor))}
                    />
                }
            }</ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}
