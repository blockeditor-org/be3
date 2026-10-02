use super::*;
use beui::KeyPress;
use beui::reactive::{
    Dynamic, Grid, GridCell, Interactive, Layers, NodeRef, Overlay, Placement, Shift, Track,
};

const SWATCH: f32 = 72.0;
const BADGE: f32 = 22.0;
const TILE_HEIGHT: f32 = 48.0;
const CARD_HEIGHT: f32 = 96.0;
const PANEL_WIDTH: f32 = 220.0;
const PLANETS: [&str; 8] = [
    "Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune",
];

#[component]
pub(crate) fn FramesPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Fills, outlines and corners" code={vec![FrameSwatches::SOURCE]}>
                <FrameSwatches />
            </Sample>
            <Sample
                title="Padding and alignment"
                code={vec![FrameAlignment::SOURCE, AlignedBox::SOURCE]}
            >
                <FrameAlignment />
            </Sample>
            <Sample title="Size limits and aspect ratio" code={vec![FrameSizes::SOURCE]}>
                <FrameSizes />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn FrameSwatches() -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=ROW_SPACING wrap=true>
            <Frame width=SWATCH height=SWATCH color={theme.accent.clone()} />
            <Frame width=SWATCH height=SWATCH color={theme.accent.clone()} radius=12 />
            <Frame width=SWATCH height=SWATCH radius=36 color={theme.success.clone()} />
            <Frame
                width=SWATCH
                height=SWATCH
                outline={theme.text.clone()}
                outline_width=2.0
                outline_visible=true
                radius=8
            />
            <Frame
                width=SWATCH
                height=SWATCH
                color={theme.surface_raised.clone()}
                outline={theme.accent.clone()}
                outline_width=2.0
                outline_offset=4.0
                outline_visible=true
                radius=8
            />
            <Frame
                width=SWATCH
                height=SWATCH
                color={theme.warning.clone()}
                radius_top_left=Some(36)
                radius_bottom_right=Some(36)
            />
        </List>
    }
}

#[sample]
#[component]
fn FrameAlignment() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=ROW_SPACING>
            <AlignedBox @sizing=ItemSize::Percent(33.0) align=Align::Start />
            <AlignedBox @sizing=ItemSize::Percent(33.0) align=Align::Center />
            <AlignedBox @sizing=ItemSize::Percent(33.0) align=Align::End />
        </List>
    }
}

#[sample]
#[component]
fn AlignedBox(align: Align) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            height=SWATCH
            color={theme.surface_raised.clone()}
            radius=RADIUS
            padding_horizontal=8.0
            padding_vertical=8.0
            align_horizontal=align
            align_vertical=align
        >
            <Frame width=BADGE height=BADGE color={theme.accent.clone()} radius=4 />
        </Frame>
    }
}

#[sample]
#[component]
fn FrameSizes() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=SECTION_SPACING>
            <Frame
                max_width=Some(320.0)
                color={theme.surface_raised.clone()}
                radius=RADIUS
                padding_horizontal=12.0
                padding_vertical=8.0
            >
                <Text
                    string="This frame stops growing at 320 points wide."
                    color={theme.text.clone()}
                    wrap=true
                />
            </Frame>
            <Frame
                width=Some(160.0)
                aspect_ratio=Some(16.0 / 9.0)
                color={theme.accent_soft.clone()}
                radius=RADIUS
            />
        </List>
    }
}

#[component]
pub(crate) fn TextNodesPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Styles" code={vec![TextStyles::SOURCE]}>
                <TextStyles />
            </Sample>
            <Sample title="Wrapping and ellipsis" code={vec![TextWrapping::SOURCE]}>
                <TextWrapping />
            </Sample>
            <Sample title="Spans" code={vec![TextRuns::SOURCE]}>
                <TextRuns />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn TextStyles() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=SECTION_SPACING>
            <Text string="Plain text" color={theme.text.clone()} />
            <Text string="Bold text" color={theme.text.clone()} bold=true />
            <Text string="Italic text" color={theme.text.clone()} italic=true />
            <Text string="Underlined text" color={theme.text.clone()} underline=true />
            <Text string="Monospace text" color={theme.text.clone()} monospace=true />
            <Text string="Large accent text" font_size=24.0 color={theme.accent.clone()} />
            <Text
                string={ICON_STAR.to_owned()}
                font_size=24.0
                color={theme.warning.clone()}
                icon=true
            />
        </List>
    }
}

