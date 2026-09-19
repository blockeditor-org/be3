use std::rc::Rc;

use block_client::blocks::pixel_art::{
    MAX_PIXEL_ART_PALETTE_COLORS, MAX_PIXEL_ART_SIZE, PixelArtAnchor, PixelArtOperation, PixelColor,
};
use block_editor_plugin::Toolbar;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_DOWNWARD, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD,
    ICON_CIRCLE, ICON_COLORIZE, ICON_CROP_SQUARE, ICON_DELETE, ICON_DIAGONAL_LINE, ICON_DOWNLOAD,
    ICON_DRAW, ICON_FIND_REPLACE, ICON_FIT_SCREEN, ICON_FORMAT_COLOR_FILL, ICON_INK_ERASER,
    ICON_NORTH_EAST, ICON_NORTH_WEST, ICON_RESIZE, ICON_SOUTH_EAST, ICON_SOUTH_WEST, ICON_SQUARE,
    ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_plugin::beui::reactive::{
    Align, ClickCallback, Direction, ForEach, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show,
    Spacer, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, Dialog as Modal, Heading, IconButton,
    NumberInput, TextInput, ToggleButton, use_theme,
};
use block_editor_plugin::beui::unstyled::Pressable;

use crate::canvas::ZOOM_STEP;
use crate::color::{parse_hex_color, swatch_color};
use crate::drawing::{BrushShape, MAX_BRUSH_SIZE, PixelTool};

use crate::app::state::{Dialog, Tools};

const SPACING: f32 = 8.0;
const SWATCH: f32 = 20.0;
const TOOL_WIDTH: f32 = 76.0;
const DIALOG_WIDTH: f32 = 300.0;

const TOOLS: [(PixelTool, &str, &str); 8] = [
    (PixelTool::Pencil, ICON_DRAW, "B or P"),
    (PixelTool::Eraser, ICON_INK_ERASER, "E"),
    (PixelTool::Fill, ICON_FORMAT_COLOR_FILL, "G"),
    (PixelTool::Eyedropper, ICON_COLORIZE, "I"),
    (PixelTool::Line, ICON_DIAGONAL_LINE, "L"),
    (PixelTool::ReplaceColor, ICON_FIND_REPLACE, "C"),
    (PixelTool::Rectangle, ICON_CROP_SQUARE, "R"),
    (PixelTool::Ellipse, ICON_CIRCLE, "O"),
];

const ANCHORS: [(PixelArtAnchor, &str, &str); 9] = [
    (PixelArtAnchor::TopLeft, ICON_NORTH_WEST, "Top left"),
    (PixelArtAnchor::Top, ICON_ARROW_UPWARD, "Top"),
    (PixelArtAnchor::TopRight, ICON_NORTH_EAST, "Top right"),
    (PixelArtAnchor::Left, ICON_ARROW_BACK, "Left"),
    (PixelArtAnchor::Center, ICON_CIRCLE, "Centre"),
    (PixelArtAnchor::Right, ICON_ARROW_FORWARD, "Right"),
    (PixelArtAnchor::BottomLeft, ICON_SOUTH_WEST, "Bottom left"),
    (PixelArtAnchor::Bottom, ICON_ARROW_DOWNWARD, "Bottom"),
    (PixelArtAnchor::BottomRight, ICON_SOUTH_EAST, "Bottom right"),
];

