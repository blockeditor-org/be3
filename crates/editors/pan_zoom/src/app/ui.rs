use std::rc::Rc;

use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::ICON_LEFT_PANEL_OPEN;
use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, CanvasView, ClickCatcher, Column, Frame, ItemSize, Memo, NodeRef,
    ReadSignal, Row, Text, WriteSignal, clone, create_memo, create_signal, intrinsic, size, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Icon, Separator, use_theme};
use block_editor_plugin::beui::unstyled;
use block_editor_plugin::beui::{CursorIcon, Document, Rect, Vec2, pos2, vec2};

const SIDEBAR_WIDTH: f32 = 220.0;
const RAIL_WIDTH: f32 = 44.0;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 10.0;
const HEADING_SIZE: f32 = 16.0;
const LABEL_SIZE: f32 = 12.0;
const CARD_TITLE_SIZE: f32 = 17.0;
const CARD_LABEL_SIZE: f32 = 11.0;
const CARD_PADDING: f32 = 12.0;
const CARD_SPACING: f32 = 6.0;
const CARD_RADIUS: f32 = 10.0;
const CARD_OUTLINE: f32 = 2.0;
const ZOOM_STEP: f32 = 1.25;
const WORLD: Vec2 = Vec2::new(880.0, 580.0);

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

pub fn world_size() -> Vec2 {
    WORLD
}

pub trait Viewport {
    fn zoom(&self, factor: f32);
    fn fit(&self);
    fn focus(&self, target: Rect);
}

pub struct PanZoomUi {
    canvas: NodeRef,
    set_view: WriteSignal<Option<CanvasView>>,
    set_scale: WriteSignal<f32>,
    set_chrome: WriteSignal<bool>,
}

impl PanZoomUi {
    pub fn new(viewport: Rc<dyn Viewport>) -> (Self, NodeId) {
        let (canvas_view, set_view) = create_signal(None::<CanvasView>);
        let (scale, set_scale) = create_signal(1.0_f32);
        let (chrome, set_chrome) = create_signal(true);
        let (open, set_open) = create_signal(true);
        let (selected, set_selected) = create_signal(None::<usize>);
        let canvas = NodeRef::new();
        let canvas_ref = canvas.clone();

        let shown = create_memo(clone!(chrome open -> move || chrome.get() && open.get()));
        let railed = create_memo(clone!(chrome open -> move || chrome.get() && !open.get()));
        let panel = sidebar(
            &viewport,
            &scale,
            &selected,
            &set_selected,
            &set_open,
            &shown,
        );
        let rail = rail(&set_open, &railed);
        let stage = stage(&canvas_ref, &canvas_view, &scale, &selected, &set_selected);
        let children = vec![
            size(panel, ItemSize::Fixed(SIDEBAR_WIDTH)),
            size(rail, ItemSize::Fixed(RAIL_WIDTH)),
            size(stage, ItemSize::Percent(100.0)),
        ];
        let root = view! {
            <Row spacing=0.0 children={children} />
        };

        (
            Self {
                canvas,
                set_view,
                set_scale,
                set_chrome,
            },
            root,
        )
    }

    pub fn canvas_rect(&self, document: &Document) -> Option<Rect> {
        self.canvas
            .try_get()
            .and_then(|canvas| document.node_rect(canvas))
    }

    pub fn set_view(&mut self, view: Option<CanvasView>, scale: f32, chrome: bool) {
        self.set_view.set(view);
        self.set_scale.set(scale);
        self.set_chrome.set(chrome);
    }
}

fn sidebar(
    viewport: &Rc<dyn Viewport>,
    scale: &ReadSignal<f32>,
    selected: &ReadSignal<Option<usize>>,
    set_selected: &WriteSignal<Option<usize>>,
    set_open: &WriteSignal<bool>,
    shown: &Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let readout = create_memo(clone!(scale -> move || format!("Zoom {:.0}%", scale.get() * 100.0)));
    let chosen = create_memo(clone!(selected -> move || match selected.get() {
        Some(index) => format!("Selected {}", CARDS[index].name),
        None => "Nothing selected".to_owned(),
    }));
    let zoom_out = clone!(viewport -> move || viewport.zoom(1.0 / ZOOM_STEP));
    let zoom_in = clone!(viewport -> move || viewport.zoom(ZOOM_STEP));
    let fit = clone!(viewport -> move || viewport.fit());
    let hide = clone!(set_open -> move || set_open.set(false));
    let rows: Vec<_> = CARDS
        .iter()
        .enumerate()
        .map(|(index, card)| intrinsic(focus_row(index, card, viewport, set_selected)))
        .collect();

    view! {
        <Frame
            visible={shown.clone()}
            color={theme.surface.clone()}
            padding_horizontal=PANEL_PADDING
            padding_vertical=PANEL_PADDING
        >
            <Column spacing=PANEL_SPACING>
                <Text
                    string="Pan and Zoom"
                    font_size=HEADING_SIZE
                    color={theme.text.clone()}
                    @test_id={"pan_zoom.title"}
                />
                <Text
                    string={readout}
                    font_size=LABEL_SIZE
                    color={theme.text_muted.clone()}
                    @test_id={"pan_zoom.zoom"}
                />
                <Row spacing=6.0>
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
                </Row>
                <Separator />
                <Text
                    string={chosen}
                    font_size=LABEL_SIZE
                    color={theme.text_muted.clone()}
                    @test_id={"pan_zoom.selected"}
                />
                <Column spacing=4.0 children={rows} />
                <Separator />
                <Button
                    label="Hide sidebar"
                    variant=ButtonVariant::Secondary
                    on_click={hide}
                    @test_id={"pan_zoom.hide_sidebar"}
                />
            </Column>
        </Frame>
    }
}