#[sample]
#[component]
fn TextWrapping() -> NodeId {
    let theme = use_theme();
    let long = "A text node lays out on one line unless it wraps, and a line that does not \
                fit can end in an ellipsis instead of spilling out of its frame.";
    view! {
        <List spacing=SECTION_SPACING>
            <Text string={long.to_owned()} color={theme.text.clone()} wrap=true />
            <Text string={long.to_owned()} color={theme.text_muted.clone()} ellipsis=true />
            <Text string="Centered" color={theme.text.clone()} align=TextAlign::Center />
            <Text string="At the end" color={theme.text.clone()} align=TextAlign::End />
        </List>
    }
}

#[sample]
#[component]
fn TextRuns() -> NodeId {
    let theme = use_theme();
    let pieces = [
        ("Spans give ", FontId::proportional(14.0), false),
        ("color", FontId::proportional(14.0), true),
        (", ", FontId::proportional(14.0), false),
        ("size", FontId::proportional(22.0), true),
        (" and ", FontId::proportional(14.0), false),
        ("font", FontId::monospace(14.0), true),
        (
            " to ranges of one string.",
            FontId::proportional(14.0),
            false,
        ),
    ];
    let string: String = pieces.iter().map(|(text, _, _)| *text).collect();
    let spans = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        let mut start = 0;
        pieces
            .iter()
            .map(|(text, font, marked)| {
                let color = match marked {
                    true => theme.accent,
                    false => theme.text,
                };
                let span = TextSpan::text(start..start + text.len(), SpanStyle::new(*font, color));
                start += text.len();
                span
            })
            .collect::<Vec<TextSpan>>()
    }));
    view! {
        <Text string={string} spans={spans} color={theme.text.clone()} wrap=true />
    }
}

#[component]
pub(crate) fn ListsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Direction, spacing and spacers"
                code={vec![ListDirections::SOURCE, Tile::SOURCE]}
            >
                <ListDirections />
            </Sample>
            <Sample title="Justify and wrap" code={vec![ListJustify::SOURCE, Tile::SOURCE]}>
                <ListJustify />
            </Sample>
            <Sample title="Grid" code={vec![TrackGrid::SOURCE, Tile::SOURCE]}>
                <TrackGrid />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn Tile(label: Prop<String>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            height=TILE_HEIGHT
            color={theme.surface_raised.clone()}
            radius=RADIUS
            padding_horizontal=12.0
            align_vertical=Align::Center
        >
            <Text string={label} color={theme.text.clone()} />
        </Frame>
    }
}

#[sample]
#[component]
fn ListDirections() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=8.0>
                <Tile label="One" />
                <Tile label="Two" />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Tile label="Pushed to the end" />
            </List>
            <List direction=Direction::Horizontal spacing=8.0>
                <Tile @sizing=ItemSize::Percent(25.0) label="25%" />
                <Tile @sizing=ItemSize::Percent(75.0) label="75%" />
            </List>
            <List direction=Direction::Horizontal spacing=8.0>
                <Tile @sizing=ItemSize::Fixed(120.0) label="120 wide" />
                <Tile @sizing=ItemSize::Percent(100.0) label="The rest" />
            </List>
        </List>
    }
}

#[sample]
#[component]
fn ListJustify() -> NodeId {
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal justify=Justify::Start spacing=8.0>
                <Tile label="Start" />
                <Tile label="B" />
            </List>
            <List direction=Direction::Horizontal justify=Justify::Center spacing=8.0>
                <Tile label="Center" />
                <Tile label="B" />
            </List>
            <List direction=Direction::Horizontal justify=Justify::End spacing=8.0>
                <Tile label="End" />
                <Tile label="B" />
            </List>
            <List direction=Direction::Horizontal spacing=8.0 wrap=true>
                <ForEach keys={PLANETS.to_vec()}>
                    {move |planet: &'static str| view! {
                        <Tile label={planet.to_owned()} />
                    }}
                </ForEach>
            </List>
        </List>
    }
}

