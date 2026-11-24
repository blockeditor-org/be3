use be_sdk_beui::Fit;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{DockBar, DockPane, DockSplit, DockTab, Docking, SplitSize};
use be_sdk_beui::beui::icons::ICON_PANEL_LEFT_OPEN;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{CursorIcon, NodeId, Rect, Vec2, pos2, vec2};
use be_sdk_beui::{Editor, ZoomBar};

const STAGE: &str = "stage";
const CARDS_TAB: &str = "cards";
const ZOOM: &str = "zoom";
const REOPEN: &str = "reopen";
const SIDEBAR_WIDTH: f32 = 220.0;
const PANEL_SPACING: Space = Space::Md;
const CARD_TITLE_SIZE: f32 = 17.0;
const CARD_LABEL_SIZE: f32 = 11.0;
const CARD_PADDING: f32 = 12.0;
const CARD_SPACING: f32 = 6.0;
const CARD_OUTLINE: f32 = 2.0;
const CARD_RADIUS: f32 = 10.0;

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
    editor.keep_fitted(|| Some(Fit::Area(Rect::from_min_size(pos2(0.0, 0.0), WORLD))));

    let chrome = editor.chrome_shown();

    view! {
        <Docking>
            <DockSplit size={SplitSize::First(SIDEBAR_WIDTH)}>
                <Show value={open}>
                    <DockPane id="sidebar" chrome=true>
                        <DockTab id=CARDS_TAB title="Cards" on_close={move || open.set(false)}>
                            <Sidebar editor selected />
                        </DockTab>
                    </DockPane>
                </Show>
                <DockPane stage=true>
                    <DockTab id=STAGE>
                        <Stage @node_ref={&stage} editor={editor} selected={selected} />
                    </DockTab>
                </DockPane>
            </DockSplit>
            <Show value={chrome}>
                <DockBar
                    id=ZOOM
                    parent=STAGE
                    placement={DockBarPlacement::Over(DockEdge::Top, DockAlign::End)}
                >
                    <ZoomBar editor />
                </DockBar>
            </Show>
            <Show value={move || chrome.get() && !open.get()}>
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
        <List spacing=PANEL_SPACING>
            <Text size=TextSize::Lg @test_id={"pan_zoom.title"}>"Pan and Zoom"</Text>
            <Text size=TextSize::Sm color=TextColor::Muted @test_id={"pan_zoom.selected"}>
                {chosen}
            </Text>
            <List spacing=Space::Xs>
                <ForEach value={card_indices()}>|index: usize|
                    <FocusRow index editor={editor} selected={selected} />
                </ForEach>
            </List>
        </List>
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
            <ForEach value={card_indices()}>|index: usize|
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
    let padding = scaled(scale, CARD_PADDING);
    let spacing = scaled(scale, CARD_SPACING);
    let outline_width = scaled(scale, CARD_OUTLINE);
    let radius = scaled(scale, CARD_RADIUS);
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
                    fill=ColorRole::Control
                    border={move || Some(Border::new(outline.get()).width(Stroke::Custom(outline_width.get())))}
                    radius={move || Radius::Custom(radius.get())}
                    padding={move || Padding::all(Space::Custom(padding.get()))}
                >
                    <List spacing={move || Space::Custom(spacing.get())}>
                        <Text size={move || TextSize::Custom(title_size.get())}>{card.name}</Text>
                        <Text
                            size={move || TextSize::Custom(label_size.get())}
                            color=TextColor::Muted
                        >
                            {position}
                        </Text>
                    </List>
                </Frame>
            </Interactive>
        </CanvasItem>
    }
}

fn scaled(scale: ReadSignal<f32>, base: f32) -> Memo<f32> {
    create_memo(move || (base * scale.get()).max(1.0))
}