#[component]
pub(crate) fn TopBar(tools: Rc<Tools>, size: Memo<(u16, u16)>, shown: ReadSignal<bool>) -> NodeId {
    let editor = tools.editor().clone();
    let tool = tools.tool.clone();
    let label = create_memo(clone!(tool -> move || tool.get().label().to_owned()));
    let dimensions = create_memo(clone!(size -> move || {
        let (width, height) = size.get();
        format!("{width} × {height} px")
    }));
    let scale = editor.scale();
    let percent = create_memo(clone!(scale -> move || format!("{:.0}%", scale.get() * 100.0)));
    let read_only = editor.read_only();

    let zoom_out = clone!(editor -> move || editor.zoom(1.0 / ZOOM_STEP));
    let zoom_in = clone!(editor -> move || editor.zoom(ZOOM_STEP));
    let reset = clone!(editor scale -> move || {
        editor.zoom(1.0 / scale.get_untracked().max(f32::EPSILON));
    });
    let fit = clone!(editor -> move || editor.fit());
    let grid = tools.show_grid.clone();
    let set_grid = tools.set_show_grid.clone();
    let toggle_grid = move |on: bool| set_grid.set(on);

    let export = clone!(tools -> move || super::app::export(&tools));
    let clear = clone!(tools -> move || tools.open_dialog(Dialog::Clear));
    let resize = clone!(tools size -> move || {
        let (width, height) = size.get_untracked();
        tools.set_resize_width.set(width);
        tools.set_resize_height.set(height);
        tools.set_resize_anchor.set(PixelArtAnchor::Center);
        tools.open_dialog(Dialog::Resize);
    });

    let failure = tools.export_error.clone();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let theme = use_theme();
    view! {
        <Toolbar shown={shown}>
            <Body content={label} />
            <Caption content={dimensions} />
            <IconButton
                glyph={ICON_ZOOM_OUT.to_owned()}
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
                glyph={ICON_ZOOM_IN.to_owned()}
                label="Zoom in"
                @test_id={"pixel-art.zoom-in"}
                on_click={zoom_in}
            />
            <IconButton
                glyph={ICON_FIT_SCREEN.to_owned()}
                label="Fit the canvas to the viewport"
                @test_id={"pixel-art.fit"}
                on_click={fit}
            />
            <Checkbox label="Grid" checked={grid} on_change={toggle_grid} />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <IconButton
                glyph={ICON_RESIZE.to_owned()}
                label="Resize"
                disabled={read_only.clone()}
                @test_id={"pixel-art.resize"}
                on_click={resize}
            />
            <IconButton
                glyph={ICON_DELETE.to_owned()}
                label="Clear"
                disabled={read_only.clone()}
                @test_id={"pixel-art.clear"}
                on_click={clear}
            />
            <IconButton
                glyph={ICON_DOWNLOAD.to_owned()}
                label="Export a PNG"
                disabled={read_only}
                @test_id={"pixel-art.export"}
                on_click={export}
            />
        </Toolbar>
    }
}

#[component]
pub(crate) fn ToolsPanel(tools: Rc<Tools>) -> NodeId {
    let chosen = tools.tool.clone();
    let rows = create_memo(|| (0..TOOLS.len() / 2).collect::<Vec<usize>>());
    let brush_size = tools.brush_size.clone();
    let set_brush_size = tools.set_brush_size.clone();
    let size_value = create_memo(clone!(brush_size -> move || f64::from(brush_size.get())));
    let shape = tools.brush_shape.clone();
    let set_shape = tools.set_brush_shape.clone();
    let square = create_memo(clone!(shape -> move || shape.get() == BrushShape::Square));
    let circle = create_memo(clone!(shape -> move || shape.get() == BrushShape::Circle));
    let filled = tools.shapes_filled.clone();
    let set_filled = tools.set_shapes_filled.clone();
    let shapes = create_memo(clone!(chosen -> move || {
        !matches!(chosen.get(), PixelTool::Rectangle | PixelTool::Ellipse)
    }));
    let horizontal = tools.mirror_horizontal.clone();
    let set_horizontal = tools.set_mirror_horizontal.clone();
    let vertical = tools.mirror_vertical.clone();
    let set_vertical = tools.set_mirror_vertical.clone();
    let square_shape = set_shape.clone();
    let tool_rows = Rc::clone(&tools);
    view! {
        <List spacing=SPACING>
            <Heading content="Tools" />
            <ForEach keys={rows}>
                {move |row: usize| {
                    let (left, right) = (TOOLS[row * 2], TOOLS[row * 2 + 1]);
                    let first = Rc::clone(&tool_rows);
                    let second = Rc::clone(&tool_rows);
                    view! {
                        <List direction=Direction::Horizontal spacing=4.0>
                            <ToolButton
                                @sizing=ItemSize::Fixed(TOOL_WIDTH)
                                tools={first}
                                entry={left}
                            />
                            <ToolButton
                                @sizing=ItemSize::Fixed(TOOL_WIDTH)
                                tools={second}
                                entry={right}
                            />
                        </List>
                    }
                }}
            </ForEach>
            <Body content="Tool options" />
            <NumberInput
                value={size_value}
                min=1.0
                max={f64::from(MAX_BRUSH_SIZE)}
                label="Brush size"
                @test_id={"pixel-art.brush-size"}
                on_change={move |value: f64| set_brush_size.set(value as u16)}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <ToggleButton
                    glyph={ICON_SQUARE.to_owned()}
                    label="Square brush"
                    pressed={square}
                    on_change={move |_| square_shape.set(BrushShape::Square)}
                />
                <ToggleButton
                    glyph={ICON_CIRCLE.to_owned()}
                    label="Circle brush"
                    pressed={circle}
                    on_change={move |_| set_shape.set(BrushShape::Circle)}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <ToggleButton
                glyph={ICON_FORMAT_COLOR_FILL.to_owned()}
                label="Fill rectangles and ellipses"
                pressed={filled}
                disabled={shapes}
                on_change={move |on| set_filled.set(on)}
            />
            <Body content="Symmetry" />
            <Checkbox
                label="Mirror horizontally"
                checked={horizontal}
                on_change={move |on| set_horizontal.set(on)}
            />
            <Checkbox
                label="Mirror vertically"
                checked={vertical}
                on_change={move |on| set_vertical.set(on)}
            />
        </List>
    }
}