#[sample]
#[component]
fn TrackGrid() -> NodeId {
    view! {
        <Grid
            columns={vec![Track::Fixed(90.0), Track::Fraction(1.0), Track::Fraction(2.0)]}
            column_spacing=8.0
            row_spacing=8.0
        >
            <Tile label="Fixed" />
            <Tile label="1 part" />
            <Tile label="2 parts" />
            <GridCell span=2>
                <Tile label="Spans two columns" />
            </GridCell>
            <Tile label="One" />
        </Grid>
    }
}

#[component]
pub(crate) fn ControlFlowPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Show" code={vec![ShowToggle::SOURCE, Tile::SOURCE]}>
                <ShowToggle />
            </Sample>
            <Sample title="ForEach" code={vec![PlanetList::SOURCE, Tile::SOURCE]}>
                <PlanetList />
            </Sample>
            <Sample title="Dynamic" code={vec![DynamicShape::SOURCE]}>
                <DynamicShape />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ShowToggle() -> NodeId {
    let (shown, set_shown) = create_signal(true);
    view! {
        <List spacing=SECTION_SPACING>
            <LabelledSwitch
                label="Shown"
                on={shown.clone()}
                on_change={move |on| set_shown.set(on)}
            />
            <Show condition={shown}>
                <Tile label="Built the first time it is shown, and kept while hidden" />
            </Show>
        </List>
    }
}

#[sample]
#[component]
fn PlanetList() -> NodeId {
    let (count, set_count) = create_signal(3usize);
    let planets = create_memo(clone!(count -> move || PLANETS[..count.get()].to_vec()));
    let fewer = set_count.clone();
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    label="Remove one"
                    variant=ButtonVariant::Secondary
                    on_click={move || fewer.update(|count| *count = count.saturating_sub(1))}
                />
                <Button
                    label="Add one"
                    variant=ButtonVariant::Secondary
                    on_click={move || set_count.update(|count| *count = (*count + 1).min(PLANETS.len()))}
                />
            </List>
            <ForEach keys={planets}>
                {move |planet: &'static str| view! {
                    <Tile label={planet.to_owned()} />
                }}
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn DynamicShape() -> NodeId {
    let theme = use_theme();
    let (round, set_round) = create_signal(false);
    view! {
        <List spacing=SECTION_SPACING>
            <LabelledSwitch
                label="Round"
                on={round.clone()}
                on_change={move |on| set_round.set(on)}
            />
            <Dynamic value={round}>
                {move |round: bool| match round {
                    true => view! {
                        <Frame width=SWATCH height=SWATCH radius=36 color={theme.accent.clone()} />
                    },
                    false => view! {
                        <Text
                            string="Rebuilt whenever its value changes"
                            color={theme.text.clone()}
                        />
                    },
                }}
            </Dynamic>
        </List>
    }
}

#[component]
pub(crate) fn InteractionPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Interactive" code={vec![InteractiveCard::SOURCE]}>
                <InteractiveCard />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn InteractiveCard() -> NodeId {
    let theme = use_theme();
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (log, set_log) = create_signal("Hover, press, or Tab to it and type".to_owned());
    let clicked = set_log.clone();
    let fill = create_memo(clone!(theme hovered active -> move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    }));
    let state = create_memo(clone!(hovered active focused -> move || {
        format!(
            "hovered: {}, active: {}, focused: {}",
            hovered.get(),
            active.get(),
            focused.get()
        )
    }));
    view! {
        <List spacing=SECTION_SPACING>
            <Interactive
                focusable=true
                on_hover_change={move |on| set_hovered.set(on)}
                on_active_change={move |on| set_active.set(on)}
                on_focus_change={move |on| set_focused.set(on)}
                on_click={move || clicked.set("Clicked".to_owned())}
                on_key={move |press: KeyPress| {
                    if press.pressed {
                        set_log.set(format!("Pressed {:?}", press.key));
                    }
                    false
                }}
            >
                <Frame
                    height=CARD_HEIGHT
                    color={fill}
                    outline={theme.accent.clone()}
                    outline_width=2.0
                    outline_visible={focus_ring(focused)}
                    radius=CARD_RADIUS
                    align_horizontal=Align::Center
                    align_vertical=Align::Center
                >
                    <Text string={log} color={theme.text.clone()} />
                </Frame>
            </Interactive>
            <Text string={state} color={theme.text_muted.clone()} monospace=true />
        </List>
    }
}

