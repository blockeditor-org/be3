use super::*;
use beui::KeyPress;
use beui::Sides;
use beui::Vec2;
use beui::primitives::Frame;
use beui::primitives::Icon;
use beui::primitives::Span;
use beui::primitives::Text;

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
            <Sample title="Animation" code={vec![AnimationSample::SOURCE]}>
                <AnimationSample />
            </Sample>
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
            <Frame width=SWATCH height=SWATCH fill={theme.accent} />
            <Frame width=SWATCH height=SWATCH fill={theme.accent} radius=12.0 />
            <Frame width=SWATCH height=SWATCH radius=36.0 fill={theme.success} />
            <Frame
                width=SWATCH
                height=SWATCH
                outline={create_memo(move || Some(Outline::new(theme.text.get(), 2.0)))}
                radius=8.0
            />
            <Frame
                width=SWATCH
                height=SWATCH
                fill={theme.surface_raised}
                outline={create_memo(move || Some(Outline::new(theme.accent.get(), 2.0).offset(4.0)))}
                radius=8.0
            />
            <Frame
                width=SWATCH
                height=SWATCH
                fill={theme.warning}
                radius={Corners::new(36.0, 0.0, 36.0, 0.0)}
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
            fill={theme.surface_raised}
            radius={Radius::Md.px()}
            padding=8.0
            align=align
        >
            <Frame width=BADGE height=BADGE fill={theme.accent} radius=4.0 />
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
                width={Extent::at_most(320.0)}
                fill={theme.surface_raised}
                radius={Radius::Md.px()}
                padding={Sides::symmetric(12.0, 8.0)}
            >
                <Text
                    string="This frame stops growing at 320 points wide."
                    color={theme.text}
                    wrap=true
                />
            </Frame>
            <Frame
                width=160.0
                aspect_ratio=Some(16.0 / 9.0)
                fill={theme.accent_soft}
                radius={Radius::Md.px()}
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
            <Sample title="Icons" code={vec![TextIcons::SOURCE]}>
                <TextIcons />
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
            <Text string="Plain text" color={theme.text} />
            <Text
                string="Bold text"
                color={theme.text}
                font={FontId::proportional(14.0).bold(true)}
            />
            <Text
                string="Italic text"
                color={theme.text}
                font={FontId::proportional(14.0).italic(true)}
            />
            <Text string="Underlined text" color={theme.text} underline=true />
            <Text string="Monospace text" color={theme.text} font={FontId::monospace(14.0)} />
            <Text
                string="Large accent text"
                color={theme.accent}
                font={FontId::proportional(24.0)}
            />
            <Text color={theme.warning} font={FontId::proportional(24.0)}>
                <Icon glyph={ICON_STAR.to_owned()} />
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn TextIcons() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=SECTION_SPACING>
            <Text color={theme.text}>
                <Icon glyph=ICON_STAR color={theme.warning} spacing_em=0.25 />
                "An icon is a span that holds no bytes"
            </Text>
            <Text color={theme.text} font={FontId::proportional(20.0)} wrap=true>
                "It takes its size from the text"
                <Icon glyph=ICON_STAR scale=1.25 spacing_em=0.25 />
                "scaled, and its room from both sides"
            </Text>
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
            <Text string={long.to_owned()} color={theme.text} wrap=true />
            <Text string={long.to_owned()} color={theme.text_muted} ellipsis=true />
            <Text string="Centered" color={theme.text} align=TextAlign::Center />
            <Text string="At the end" color={theme.text} align=TextAlign::End />
        </List>
    }
}

#[sample]
#[component]
fn TextRuns() -> NodeId {
    let theme = use_theme();
    view! {
        <Text color={theme.text} wrap=true>
            "Spans give "
            <Span color={theme.accent}>"color"</Span>
            ", "
            <Span font=FontId::proportional(22.0) color={theme.accent}>"size"</Span>
            " and "
            <Span font=FontId::monospace(14.0) color={theme.accent}>"font"</Span>
            " to the parts of one text, and "
            <Span underline=true>"underline"</Span>
            " or "
            <Span strikethrough=true>"strike"</Span>
            " them, and a gap"
            <Gap width=24.0 />
            "leaves room between them."
        </Text>
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
            <Sample title="LayoutBuilder" code={vec![ColumnsForWidth::SOURCE, Tile::SOURCE]}>
                <ColumnsForWidth />
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
            height={Extent::at_least(TILE_HEIGHT)}
            fill={theme.surface_raised}
            radius={Radius::Md.px()}
            padding={Sides::horizontal(12.0)}
            align={Align2::vertical(Align::Center)}
        >
            <Text string={label} color={theme.text} wrap=true />
        </Frame>
    }
}

