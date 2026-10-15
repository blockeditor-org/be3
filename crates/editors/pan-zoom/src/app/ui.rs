use block_editor_beui::Editor;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::icons::ICON_LEFT_PANEL_OPEN;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled;
use block_editor_beui::beui::{CursorIcon, NodeId, Rect, Vec2, pos2, vec2};

const SIDEBAR_WIDTH: f32 = 220.0;
const RAIL_WIDTH: f32 = 44.0;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 10.0;
const CARD_TITLE_SIZE: f32 = 17.0;
const CARD_LABEL_SIZE: f32 = 11.0;
const CARD_PADDING: f32 = 12.0;
const CARD_SPACING: f32 = 6.0;
const CARD_RADIUS: f32 = 10.0;
const CARD_OUTLINE: f32 = 2.0;
const ZOOM_STEP: f32 = 1.25;

pub const WORLD: Vec2 = Vec2::new(880.0, 580.0);

pub struct Card {
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Card {
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(pos2(self.x, self.y), vec2(self.width, self.height))
    }
}

fn card_indices() -> Vec<usize> {
    (0..CARDS.len()).collect()
}

pub const CARDS: &[Card] = &[
    Card {
        name: "Origin",
        x: 0.0,
        y: 0.0,
        width: 210.0,
        height: 110.0,
    },
    Card {
        name: "Sidebar",
        x: 260.0,
        y: 40.0,
        width: 220.0,
        height: 120.0,
    },
    Card {
        name: "Host view",
        x: 540.0,
        y: 0.0,
        width: 260.0,
        height: 130.0,
    },
    Card {
        name: "Canvas",
        x: 40.0,
        y: 210.0,
        width: 240.0,
        height: 140.0,
    },
    Card {
        name: "Zoom",
        x: 360.0,
        y: 230.0,
        width: 180.0,
        height: 100.0,
    },
    Card {
        name: "Pan",
        x: 620.0,
        y: 200.0,
        width: 200.0,
        height: 120.0,
    },
    Card {
        name: "Far corner",
        x: 300.0,
        y: 430.0,
        width: 300.0,
        height: 120.0,
    },
];

#[component]
pub fn PanZoom(editor: Editor) -> NodeId {
    let (open, set_open) = create_signal(true);
    let (selected, set_selected) = create_signal(None::<usize>);
    let stage = NodeRef::new();
    editor.content(&stage);

    let chrome = editor.chrome_shown();
    let panel_shown = create_memo(move || chrome.get() && open.get());
    let rail_shown = create_memo(move || chrome.get() && !open.get());
    let panel = (editor, selected, set_selected, set_open);
    let rail_open = set_open;

    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Show condition={panel_shown}>
                {move || {
                    let (editor, selected, set_selected, set_open) = panel;
                    view! {
                        <Sidebar
                            @sizing=ItemSize::Fixed(SIDEBAR_WIDTH)
                            editor
                            selected
                            set_selected
                            set_open
                        />
                    }
                }}
            </Show>
            <Show condition={rail_shown}>
                <Rail @sizing=ItemSize::Fixed(RAIL_WIDTH) set_open={rail_open} />
            </Show>
            <Stage
                @sizing=ItemSize::Percent(100.0)
                @node_ref={&stage}
                editor={editor}
                selected={selected}
                set_selected={set_selected}
            />
        </List>
    }
}

#[component]
fn Sidebar(
    editor: Editor,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    set_open: WriteSignal<bool>,
) -> NodeId {
    let scale = editor.scale();
    let readout = create_memo(move || format!("Zoom {:.0}%", scale.get() * 100.0));
    let chosen = create_memo(move || match selected.get() {
        Some(index) => format!("Selected {}", CARDS[index].name),
        None => "Nothing selected".to_owned(),
    });
    let zoom_out = move || editor.zoom(1.0 / ZOOM_STEP);
    let zoom_in = move || editor.zoom(ZOOM_STEP);
    let fit = move || editor.fit();
    let hide = move || set_open.set(false);
    view! {
        <Frame fill=ColorRole::Surface padding=PANEL_PADDING>
            <List spacing=PANEL_SPACING>
                <Text string="Pan and Zoom" size=TextSize::Lg @test_id={"pan_zoom.title"} />
                <Text
                    string={readout}
                    size=TextSize::Sm
                    color=TextColor::Muted
                    @test_id={"pan_zoom.zoom"}
                />
                <List direction=Direction::Horizontal spacing=6.0>
                    <Button
                        label="-"
                        variant=ButtonVariant::Secondary
                        on_click={zoom_out}
                        @test_id={"pan_zoom.zoom_out"}
                    />
                    <Button
                        label="+"
                        variant=ButtonVariant::Secondary
                        on_click={zoom_in}
                        @test_id={"pan_zoom.zoom_in"}
                    />
                    <Button
                        label="Fit"
                        variant=ButtonVariant::Secondary
                        on_click={fit}
                        @test_id={"pan_zoom.fit"}
                    />
                </List>
                <Separator />
                <Text
                    string={chosen}
                    size=TextSize::Sm
                    color=TextColor::Muted
                    @test_id={"pan_zoom.selected"}
                />
                <List spacing=4.0>
                    <ForEach keys={card_indices()}>
                        {move |index: usize| view! {
                            <FocusRow index editor={editor} set_selected={set_selected} />
                        }}
                    </ForEach>
                </List>
                <Separator />
                <Button
                    label="Hide sidebar"
                    variant=ButtonVariant::Secondary
                    on_click={hide}
                    @test_id={"pan_zoom.hide_sidebar"}
                />
            </List>
        </Frame>
    }
}

