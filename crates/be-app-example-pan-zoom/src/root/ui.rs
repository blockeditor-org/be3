use be_sdk_beui::Fit;
use be_sdk_beui::beui::dock::styled::{
    DockBar, DockBarGrip, DockBarSurface, DockPane, DockSplit, DockTab, Docking, DockingLayout,
    SplitSize,
};
use be_sdk_beui::beui::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::icons::{ICON_PANEL_LEFT_OPEN, ICON_SCAN, ICON_ZOOM_IN, ICON_ZOOM_OUT};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{CursorIcon, NodeId, Rect, Vec2, pos2, vec2};
use be_sdk_beui::{Editor, Fitting};

const STAGE: &str = "stage";
const CARDS_TAB: &str = "cards";
const ZOOM: &str = "zoom";
const REOPEN: &str = "reopen";
const SIDEBAR_SHARE: f32 = 0.275;
const PANEL_PADDING: Space = Space::Lg;
const PANEL_SPACING: Space = Space::Md;
const CARD_TITLE_SIZE: f32 = 17.0;
const CARD_LABEL_SIZE: f32 = 11.0;
const CARD_PADDING: f32 = 12.0;
const CARD_SPACING: f32 = 6.0;
const CARD_OUTLINE: f32 = 2.0;
const CARD_RADIUS: f32 = 10.0;
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
    let open = create_signal(true);
    let selected = create_signal(None::<usize>);
    let stage = NodeRef::new();
    editor.content(&stage);
    let fitting =
        editor.keep_fitted(|| Some(Fit::Area(Rect::from_min_size(pos2(0.0, 0.0), WORLD))));

    let chrome = editor.chrome_shown();
    let panel_shown = create_memo(move || chrome.get() && open.get());
    let reopen_shown = create_memo(move || chrome.get() && !open.get());
    let zoom_shown = create_memo(move || chrome.get());
    let layout = DockingLayout::<&'static str>::new();

    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="pan_zoom" size={SplitSize::Fraction(SIDEBAR_SHARE)}>
                <Show condition={panel_shown}>
                    <DockPane id="sidebar" locked=true>
                        <DockTab id=CARDS_TAB title="Cards" on_close={move || open.set(false)}>
                            <Sidebar editor selected />
                        </DockTab>
                    </DockPane>
                </Show>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Canvas">
                        <Stage @node_ref={&stage} editor={editor} selected={selected} />
                    </DockTab>
                </DockPane>
            </DockSplit>
            <Show condition={zoom_shown}>
                <DockBar
                    id=ZOOM
                    parent=STAGE
                    placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::End)}
                >
                    <ZoomBar editor fitting />
                </DockBar>
            </Show>
            <Show condition={reopen_shown}>
                <DockBar
                    id=REOPEN
                    parent=STAGE
                    placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::Start)}
                >
                    <ReopenBar open />
                </DockBar>
            </Show>
        </Docking>
    }
}

#[component]
fn ZoomBar(editor: Editor, fitting: Fitting) -> NodeId {
    let scale = editor.scale();
    let readout = create_memo(move || format!("Zoom {:.0}%", scale.get() * 100.0));
    let zoom_out = move || editor.zoom(1.0 / ZOOM_STEP);
    let zoom_in = move || editor.zoom(ZOOM_STEP);
    let fit = move || fitting.fit();
    view! {
        <DockBarSurface @test_id={"pan_zoom.zoom_bar"}>
            <DockBarGrip />
            <Button
                icon_only=true
                glyph=ICON_ZOOM_OUT
                label="Zoom out"
                on_click={zoom_out}
                @test_id={"pan_zoom.zoom_out"}
            />
            <Text size=TextSize::Sm color=TextColor::Muted @test_id={"pan_zoom.zoom"}>
                {readout}
            </Text>
            <Button
                icon_only=true
                glyph=ICON_ZOOM_IN
                label="Zoom in"
                on_click={zoom_in}
                @test_id={"pan_zoom.zoom_in"}
            />
            <Button
                icon_only=true
                glyph=ICON_SCAN
                label="Fit"
                on_click={fit}
                @test_id={"pan_zoom.fit"}
            />
        </DockBarSurface>
    }
}