#[component]
fn ToolButton(tools: Rc<Tools>, entry: (PixelTool, &'static str, &'static str)) -> NodeId {
    let (tool, glyph, shortcut) = entry;
    let chosen = tools.tool.clone();
    let pressed = create_memo(move || chosen.get() == tool);
    let select = clone!(tools -> move |_| tools.select_tool(tool));
    view! {
        <ToggleButton
            glyph={glyph.to_owned()}
            label={format!("{} ({shortcut})", tool.label())}
            pressed={pressed}
            @test_id={format!("pixel-art.tool.{}", tool.label())}
            on_change={select}
        />
    }
}

#[component]
pub(crate) fn ColorsPanel(tools: Rc<Tools>, palette: Memo<Vec<PixelColor>>) -> NodeId {
    let color = tools.color.clone();
    let hex = tools.color_hex.clone();
    let read_only = tools.editor().read_only();
    let typing = clone!(tools -> move |typed: String| {
        if let Some(color) = parse_hex_color(&typed) {
            tools.set_active_color(color, false);
        } else {
            tools.set_color_text(typed);
        }
    });
    let channels = create_memo(|| ["Red", "Green", "Blue", "Alpha"].to_vec());
    let recent = tools.recent.clone();
    let recent_colors = create_memo(clone!(recent -> move || recent.get()));

    let can_add = create_memo(clone!(palette color read_only -> move || {
        read_only.get()
            || palette.with(|palette| {
                palette.len() >= MAX_PIXEL_ART_PALETTE_COLORS || palette.contains(&color.get())
            })
    }));
    let can_remove = create_memo(clone!(palette color read_only -> move || {
        read_only.get() || palette.with(|palette| !palette.contains(&color.get()))
    }));
    let add = clone!(tools palette color -> move || {
        let mut colors = palette.get_untracked();
        colors.push(color.get_untracked());
        tools.operate(PixelArtOperation::SetPalette { colors });
    });
    let remove = clone!(tools palette color -> move || {
        let removed = color.get_untracked();
        let colors = palette
            .get_untracked()
            .into_iter()
            .filter(|listed| *listed != removed)
            .collect();
        tools.operate(PixelArtOperation::SetPalette { colors });
    });
    let active = color.clone();
    let palette_color = color.clone();
    let palette_tools = Rc::clone(&tools);
    let channel_tools = Rc::clone(&tools);
    view! {
        <List spacing=SPACING>
            <Heading content="Color" />
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Swatch color={active} selected=false on_click={move || {}} />
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={hex}
                    label="Colour"
                    placeholder="#RRGGBBAA"
                    @test_id={"pixel-art.hex"}
                    on_change={typing}
                />
            </List>
            <ForEach keys={channels}>
                {move |channel: &'static str| {
                    let tools = Rc::clone(&channel_tools);
                    view! {
                        <Channel tools={tools} channel={channel} />
                    }
                }}
            </ForEach>
            <Body content="Palette" />
            <List direction=Direction::Horizontal wrap=true spacing=4.0>
                <ForEach keys={palette}>
                    {move |swatch: PixelColor| {
                        let active = palette_color.clone();
                        let chosen = create_memo(move || active.get() == swatch);
                        let picker = Rc::clone(&palette_tools);
                        let pick = move || picker.set_active_color(swatch, true);
                        view! {
                            <Swatch color={swatch} selected={chosen} on_click={pick} />
                        }
                    }}
                </ForEach>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <IconButton
                    glyph={ICON_ADD.to_owned()}
                    label="Add the active colour to the palette"
                    disabled={can_add}
                    @test_id={"pixel-art.palette-add"}
                    on_click={add}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Remove the active colour from the palette"
                    disabled={can_remove}
                    @test_id={"pixel-art.palette-remove"}
                    on_click={remove}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Body content="Recent" />
            <List direction=Direction::Horizontal wrap=true spacing=4.0>
                <ForEach keys={recent_colors}>
                    {move |swatch: PixelColor| {
                        view! {
                            <Swatch color={swatch} selected=false on_click={move || {}} />
                        }
                    }}
                </ForEach>
            </List>
        </List>
    }
}