fn focus_row(
    index: usize,
    card: &'static Card,
    viewport: &Rc<dyn Viewport>,
    set_selected: &WriteSignal<Option<usize>>,
) -> NodeId {
    let viewport = viewport.clone();
    let set_selected = set_selected.clone();
    let focus = move || {
        set_selected.set(Some(index));
        viewport.focus(card.rect());
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

fn rail(set_open: &WriteSignal<bool>, railed: &Memo<bool>) -> NodeId {
    let theme = use_theme();
    let show = clone!(set_open -> move || set_open.set(true));
    view! {
        <Frame
            visible={railed.clone()}
            color={theme.surface.clone()}
            padding_horizontal=6.0
            padding_vertical=PANEL_PADDING
        >
            <Column spacing=0.0>
                <unstyled::Button on_click={show} @test_id={"pan_zoom.show_sidebar"}>
                    <Frame
                        color={theme.surface_raised.clone()}
                        radius=6
                        padding_horizontal=8.0
                        padding_vertical=8.0
                    >
                        <Icon glyph={ICON_LEFT_PANEL_OPEN.to_owned()} color={theme.text.clone()} />
                    </Frame>
                </unstyled::Button>
            </Column>
        </Frame>
    }
}

fn stage(
    canvas: &NodeRef,
    canvas_view: &ReadSignal<Option<CanvasView>>,
    scale: &ReadSignal<f32>,
    selected: &ReadSignal<Option<usize>>,
    set_selected: &WriteSignal<Option<usize>>,
) -> NodeId {
    let cards: Vec<_> = CARDS
        .iter()
        .enumerate()
        .map(|(index, card)| intrinsic(card_node(index, card, scale, selected, set_selected)))
        .collect();
    view! {
        <Canvas view={canvas_view.clone()} children={cards} @node_ref={canvas} />
    }
}

fn card_node(
    index: usize,
    card: &'static Card,
    scale: &ReadSignal<f32>,
    selected: &ReadSignal<Option<usize>>,
    set_selected: &WriteSignal<Option<usize>>,
) -> NodeId {
    let theme = use_theme();
    let chosen = create_memo(clone!(selected -> move || selected.get() == Some(index)));
    let outline = {
        let theme = theme.clone();
        create_memo(clone!(chosen -> move || match chosen.get() {
            true => theme.get().accent,
            false => theme.get().border,
        }))
    };
    let title_size = scaled(scale, CARD_TITLE_SIZE);
    let label_size = scaled(scale, CARD_LABEL_SIZE);
    let padding = scaled(scale, CARD_PADDING);
    let spacing = scaled(scale, CARD_SPACING);
    let radius = create_memo(clone!(scale -> move || (CARD_RADIUS * scale.get()) as u8));
    let outline_width = scaled(scale, CARD_OUTLINE);
    let position = format!("{:.0}, {:.0}", card.x, card.y);
    let select = clone!(set_selected -> move || set_selected.set(Some(index)));

    view! {
        <CanvasItem
            x={card.x}
            y={card.y}
            width={card.width}
            height={card.height}
            @test_id={format!("pan_zoom.card.{index}")}
        >
            <ClickCatcher cursor=CursorIcon::PointingHand on_click={select}>
                <Frame
                    color={theme.surface_raised.clone()}
                    outline={outline}
                    outline_width={outline_width}
                    outline_visible=true
                    radius={radius}
                    padding_horizontal={padding.clone()}
                    padding_vertical={padding}
                >
                    <Column spacing={spacing}>
                        <Text
                            string={card.name}
                            font_size={title_size}
                            color={theme.text.clone()}
                        />
                        <Text
                            string={position}
                            font_size={label_size}
                            color={theme.text_muted.clone()}
                        />
                    </Column>
                </Frame>
            </ClickCatcher>
        </CanvasItem>
    }
}

fn scaled(scale: &ReadSignal<f32>, base: f32) -> Memo<f32> {
    create_memo(clone!(scale -> move || (base * scale.get()).max(1.0)))
}