#[component]
fn ReopenBar(open: Signal<bool>) -> NodeId {
    let show = move || open.set(true);
    view! {
        <DockBarSurface>
            <DockBarGrip />
            <Button
                icon_only=true
                glyph=ICON_PANEL_LEFT_OPEN
                label="Show the sidebar"
                @test_id={"pan_zoom.show_sidebar"}
                on_click={show}
            />
        </DockBarSurface>
    }
}

#[component]
fn Sidebar(editor: Editor, selected: Signal<Option<usize>>) -> NodeId {
    let chosen = create_memo(move || match selected.get() {
        Some(index) => format!("Selected {}", CARDS[index].name),
        None => "Nothing selected".to_owned(),
    });
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll>
                <Frame padding=PANEL_PADDING>
                    <List spacing=PANEL_SPACING>
                        <Text size=TextSize::Lg @test_id={"pan_zoom.title"}>"Pan and Zoom"</Text>
                        <Text
                            size=TextSize::Sm
                            color=TextColor::Muted
                            @test_id={"pan_zoom.selected"}
                        >
                            {chosen}
                        </Text>
                        <List spacing=Space::Xs>
                            <ForEach keys={card_indices()}>|index: usize|
                                <FocusRow index editor={editor} selected={selected} />
                            </ForEach>
                        </List>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
fn FocusRow(editor: Editor, index: usize, selected: Signal<Option<usize>>) -> NodeId {
    let card: &'static Card = &CARDS[index];
    let focus = move || {
        selected.set(Some(index));
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
fn Stage(editor: Editor, selected: Signal<Option<usize>>) -> NodeId {
    let scale = editor.scale();
    view! {
        <Canvas view={editor.canvas()} @test_id={"pan_zoom.stage"}>
            <ForEach keys={card_indices()}>|index: usize|
                <CardView index scale={scale} selected={selected} />
            </ForEach>
        </Canvas>
    }
}

#[component]
fn CardView(index: usize, scale: ReadSignal<f32>, selected: Signal<Option<usize>>) -> CanvasItem {
    let card: &'static Card = &CARDS[index];
    let chosen = create_memo(move || selected.get() == Some(index));
    let outline = create_memo(move || match chosen.get() {
        true => ColorRole::Accent,
        false => ColorRole::Border,
    });
    let title_size = scaled(scale, CARD_TITLE_SIZE);
    let label_size = scaled(scale, CARD_LABEL_SIZE);
    let title_size = create_memo(move || TextSize::Custom(title_size.get()));
    let label_size = create_memo(move || TextSize::Custom(label_size.get()));
    let padding = scaled(scale, CARD_PADDING);
    let padding = create_memo(move || Padding::all(Space::Custom(padding.get())));
    let spacing = scaled(scale, CARD_SPACING);
    let spacing = create_memo(move || Space::Custom(spacing.get()));
    let outline_width = scaled(scale, CARD_OUTLINE);
    let radius = scaled(scale, CARD_RADIUS);
    let radius = create_memo(move || Radius::Custom(radius.get()));
    let position = format!("{:.0}, {:.0}", card.x, card.y);
    let select = move || selected.set(Some(index));

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
                    border={create_memo(move || Some(Border::new(outline.get()).width(Stroke::Custom(outline_width.get()))))}
                    radius={radius}
                    padding=padding
                >
                    <List spacing={spacing}>
                        <Text size={title_size}>{card.name}</Text>
                        <Text size={label_size} color=TextColor::Muted>{position}</Text>
                    </List>
                </Frame>
            </Interactive>
        </CanvasItem>
    }
}

fn scaled(scale: ReadSignal<f32>, base: f32) -> Memo<f32> {
    create_memo(move || (base * scale.get()).max(1.0))
}