#[component]
pub(crate) fn LayeringPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Layers" code={vec![BadgedCard::SOURCE]}>
                <BadgedCard />
            </Sample>
            <Sample title="Overlay" code={vec![AnchoredOverlay::SOURCE]}>
                <AnchoredOverlay />
            </Sample>
            <Sample title="Shift" code={vec![NudgedTile::SOURCE]}>
                <NudgedTile />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn BadgedCard() -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Layers>
                <Frame
                    width=Some(200.0)
                    height=CARD_HEIGHT
                    color={theme.surface_raised.clone()}
                    radius=CARD_RADIUS
                    padding_horizontal=12.0
                    padding_vertical=12.0
                >
                    <Text string="Inbox" color={theme.text.clone()} />
                </Frame>
                <Frame
                    align_horizontal=Align::End
                    align_vertical=Align::Start
                    padding_horizontal=8.0
                    padding_vertical=8.0
                >
                    <Frame
                        width=BADGE
                        height=BADGE
                        radius=11
                        color={theme.danger.clone()}
                        align_horizontal=Align::Center
                        align_vertical=Align::Center
                    >
                        <Text string="3" font_size=12.0 color={theme.on_accent.clone()} bold=true />
                    </Frame>
                </Frame>
            </Layers>
        </List>
    }
}

#[sample]
#[component]
fn NudgedTile() -> NodeId {
    let theme = use_theme();
    let (nudged, set_nudged) = create_signal(false);
    let by = create_memo(move || match nudged.get() {
        true => beui::vec2(SWATCH, 0.0),
        false => beui::Vec2::ZERO,
    });
    let face = theme.surface_raised.clone();
    let ink = theme.text.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive on_click={move || set_nudged.update(|nudged| *nudged = !*nudged)}>
                <Frame color={face} radius=RADIUS padding_horizontal=16.0 padding_vertical=10.0>
                    <Text string="Nudge" color={ink} />
                </Frame>
            </Interactive>
            <Shift by={by}>
                <Frame width=SWATCH height=SWATCH color={theme.accent.clone()} radius=12 />
            </Shift>
        </List>
    }
}

#[sample]
#[component]
fn AnchoredOverlay() -> NodeId {
    let theme = use_theme();
    let anchor = NodeRef::new();
    let (open, set_open) = create_signal(false);
    let dismiss = set_open.clone();
    let target = anchor.clone();
    let face = theme.surface_raised.clone();
    let ink = theme.text.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Interactive
                @test_id="demo.overlay.trigger"
                on_click={move || set_open.update(|open| *open = !*open)}
            >
                <Frame
                    @node_ref=&target
                    color={face}
                    radius=RADIUS
                    padding_horizontal=16.0
                    padding_vertical=10.0
                >
                    <Text string="Open the overlay" color={ink} />
                </Frame>
            </Interactive>
            <Overlay
                anchor=&anchor
                placement=Placement::BelowStart
                open={open}
                on_dismiss={move || dismiss.set(false)}
            >
                <Frame padding_vertical=4.0>
                    <Frame
                        @test_id="demo.overlay.panel"
                        width=PANEL_WIDTH
                        color={theme.surface_raised.clone()}
                        outline={theme.border.clone()}
                        outline_width=1.0
                        outline_visible=true
                        radius=CARD_RADIUS
                        padding_horizontal=12.0
                        padding_vertical=12.0
                    >
                        <Text
                            string="An overlay floats above the document, placed against its anchor. Escape or a click outside dismisses it."
                            color={theme.text.clone()}
                            wrap=true
                        />
                    </Frame>
                </Frame>
            </Overlay>
        </List>
    }
}