#[sample]
#[component]
fn ColumnsForWidth() -> NodeId {
    view! {
        <LayoutBuilder>|size: ReadSignal<Vec2>| {
            let wide = create_memo(move || size.get().x >= 480.0);
            let direction = create_memo(move || match wide.get() {
                true => Direction::Horizontal,
                false => Direction::Vertical,
            });
            let label = create_memo(move || format!("{:.0} wide", size.get().x));
            view! {
                <List direction spacing=8.0>
                    <Tile @sizing=ItemSize::Percent(100.0) label />
                    <Tile @sizing=ItemSize::Percent(100.0) label="Side by side from 480" />
                </List>
            }
        }</LayoutBuilder>
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
                <ForEach keys={PLANETS.to_vec()}>|planet: &'static str|
                    <Tile label={planet.to_owned()} />
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
    let shown = create_signal(true);
    view! {
        <List spacing=SECTION_SPACING>
            <LabelledSwitch label="Shown" on={shown} />
            <Show condition={shown}>
                <Tile label="Built the first time it is shown, and kept while hidden" />
            </Show>
        </List>
    }
}

#[sample]
#[component]
fn PlanetList() -> NodeId {
    let count = create_signal(3usize);
    let planets = create_memo(move || PLANETS[..count.get()].to_vec());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    label="Remove one"
                    variant=ButtonVariant::Secondary
                    on_click={move || count.update(|count| *count = count.saturating_sub(1))}
                />
                <Button
                    label="Add one"
                    variant=ButtonVariant::Secondary
                    on_click={move || count.update(|count| *count = (*count + 1).min(PLANETS.len()))}
                />
            </List>
            <ForEach keys={planets}>|planet: &'static str|
                <Tile label={planet.to_owned()} />
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn DynamicShape() -> NodeId {
    let theme = use_theme();
    let round = create_signal(false);
    view! {
        <List spacing=SECTION_SPACING>
            <LabelledSwitch label="Round" on={round} />
            <Dynamic value={round}>|round: bool| {
                match round {
                    true => view! {
                        <Frame width=SWATCH height=SWATCH radius=36.0 fill={theme.accent} />
                    },
                    false => view! {
                        <Text string="Rebuilt whenever its value changes" color={theme.text} />
                    },
                }
            }</Dynamic>
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
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let log = create_signal("Hover, press, or Tab to it and type".to_owned());
    let fill = create_memo(move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    });
    let state = create_memo(move || {
        format!(
            "hovered: {}, active: {}, focused: {}",
            hovered.get(),
            active.get(),
            focused.get()
        )
    });
    view! {
        <List spacing=SECTION_SPACING>
            <Interactive
                focusable=true
                on_hover_change={move |on| hovered.set(on)}
                on_active_change={move |on| active.set(on)}
                on_focus_change={move |on| focused.set(on)}
                on_click={move || log.set("Clicked".to_owned())}
                on_key={move |press: KeyPress| {
                    if press.pressed {
                        log.set(format!("Pressed {:?}", press.key));
                    }
                    false
                }}
            >
                <Frame
                    height=CARD_HEIGHT
                    fill=fill
                    outline={focus_outline(focused, theme.accent, 2.0, 0.0)}
                    radius={Radius::Lg.px()}
                    align=Align::Center
                >
                    <Text string={log} color={theme.text} />
                </Frame>
            </Interactive>
            <Text
                string={state}
                color={theme.text_muted}
                wrap=true
                font={FontId::monospace(14.0)}
            />
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
            <Sample title="Turn" code={vec![TurnedLabel::SOURCE]}>
                <TurnedLabel />
            </Sample>
            <Sample title="Fade" code={vec![FadedStrip::SOURCE]}>
                <FadedStrip />
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
                    width=200.0
                    height=CARD_HEIGHT
                    fill={theme.surface_raised}
                    radius={Radius::Lg.px()}
                    padding=12.0
                >
                    <Text string="Inbox" color={theme.text} />
                </Frame>
                <Frame align={Align2::new(Align::End, Align::Start)} padding=8.0>
                    <Frame
                        width=BADGE
                        height=BADGE
                        radius=11.0
                        fill={theme.danger}
                        align=Align::Center
                    >
                        <Text
                            string="3"
                            color={theme.on_accent}
                            font={FontId::proportional(12.0).bold(true)}
                        />
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
    let nudged = create_signal(false);
    let by = create_memo(move || match nudged.get() {
        true => beui::vec2(SWATCH, 0.0),
        false => beui::Vec2::ZERO,
    });
    let face = theme.surface_raised;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive on_click={move || nudged.update(|nudged| *nudged = !*nudged)}>
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text string="Nudge" color={ink} />
                </Frame>
            </Interactive>
            <Shift by={by}>
                <Frame width=SWATCH height=SWATCH fill={theme.accent} radius=12.0 />
            </Shift>
        </List>
    }
}

#[sample]
#[component]
fn TurnedLabel() -> NodeId {
    let theme = use_theme();
    let quarters = create_signal(1u8);
    let face = theme.surface_raised;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive
                on_click={move || quarters.update(|quarters| *quarters = (*quarters + 1) % 4)}
            >
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text string="Turn" color={ink} />
                </Frame>
            </Interactive>
            <Turn quarter_turns={quarters}>
                <Frame
                    fill={theme.accent}
                    radius={Radius::Md.px()}
                    padding={Sides::symmetric(16.0, 10.0)}
                >
                    <Text string="Upright at no turns" color={theme.on_accent} />
                </Frame>
            </Turn>
        </List>
    }
}

#[sample]
#[component]
fn FadedStrip() -> NodeId {
    let edges = Sides::symmetric(48.0, 0.0);
    view! {
        <Fade edges>
            <Scroll direction=Direction::Horizontal>
                <List direction=Direction::Horizontal spacing=8.0>
                    <ForEach keys={PLANETS.to_vec()}>|planet: &'static str|
                        <Tile label={planet.to_owned()} />
                    </ForEach>
                </List>
            </Scroll>
        </Fade>
    }
}

#[sample]
#[component]
fn AnchoredOverlay() -> NodeId {
    let theme = use_theme();
    let anchor = NodeRef::new();
    let open = create_signal(false);
    let face = theme.surface_raised;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Interactive
                @test_id="demo.overlay.trigger"
                on_click={move || open.update(|open| *open = !*open)}
            >
                <Frame
                    @node_ref=&anchor
                    fill=face
                    radius={Radius::Md.px()}
                    padding={Sides::symmetric(16.0, 10.0)}
                >
                    <Text string="Open the overlay" color={ink} />
                </Frame>
            </Interactive>
            <Overlay
                anchor=anchor
                placement=Placement::BelowStart
                open={open}
                on_dismiss={move |_: Dismiss| open.set(false)}
            >
                <Frame padding={Sides::vertical(4.0)}>
                    <Frame
                        @test_id="demo.overlay.panel"
                        width=PANEL_WIDTH
                        fill={theme.surface_raised}
                        outline={create_memo(move || Some(Outline::new(theme.border.get(), 1.0)))}
                        radius={Radius::Lg.px()}
                        padding=12.0
                    >
                        <Text
                            string="An overlay floats above the document, placed against its anchor. Escape or a click outside dismisses it."
                            color={theme.text}
                            wrap=true
                        />
                    </Frame>
                </Frame>
            </Overlay>
        </List>
    }
}