#[component]
fn FocusRow(editor: Editor, index: usize, set_selected: WriteSignal<Option<usize>>) -> NodeId {
    let card: &'static Card = &CARDS[index];
    let focus = move || {
        set_selected.set(Some(index));
        editor.reveal(card.rect());
    };
    view! {
        <Button
            label={card.name}
            variant=ButtonVariant::Secondary
            on_click={focus}
            @test_id={format!("pan_zoom.focus.{index}")}
        />
    }
}

#[component]
fn Rail(set_open: WriteSignal<bool>) -> NodeId {
    let show = move || set_open.set(true);
    view! {
        <Frame fill=ColorRole::Surface padding={Sides::symmetric(6.0, PANEL_PADDING)}>
            <List spacing=0.0>
                <unstyled::Button on_click={show} @test_id={"pan_zoom.show_sidebar"}>
                    <Frame fill=ColorRole::SurfaceRaised radius=Radius::Md padding=8.0>
                        <Text align=TextAlign::Center>
                            <Icon glyph={ICON_LEFT_PANEL_OPEN.to_owned()} />
                        </Text>
                    </Frame>
                </unstyled::Button>
            </List>
        </Frame>
    }
}

#[component]
fn Stage(
    editor: Editor,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
) -> NodeId {
    let scale = editor.scale();
    view! {
        <Canvas view={editor.canvas()}>
            <ForEach keys={card_indices()}>
                {move |index: usize| view! {
                    <CardView index scale={scale} selected={selected} set_selected={set_selected} />
                }}
            </ForEach>
        </Canvas>
    }
}

#[component]
fn CardView(
    index: usize,
    scale: ReadSignal<f32>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
) -> CanvasItem {
    let card: &'static Card = &CARDS[index];
    let theme = use_theme();
    let chosen = create_memo(move || selected.get() == Some(index));
    let outline = {
        create_memo(move || match chosen.get() {
            true => theme.get().accent,
            false => theme.get().border,
        })
    };
    let title_size = scaled(scale, CARD_TITLE_SIZE);
    let label_size = scaled(scale, CARD_LABEL_SIZE);
    let title_size = create_memo(move || TextSize::Custom(title_size.get()));
    let label_size = create_memo(move || TextSize::Custom(label_size.get()));
    let padding = scaled(scale, CARD_PADDING);
    let spacing = scaled(scale, CARD_SPACING);
    let radius = create_memo(move || Radius::custom((CARD_RADIUS * scale.get()).floor()));
    let outline_width = scaled(scale, CARD_OUTLINE);
    let position = format!("{:.0}, {:.0}", card.x, card.y);
    let select = move || set_selected.set(Some(index));

    view! {
        <CanvasItem
            x={card.x}
            y={card.y}
            width={card.width}
            height={card.height}
            @test_id={format!("pan_zoom.card.{index}")}
        >
            <Interactive cursor=CursorIcon::PointingHand on_click={select}>
                <Frame
                    fill=ColorRole::SurfaceRaised
                    border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get())).width(outline_width.get())))}
                    radius=radius
                    padding=padding
                >
                    <List spacing={spacing}>
                        <Text string={card.name} size={title_size} />
                        <Text string={position} size={label_size} color=TextColor::Muted />
                    </List>
                </Frame>
            </Interactive>
        </CanvasItem>
    }
}

fn scaled(scale: ReadSignal<f32>, base: f32) -> Memo<f32> {
    create_memo(move || (base * scale.get()).max(1.0))
}