#[component]
fn Channel(tools: Rc<Tools>, channel: &'static str) -> NodeId {
    let index = match channel {
        "Red" => 0,
        "Green" => 1,
        "Blue" => 2,
        _ => 3,
    };
    let color = tools.color.clone();
    let value = create_memo(clone!(color -> move || f64::from(color.get().rgba()[index])));
    let edit = clone!(tools color -> move |typed: f64| {
        let mut rgba = color.get_untracked().rgba();
        rgba[index] = typed.clamp(0.0, 255.0) as u8;
        tools.set_active_color(PixelColor::new(rgba[0], rgba[1], rgba[2], rgba[3]), false);
    });
    view! {
        <NumberInput value={value} min=0.0 max=255.0 label={channel} on_change={edit} />
    }
}

#[component]
fn Swatch(color: Prop<PixelColor>, selected: Prop<bool>, on_click: ClickCallback) -> NodeId {
    let fill = create_memo(move || swatch_color(color.get()));
    let chosen = create_memo(move || selected.get());
    let theme = use_theme();
    let outline = create_memo(clone!(theme chosen -> move || match chosen.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    view! {
        <Pressable on_click={move || on_click.call()}>
            <Frame
                width=SWATCH
                height=SWATCH
                color={fill}
                outline={outline}
                outline_width=2.0
                outline_visible=true
                radius=2
            />
        </Pressable>
    }
}

#[component]
pub(crate) fn Dialogs(tools: Rc<Tools>, size: Memo<(u16, u16)>) -> NodeId {
    let dialog = tools.dialog();
    let resizing = create_memo(clone!(dialog -> move || dialog.get() == Dialog::Resize));
    let clearing = create_memo(clone!(dialog -> move || dialog.get() == Dialog::Clear));
    let width = tools.resize_width.clone();
    let height = tools.resize_height.clone();
    let set_width = tools.set_resize_width.clone();
    let set_height = tools.set_resize_height.clone();
    let width_value = create_memo(clone!(width -> move || f64::from(width.get())));
    let height_value = create_memo(clone!(height -> move || f64::from(height.get())));
    let cropping = create_memo(clone!(width height size -> move || {
        let (current_width, current_height) = size.get();
        width.get() < current_width || height.get() < current_height
    }));
    let anchors = create_memo(|| (0..3).collect::<Vec<usize>>());

    let apply = clone!(tools width height -> move || {
        tools.operate(PixelArtOperation::Resize {
            width: width.get_untracked(),
            height: height.get_untracked(),
            anchor: tools.resize_anchor.get_untracked(),
        });
        tools.editor().fit();
        tools.close_dialog();
    });
    let clear = clone!(tools -> move || {
        tools.operate(PixelArtOperation::Clear);
        tools.close_dialog();
    });
    let cancel_resize = clone!(tools -> move || tools.close_dialog());
    let dismiss_resize = clone!(tools -> move || tools.close_dialog());
    let cancel_clear = clone!(tools -> move || tools.close_dialog());
    let dismiss_clear = clone!(tools -> move || tools.close_dialog());
    let theme = use_theme();
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
                            value={width_value}
                            min=1.0
                            max={f64::from(MAX_PIXEL_ART_SIZE)}
                            label="Width"
                            @test_id={"pixel-art.resize-width"}
                            on_change={move |value: f64| set_width.set(value as u16)}
                        />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            value={height_value}
                            min=1.0
                            max={f64::from(MAX_PIXEL_ART_SIZE)}
                            label="Height"
                            @test_id={"pixel-art.resize-height"}
                            on_change={move |value: f64| set_height.set(value as u16)}
                        />
                    </List>
                    <Caption content="Anchor" />
                    <ForEach keys={anchors}>
                        {move |row: usize| {
                            view! {
                                <AnchorRow tools={Rc::clone(&tools)} row={row} />
                            }
                        }}
                    </ForEach>
                    <Show condition={cropping}>
                        <Caption
                            content="Shrinking crops pixels outside the anchored region."
                            color={theme.warning.clone()}
                            wrap=true
                        />
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
                    <Caption content="This makes every pixel transparent." wrap=true />
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
fn AnchorRow(tools: Rc<Tools>, row: usize) -> NodeId {
    let entries = create_memo(move || (0..3).map(|column| row * 3 + column).collect::<Vec<_>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <ForEach keys={entries}>
                {move |index: usize| {
                    let (anchor, glyph, label) = ANCHORS[index];
                    let chosen = tools.resize_anchor.clone();
                    let pressed = create_memo(move || chosen.get() == anchor);
                    let set = tools.set_resize_anchor.clone();
                    view! {
                        <ToggleButton
                            glyph={glyph.to_owned()}
                            label={label}
                            pressed={pressed}
                            on_change={move |_| set.set(anchor)}
                        />
                    }
                }}
            </ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}