#[sample]
#[component]
fn AnimationSample() -> NodeId {
    let theme = use_theme();
    let position = create_signal(0.0_f32);
    let spring_position = create_signal(0.0_f32);
    let target = StoredCell::new(0.0_f32);
    let from = StoredCell::new(0.0_f32);
    let velocity = StoredCell::new(0.0_f32);
    let transition = create_animation(
        move |frame| {
            let progress = tween(frame.elapsed.as_secs_f32(), 0.5, Easing::OutCubic);
            position.set(from.get() + (target.get() - from.get()) * progress);
            progress < 1.0
        },
        move || position.set(target.get()),
    );
    let spring = create_animation(
        move |frame| {
            let mut offset = spring_position.get_untracked() - target.get();
            let mut speed = velocity.get();
            Spring {
                stiffness: 180.0,
                damping: 24.0,
            }
            .advance(&mut offset, &mut speed, frame.delta.as_secs_f32());
            velocity.set(speed);
            spring_position.set(target.get() + offset);
            offset != 0.0 || speed != 0.0
        },
        move || {
            velocity.set(0.0);
            spring_position.set(target.get());
        },
    );
    let preference = with_document(|document| document.watch_motion());
    view! {
        <List spacing=SECTION_SPACING>
            <Checkbox
                @test_id="animation.instant"
                label="Disable animations"
                checked={Binding::new(create_memo(move || !preference.get().animates()), move |disabled| with_document(|document| {
                    if let Some(context) = document.context() {
                        context.set_motion(if disabled { Motion::Instant } else { Motion::Animated });
                    }
                }))}
            />
            <Button
                @test_id="animation.retarget"
                label="Move or retarget"
                variant=ButtonVariant::Secondary
                on_click={move || {
                    from.set(position.get_untracked());
                    target.set(if target.get() == 0.0 { 220.0 } else { 0.0 });
                    transition.stop();
                    transition.start();
                    spring.start();
                }}
            />
            <Button
                @test_id="animation.stop"
                label="Stop"
                variant=ButtonVariant::Secondary
                on_click={move || { transition.stop(); spring.stop(); }}
            />
            <Text string="Tween" color={theme.text} />
            <Frame
                width={create_memo(move || position.get() + 20.0)}
                height=12.0
                fill={theme.accent}
            />
            <Text string="Spring" color={theme.text} />
            <Frame
                width={create_memo(move || spring_position.get() + 20.0)}
                height=12.0
                fill={theme.success}
            />
        </List>
    }
}
