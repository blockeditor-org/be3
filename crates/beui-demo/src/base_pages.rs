use super::*;
use beui::KeyPress;
use beui::Sides;
use beui::Vec2;
use beui::primitives::Drawing;
use beui::primitives::Frame;
use beui::primitives::Gap;
use beui::primitives::Icon;
use beui::primitives::Span;
use beui::primitives::Text;
use beui::primitives::{Grid, List};
use beui::reactive::Painter;
use beui::styled::Glide;
use beui::unstyled::FirstFit;
use beui::{
    Align, Align2, Corners, Dismiss, Extent, Justify, LayerPolicy, Outline, Placement, Track,
};
use beui::{Color, Pos2, vec2};
use beui::{FillRule, LineCap, LineJoin, Path, Stroke};
use beui::{Overflow, TextLayout, WrapAt};
use beui::{Window, WindowHandle};
use beui::{WindowPlacement, WindowRole};

const SWATCH: f32 = 72.0;
const BADGE: f32 = 22.0;
const TILE_HEIGHT: f32 = 48.0;
const CARD_HEIGHT: f32 = 96.0;
const PANEL_WIDTH: f32 = 220.0;
const WINDOW_HEIGHT: f32 = 120.0;
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
        <List direction=Direction::Horizontal spacing={ROW_SPACING.px()} wrap=true>
            <Frame width=SWATCH height=SWATCH fill={theme.accent} />
            <Frame width=SWATCH height=SWATCH fill={theme.accent} radius=12.0 />
            <Frame width=SWATCH height=SWATCH radius=36.0 fill={theme.success} />
            <Frame
                width=SWATCH
                height=SWATCH
                outline={move || Some(Outline::new(theme.text.get(), 2.0))}
                radius=8.0
            />
            <Frame
                width=SWATCH
                height=SWATCH
                fill={theme.control}
                outline={move || Some(Outline::new(theme.accent.get(), 2.0).offset(4.0))}
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
        <List direction=Direction::Horizontal spacing={ROW_SPACING.px()}>
            <ListItem grow=1.0>
                <AlignedBox align=Align::Start />
            </ListItem>
            <ListItem grow=1.0>
                <AlignedBox align=Align::Center />
            </ListItem>
            <ListItem grow=1.0>
                <AlignedBox align=Align::End />
            </ListItem>
        </List>
    }
}

#[sample]
#[component]
fn AlignedBox(align: Align) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame height=SWATCH fill={theme.control} radius={Radius::Md.px()} padding=8.0 align=align>
            <Frame width=BADGE height=BADGE fill={theme.accent} radius=4.0 />
        </Frame>
    }
}

#[sample]
#[component]
fn FrameSizes() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Frame
                width={Extent::at_most(320.0)}
                fill={theme.control}
                radius={Radius::Md.px()}
                padding={Sides::symmetric(12.0, 8.0)}
            >
                <Text color={theme.text}>"This frame stops growing at 320 points wide."</Text>
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
            <Sample title="Line breaks" code={vec![TextBreaks::SOURCE]}>
                <TextBreaks />
            </Sample>
            <Sample title="Icons" code={vec![TextIcons::SOURCE]}>
                <TextIcons />
            </Sample>
            <Sample title="Text in a drawing" code={vec![TextInADrawing::SOURCE]}>
                <TextInADrawing />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn TextStyles() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Text color={theme.text}>"Plain text"</Text>
            <Text color={theme.text} font={FontId::proportional(14.0).bold(true)}>"Bold text"</Text>
            <Text color={theme.text} font={FontId::proportional(14.0).italic(true)}>
                "Italic text"
            </Text>
            <Text color={theme.text} underline=true>"Underlined text"</Text>
            <Text color={theme.text} font={FontId::monospace(14.0)}>"Monospace text"</Text>
            <Text color={theme.accent} font={FontId::proportional(24.0)}>"Large accent text"</Text>
            <Text color={theme.warning} font={FontId::proportional(24.0)}>
                <Icon glyph=ICON_STAR />
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn TextIcons() -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Text color={theme.text}>
                <Icon glyph=ICON_STAR color={theme.warning} spacing_em=0.25 />
                "An icon is a span that holds no bytes"
            </Text>
            <Text color={theme.text} font={FontId::proportional(20.0)}>
                "It takes its size from the text"
                <Icon glyph=ICON_STAR scale=1.25 spacing_em=0.25 />
                "scaled, and its room from both sides"
            </Text>
        </List>
    }
}

#[sample]
#[component]
fn TextInADrawing() -> NodeId {
    let theme = use_theme();
    let font = FontId::proportional(16.0);
    let draw = Prop::dynamic(move || -> Draw {
        let (fill, ink, accent) = (theme.control.get(), theme.text.get(), theme.accent.get());
        Rc::new(move |painter: &Painter, rect: Rect| {
            let label = painter.lay_out_text("Placed at its corner", font, TextLayout::DEFAULT);
            painter.rect_filled(Rect::from_min_size(rect.min, label.size()), 0.0, fill);
            painter.draw_text(rect.min, &label, ink);
            let wrapped = painter.lay_out_text(
                "Measured, wrapped and centred as it is drawn",
                font,
                TextLayout {
                    align: TextAlign::Center,
                    ..TextLayout::wrapped(140.0)
                },
            );
            let at = rect.min + vec2(0.0, label.size().y + 12.0);
            painter.rect_filled(Rect::from_min_size(at, wrapped.size()), 0.0, fill);
            for line in wrapped.lines() {
                painter.line(
                    at + vec2(line.rect.left(), line.baseline),
                    at + vec2(line.rect.right(), line.baseline),
                    1.0,
                    accent,
                );
            }
            painter.draw_text(at, &wrapped, ink);
        })
    });
    view! {
        <List
            @test_id="demo.text.drawn"
            direction=Direction::Horizontal
            align=Align::Start
            spacing={SECTION_SPACING.px()}
        >
            <Frame fill={theme.control}>
                <Text color={theme.text} font>"Placed at its corner"</Text>
            </Frame>
            <Drawing draw size={Some(vec2(200.0, 100.0))} />
        </List>
    }
}

#[sample]
#[component]
fn TextWrapping() -> NodeId {
    let theme = use_theme();
    let wraps = "Text wraps to the width it is given, breaking after spaces and hyphens, \
                 and between letters only when a word is wider than a line.";
    let clamped = "With a maximum number of lines, whatever does not fit on them is left out, \
                   and the last line that shows ends in an ellipsis to say so.";
    let labels = "An ellipsis keeps a label to one line and cuts what does not fit\n\
                  A hard line break still starts a new line, cut on its own";
    let two_lines = Overflow::Wrap {
        at: WrapAt::Words,
        max_lines: Some(2),
    };
    view! {
        <List @test_id="demo.text.wrapping" spacing={SECTION_SPACING.px()}>
            <Frame width=260.0 fill={theme.control}>
                <Text color={theme.text}>{wraps.to_owned()}</Text>
            </Frame>
            <Frame width=260.0 fill={theme.control}>
                <Text color={theme.text} overflow={two_lines}>{clamped.to_owned()}</Text>
            </Frame>
            <Frame width=260.0 fill={theme.control}>
                <Text color={theme.text_muted} overflow=Overflow::Ellipsis>
                    {labels.to_owned()}
                </Text>
            </Frame>
            <Text color={theme.text} align=TextAlign::Center>"Centered"</Text>
            <Text color={theme.text} align=TextAlign::End>"At the end"</Text>
        </List>
    }
}

#[sample]
#[component]
fn TextRuns() -> NodeId {
    let theme = use_theme();
    view! {
        <Text color={theme.text}>
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

#[sample]
#[component]
fn TextBreaks() -> NodeId {
    let theme = use_theme();
    let mono = FontId::monospace(14.0);
    view! {
        <List @test_id="demo.text.breaks" spacing={SECTION_SPACING.px()}>
            <Text color={theme.text} font={mono}>
                "A break ends a line"
                <Break />
                "but holds no bytes,"
                <Break />
                "so a copy reads one line."
            </Text>
            <Frame width=200.0 fill={theme.control}>
                <Text
                    color={theme.text}
                    font={mono}
                    overflow={Overflow::Wrap { at: WrapAt::Anywhere, max_lines: None }}
                >
                    "Breaking anywhere fills each line to its last column: /usr/local/share"
                </Text>
            </Frame>
        </List>
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
            <Sample title="measure" code={vec![ColumnsForWidth::SOURCE, Tile::SOURCE]}>
                <ColumnsForWidth />
            </Sample>
            <Sample title="FirstFit" code={vec![FittingLabels::SOURCE, Tile::SOURCE]}>
                <FittingLabels />
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
            fill={theme.control}
            radius={Radius::Md.px()}
            padding={Sides::horizontal(12.0)}
            align={Align2::vertical(Align::Center)}
        >
            <Text color={theme.text}>{label}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn FittingLabels() -> NodeId {
    view! {
        <List @test_id="demo.lists.first_fit" direction=Direction::Horizontal spacing=8.0>
            <Frame width={Extent::px(PANEL_WIDTH)}>
                <FirstFit>
                    <Tile label="Mercury, Venus, Earth" />
                    <Tile label="Mercury" />
                </FirstFit>
            </Frame>
            <Frame width={Extent::px(PANEL_WIDTH / 2.0)}>
                <FirstFit>
                    <Tile label="Mercury, Venus, Earth" />
                    <Tile label="Mercury" />
                </FirstFit>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn ColumnsForWidth() -> NodeId {
    let width = create_signal(0.0f32);
    let wide = create_memo(move || width.get() >= 480.0);
    let direction = create_memo(move || match wide.get() {
        true => Direction::Horizontal,
        false => Direction::Vertical,
    });
    let label = create_memo(move || format!("{:.0} wide", width.get()));
    let columns = view! {
        <List direction spacing=8.0>
            <ListItem grow=1.0>
                <Tile label />
            </ListItem>
            <ListItem grow=1.0>
                <Tile label="Side by side from 480" />
            </ListItem>
        </List>
    };
    let measured = measure(columns);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            width.set(measured.size.x);
        }
    });
    columns
}

#[sample]
#[component]
fn ListDirections() -> NodeId {
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <List direction=Direction::Horizontal spacing=8.0>
                <Tile label="One" />
                <Tile label="Two" />
                <Spacer />
                <Tile label="Pushed to the end" />
            </List>
            <List direction=Direction::Horizontal spacing=8.0>
                <ListItem grow=0.25>
                    <Tile label="25%" />
                </ListItem>
                <ListItem grow=0.75>
                    <Tile label="75%" />
                </ListItem>
            </List>
            <List direction=Direction::Horizontal spacing=8.0>
                <ListItem length=120.0>
                    <Tile label="120 wide" />
                </ListItem>
                <ListItem grow=1.0>
                    <Tile label="The rest" />
                </ListItem>
            </List>
        </List>
    }
}

#[sample]
#[component]
fn ListJustify() -> NodeId {
    view! {
        <List spacing={SECTION_SPACING.px()}>
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
                <ForEach value={PLANETS.to_vec()}>|planet: &'static str|
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
            <Sample title="Show with a fallback" code={vec![ShowFallback::SOURCE, Tile::SOURCE]}>
                <ShowFallback />
            </Sample>
            <Sample title="Show over an Option" code={vec![ShowPicked::SOURCE, Tile::SOURCE]}>
                <ShowPicked />
            </Sample>
            <Sample title="ForEach" code={vec![PlanetList::SOURCE, Tile::SOURCE]}>
                <PlanetList />
            </Sample>
            <Sample title="ForEach with a key" code={vec![CounterList::SOURCE, Tile::SOURCE]}>
                <CounterList />
            </Sample>
            <Sample title="Match" code={vec![MatchShape::SOURCE]}>
                <MatchShape />
            </Sample>
            <Sample title="Dynamic" code={vec![DynamicCount::SOURCE, Tile::SOURCE]}>
                <DynamicCount />
            </Sample>
            <Sample title="KeepAlive and Kept" code={vec![KeptCounter::SOURCE]}>
                <KeptCounter />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ShowToggle() -> NodeId {
    let shown = create_signal(true);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <LabelledSwitch label="Shown" on={shown} />
            <Show value={shown}>
                <Tile label="Built each time it is shown, and dropped while hidden" />
            </Show>
        </List>
    }
}

#[sample]
#[component]
fn ShowFallback() -> NodeId {
    let editing = create_signal(false);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <LabelledSwitch label="Editing" on={editing} />
            <Frame padding={Sides::all(8.0)}>
                <Show
                    value={editing}
                    fallback={|| view! {
                        <Tile label="Reading" />
                    }}
                >||
                    <Tile label="Editing: one child either way, so a frame can hold it" />
                </Show>
            </Frame>
            <List spacing=8.0>
                <Show
                    value={editing}
                    fallback={|| view! {
                        <Tile label="A run of three" />
                        <Tile label="while reading" />
                        <Tile label="in a list" />
                    }}
                >||
                    <Tile label="A run of two" />
                    <Tile label="while editing" />
                </Show>
            </List>
            <Tile
                label={move || match editing.get() {
                    true => "A closure is a prop: editing".to_owned(),
                    false => "A closure is a prop: reading".to_owned(),
                }}
            />
        </List>
    }
}

#[sample]
#[component]
fn PlanetList() -> NodeId {
    let count = create_signal(3usize);
    let planets = create_memo(move || PLANETS[..count.get()].to_vec());
    view! {
        <List spacing={SECTION_SPACING.px()}>
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
            <ForEach
                value={planets}
                fallback={|| view! {
                    <Tile label="No planets: the fallback shows while the list is empty" />
                }}
            >|planet: &'static str, index: ReadSignal<usize>|
                <Tile label={move || format!("{}. {planet}", index.get() + 1)} />
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn ShowPicked() -> NodeId {
    let picked = create_signal(None::<&'static str>);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <List direction=Direction::Horizontal spacing=8.0>
                <ForEach value={PLANETS[..3].to_vec()}>|planet: &'static str|
                    <Button
                        label={planet.to_owned()}
                        variant=ButtonVariant::Secondary
                        on_click={move || picked.set(Some(planet))}
                    />
                </ForEach>
                <Button
                    label="None"
                    variant=ButtonVariant::Secondary
                    on_click={move || picked.set(None)}
                />
            </List>
            <Show
                value={picked}
                fallback={|| view! {
                    <Tile label="Nothing picked" />
                }}
            >|planet: ReadSignal<&'static str>|
                <Tile label={move || format!("Picked {}: changed in place", planet.get())} />
            </Show>
        </List>
    }
}

#[sample]
#[component]
fn CounterList() -> NodeId {
    #[derive(Clone, PartialEq)]
    struct Counter {
        id: u32,
        name: &'static str,
        count: u32,
    }
    let counters = create_signal(vec![
        Counter {
            id: 1,
            name: "Apples",
            count: 0,
        },
        Counter {
            id: 2,
            name: "Pears",
            count: 0,
        },
    ]);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    label="Count an apple"
                    variant=ButtonVariant::Secondary
                    on_click={move || counters.update(|counters| counters[0].count += 1)}
                />
                <Button
                    label="Swap"
                    variant=ButtonVariant::Secondary
                    on_click={move || counters.update(|counters| counters.swap(0, 1))}
                />
            </List>
            <ForEach
                value={counters}
                key={|counter: &Counter| counter.id}
            >|counter: ReadSignal<Counter>|
                <Tile
                    label={move || counter.with(|counter| format!("{}: {}", counter.name, counter.count))}
                />
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn MatchShape() -> NodeId {
    #[derive(Clone, Copy, PartialEq)]
    enum Shape {
        Caption,
        Square,
        Circle,
        Wide(u8),
    }
    let theme = use_theme();
    let shape = create_signal(Shape::Caption);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Button
                label="Next shape"
                variant=ButtonVariant::Secondary
                on_click={move || shape.update(|shape| {
                    *shape = match shape {
                        Shape::Caption => Shape::Square,
                        Shape::Square => Shape::Circle,
                        Shape::Circle => Shape::Wide(2),
                        Shape::Wide(3) => Shape::Caption,
                        Shape::Wide(wide) => Shape::Wide(*wide + 1),
                    }
                })}
            />
            <Match value={shape}>
                <Case is={Shape::Caption}>
                    <Text color={theme.text}>"The first case that holds shows"</Text>
                </Case>
                <Case is={Shape::Square}>
                    <Frame width=SWATCH height=SWATCH fill={theme.accent} />
                </Case>
                <Case is={Shape::Circle}>
                    <Frame width=SWATCH height=SWATCH radius=36.0 fill={theme.accent} />
                </Case>
                <Case
                    when={|shape: &Shape| matches!(shape, Shape::Wide(_))}
                >|shape: ReadSignal<Shape>|
                    <Frame
                        width={move || match shape.get() {
                            Shape::Wide(wide) => Extent::px(SWATCH * f32::from(wide)),
                            _ => Extent::px(SWATCH),
                        }}
                        height=SWATCH
                        fill={theme.accent}
                    />
                </Case>
            </Match>
        </List>
    }
}

#[sample]
#[component]
fn DynamicCount() -> NodeId {
    let count = create_signal(1u32);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Button
                label="Add one"
                variant=ButtonVariant::Secondary
                on_click={move || count.update(|count| *count += 1)}
            />
            <Dynamic value={count}>|count: u32|
                <Tile label={format!("Built again for {count}")} />
            </Dynamic>
        </List>
    }
}

#[sample]
#[component]
fn KeptCounter() -> NodeId {
    let shown = create_signal(true);
    let right = create_signal(false);
    let left_shown = create_memo(move || shown.get() && !right.get());
    let right_shown = create_memo(move || shown.get() && right.get());
    // Built once, then kept, paused, while hidden, and shown in whichever column holds it.
    let counter = move || {
        let count = create_signal(0u32);
        view! {
            <Button
                label={move || format!("Clicked {} times", count.get())}
                variant=ButtonVariant::Secondary
                on_click={move || count.update(|count| *count += 1)}
            />
        }
    };
    view! {
        <KeepAlive>
            <List spacing={SECTION_SPACING.px()}>
                <LabelledSwitch label="Shown" on={shown} />
                <LabelledSwitch label="In the right column" on={right} />
                <List direction=Direction::Horizontal spacing={SECTION_SPACING.px()}>
                    <Frame width=180.0>
                        <Show value={left_shown}>
                            <Kept>{counter()}</Kept>
                        </Show>
                    </Frame>
                    <Frame width=180.0>
                        <Show value={right_shown}>
                            <Kept>{counter()}</Kept>
                        </Show>
                    </Frame>
                </List>
            </List>
        </KeepAlive>
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
            (false, false) => theme.control,
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
        <List spacing={SECTION_SPACING.px()}>
            <Interactive
                focusable=true
                on_hover_change={hovered}
                on_active_change={active}
                on_focus_change={focused}
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
                    <Text color={theme.text}>{log}</Text>
                </Frame>
            </Interactive>
            <Text color={theme.text_muted} font={FontId::monospace(14.0)}>{state}</Text>
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
            <Sample title="Window" code={vec![FloatingWindow::SOURCE]}>
                <FloatingWindow />
            </Sample>
            <Sample title="Popup window" code={vec![PopupWindow::SOURCE]}>
                <PopupWindow />
            </Sample>
            <Sample title="Translate" code={vec![NudgedTile::SOURCE]}>
                <NudgedTile />
            </Sample>
            <Sample title="Glide" code={vec![GlidingPlanets::SOURCE, Tile::SOURCE]}>
                <GlidingPlanets />
            </Sample>
            <Sample title="Rotate" code={vec![TiltedLabel::SOURCE]}>
                <TiltedLabel />
            </Sample>
            <Sample title="Quarter turns" code={vec![TurnedLabel::SOURCE]}>
                <TurnedLabel />
            </Sample>
            <Sample title="Fade edges" code={vec![FadedStrip::SOURCE]}>
                <FadedStrip />
            </Sample>
            <Sample title="Frame effects" code={vec![FrameEffects::SOURCE]}>
                <FrameEffects />
            </Sample>
            <Sample title="Opacity" code={vec![FadedTiles::SOURCE]}>
                <FadedTiles />
            </Sample>
            <Sample title="Group opacity" code={vec![FadedOverlap::SOURCE]}>
                <FadedOverlap />
            </Sample>
            <Sample title="Placements" code={vec![PlacementSet::SOURCE]}>
                <PlacementSet />
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
                    fill={theme.control}
                    radius={Radius::Lg.px()}
                    padding=12.0
                >
                    <Text color={theme.text}>"Inbox"</Text>
                </Frame>
                <Frame align={Align2::new(Align::End, Align::Start)} padding=8.0>
                    <Frame
                        width=BADGE
                        height=BADGE
                        radius=11.0
                        fill={theme.danger}
                        align=Align::Center
                    >
                        <Text color={theme.on_accent} font={FontId::proportional(12.0).bold(true)}>
                            "3"
                        </Text>
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
    let face = theme.control;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive on_click={move || nudged.update(|nudged| *nudged = !*nudged)}>
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text color={ink}>"Nudge"</Text>
                </Frame>
            </Interactive>
            <Frame translate={by} width=SWATCH height=SWATCH fill={theme.accent} radius=12.0 />
        </List>
    }
}

#[sample]
#[component]
fn TiltedLabel() -> NodeId {
    let theme = use_theme();
    let angle = create_signal(0.25f32);
    let face = theme.control;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive
                on_click={move || angle.update(|angle| *angle = (*angle + 0.25) % std::f32::consts::FRAC_PI_2)}
            >
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text color={ink}>"Tilt"</Text>
                </Frame>
            </Interactive>
            <Frame
                rotate={angle}
                fill={theme.accent}
                radius={Radius::Md.px()}
                padding={Sides::symmetric(16.0, 10.0)}
            >
                <Text color={theme.on_accent}>"Tilted in its own box"</Text>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn GlidingPlanets() -> NodeId {
    let order = create_signal((0..5).collect::<Vec<usize>>());
    let moved = create_trigger();
    let reorder = move |change: fn(&mut Vec<usize>)| {
        order.update(change);
        moved.notify();
    };
    view! {
        <List @test_id="demo.layering.glide" spacing=12.0>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    @test_id="demo.glide.rotate"
                    label="Rotate"
                    variant=ButtonVariant::Secondary
                    on_click={move || reorder(|order| order.rotate_left(1))}
                />
                <Button
                    @test_id="demo.glide.reverse"
                    label="Reverse"
                    variant=ButtonVariant::Secondary
                    on_click={move || reorder(|order| order.reverse())}
                />
            </List>
            <List direction=Direction::Horizontal spacing=8.0>
                <ForEach value={order}>|planet: usize|
                    <Glide on={moved}>
                        <Tile label={PLANETS[planet].to_owned()} />
                    </Glide>
                </ForEach>
            </List>
        </List>
    }
}

#[sample]
#[component]
fn TurnedLabel() -> NodeId {
    let theme = use_theme();
    let quarters = create_signal(1u8);
    let face = theme.control;
    let ink = theme.text;
    let angle = create_memo(move || f32::from(quarters.get()) * std::f32::consts::FRAC_PI_2);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
            <Interactive
                on_click={move || quarters.update(|quarters| *quarters = (*quarters + 1) % 4)}
            >
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text color={ink}>"Turn"</Text>
                </Frame>
            </Interactive>
            <Frame
                width=96.0
                height=96.0
                fill={theme.accent}
                radius={Radius::Md.px()}
                padding={Sides::all(12.0)}
                align=Align2::CENTER
                rotate={angle}
            >
                <Text color={theme.on_accent} align=TextAlign::Center>"Upright at no turns"</Text>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn FadedStrip() -> NodeId {
    let edges = Sides::symmetric(48.0, 0.0);
    view! {
        <Frame fade_edges={edges}>
            <Scroll direction=Direction::Horizontal>
                <List direction=Direction::Horizontal spacing=8.0>
                    <ForEach value={PLANETS.to_vec()}>|planet: &'static str|
                        <Tile label={planet.to_owned()} />
                    </ForEach>
                </List>
            </Scroll>
        </Frame>
    }
}

#[sample]
#[component]
fn FadedTiles() -> NodeId {
    view! {
        <List @test_id="demo.layering.opacity" direction=Direction::Horizontal spacing=8.0>
            <ForEach value={vec![100_u32, 60, 30]}>|percent: u32|
                <Frame opacity={percent as f32 / 100.0}>
                    <Tile label={format!("{percent}%")} />
                </Frame>
            </ForEach>
        </List>
    }
}

/// A frame's opacity fades what it holds as one picture: where the squares
/// overlap, the one below does not show through the one above.
#[sample]
#[component]
fn FadedOverlap() -> NodeId {
    let theme = use_theme();
    let offset = Sides::new(SWATCH / 2.0, SWATCH / 2.0, 0.0, 0.0);
    view! {
        <List @test_id="demo.layering.faded_overlap" direction=Direction::Horizontal spacing=24.0>
            <ForEach value={vec![100_u32, 50]}>|percent: u32|
                <Frame opacity={percent as f32 / 100.0}>
                    <Layers>
                        <Frame
                            width=SWATCH
                            height=SWATCH
                            fill={theme.accent}
                            radius={Radius::Md.px()}
                        />
                        <Frame padding=offset>
                            <Frame
                                width=SWATCH
                                height=SWATCH
                                fill={theme.success}
                                radius={Radius::Md.px()}
                            />
                        </Frame>
                    </Layers>
                </Frame>
            </ForEach>
        </List>
    }
}

#[sample]
#[component]
fn FrameEffects() -> NodeId {
    view! {
        <List
            @test_id="demo.layering.frame_effects"
            direction=Direction::Horizontal
            wrap=true
            align=Align::Center
            spacing=24.0
        >
            <Frame translate={vec2(0.0, 12.0)}>
                <Tile label={"translate".to_owned()} />
            </Frame>
            <Frame rotate=0.3>
                <Tile label={"rotate".to_owned()} />
            </Frame>
            <Frame opacity=0.5>
                <Tile label={"opacity".to_owned()} />
            </Frame>
            <Frame fade_edges={Sides::symmetric(32.0, 0.0)}>
                <Tile label={"fade edges at both sides".to_owned()} />
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn AnchoredOverlay() -> NodeId {
    let theme = use_theme();
    let anchor = NodeRef::new();
    let open = create_signal(false);
    let face = theme.control;
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
                    <Text color={ink}>"Open the overlay"</Text>
                </Frame>
            </Interactive>
            <Overlay
                anchor=anchor
                placement=Placement::BELOW_START
                open={open}
                on_dismiss={move |_: Dismiss| open.set(false)}
            >
                <Frame padding={Sides::vertical(4.0)}>
                    <Frame
                        @test_id="demo.overlay.panel"
                        width=PANEL_WIDTH
                        fill={theme.surface}
                        outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                        radius={Radius::Lg.px()}
                        padding=12.0
                    >
                        <Text color={theme.text}>
                            "An overlay floats above the document, placed against its anchor. Escape or a click outside dismisses it."
                        </Text>
                    </Frame>
                </Frame>
            </Overlay>
        </List>
    }
}

const PLACEMENTS: [(Placement, &str); 12] = [
    (Placement::ABOVE_START, "above start"),
    (Placement::ABOVE_CENTER, "above center"),
    (Placement::ABOVE_END, "above end"),
    (Placement::BELOW_START, "below start"),
    (Placement::BELOW_CENTER, "below center"),
    (Placement::BELOW_END, "below end"),
    (Placement::LEFT_START, "left start"),
    (Placement::LEFT_CENTER, "left center"),
    (Placement::LEFT_END, "left end"),
    (Placement::RIGHT_START, "right start"),
    (Placement::RIGHT_CENTER, "right center"),
    (Placement::RIGHT_END, "right end"),
];

/// Where the corner sample's marker stands, from the window's bottom right corner.
const CORNER_MARKER: Rect = Rect {
    min: Pos2::new(-80.0, -40.0),
    max: Pos2::new(-40.0, -20.0),
};

/// Every side and alignment an overlay can open at beside its anchor, all at once; and near
/// the window's corner, where they flip and shift to stay in it, clear of its edges.
#[sample]
#[component]
fn PlacementSet() -> NodeId {
    let theme = use_theme();
    let anchor = NodeRef::new();
    let open = create_signal(false);
    let corner = create_signal(false);
    let marker = NodeRef::new();
    let viewport = use_viewport_size();
    let marked = create_memo(move || {
        OverlayAnchor::Point(CORNER_MARKER.min + viewport.get())
    });
    let near_corner = [
        (Placement::BELOW_START.gap(4).margin(8), "below start", "demo.corner.below"),
        (Placement::RIGHT_START.gap(4).margin(8), "right start", "demo.corner.right"),
    ];
    let face = theme.control;
    let ink = theme.text;
    view! {
        <Frame padding={Sides::symmetric(120.0, 40.0)}>
            <List direction=Direction::Horizontal spacing=0.0>
            <Interactive
                @test_id="demo.placements.trigger"
                on_click={move || open.update(|open| *open = !*open)}
            >
                <Frame
                    @node_ref=&anchor
                    @test_id="demo.placements.anchor"
                    width=320.0
                    height=110.0
                    fill=face
                    radius={Radius::Md.px()}
                    padding=12.0
                >
                    <Text color={ink}>"Show every placement"</Text>
                </Frame>
            </Interactive>
            <Frame padding={Sides { left: 130.0, top: 0.0, right: 0.0, bottom: 0.0 }}>
                <Interactive
                    @test_id="demo.placements.corner.trigger"
                    on_click={move || corner.update(|corner| *corner = !*corner)}
                >
                    <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                        <Text color={ink}>"Near the corner"</Text>
                    </Frame>
                </Interactive>
            </Frame>
            <Overlay
                anchor={marked}
                placement=Placement::At
                policy=LayerPolicy::Inert
                traps_focus=false
                dismissable=false
                open={corner}
                on_dismiss={|_: Dismiss| {}}
            >
                <Frame
                    @node_ref=&marker
                    @test_id="demo.placements.corner"
                    width={CORNER_MARKER.width()}
                    height={CORNER_MARKER.height()}
                    fill={theme.control}
                    radius={Radius::Sm.px()}
                />
            </Overlay>
            <ForEach value={(0..near_corner.len()).collect::<Vec<usize>>()}>|index: usize|
                <Overlay
                    anchor=marker
                    placement={near_corner[index].0}
                    policy=LayerPolicy::Inert
                    traps_focus=false
                    dismissable=false
                    open={corner}
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame
                        @test_id={near_corner[index].2}
                        width=100.0
                        fill={theme.surface}
                        outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                        radius={Radius::Sm.px()}
                        padding={Sides::symmetric(6.0, 2.0)}
                    >
                        <Text color={theme.text}>{near_corner[index].1}</Text>
                    </Frame>
                </Overlay>
            </ForEach>
            <ForEach value={(0..PLACEMENTS.len()).collect::<Vec<usize>>()}>|index: usize|
                <Overlay
                    anchor=anchor
                    placement={PLACEMENTS[index].0.gap(4)}
                    policy=LayerPolicy::Inert
                    traps_focus=false
                    dismissable=false
                    open={open}
                    on_dismiss={|_: Dismiss| {}}
                >
                    <Frame
                        @test_id={format!("demo.placement.{index}")}
                        fill={theme.surface}
                        outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                        radius={Radius::Sm.px()}
                        padding={Sides::symmetric(6.0, 2.0)}
                    >
                        <Text color={theme.text}>{PLACEMENTS[index].1}</Text>
                    </Frame>
                </Overlay>
            </ForEach>
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn FloatingWindow() -> NodeId {
    let theme = use_theme();
    let open = create_signal(false);
    let at = create_signal(Pos2::ZERO);
    let face = theme.control;
    let ink = theme.text;
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Interactive
                @test_id="demo.window.trigger"
                on_click_at={move |press: beui::PointerPress| {
                    at.set(press.pos + vec2(16.0, 16.0));
                    open.update(|open| *open = !*open);
                }}
            >
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text color={ink}>"Open a window"</Text>
                </Frame>
            </Interactive>
            <Window
                size={vec2(PANEL_WIDTH, WINDOW_HEIGHT)}
                at={at}
                title={Some("A window".to_owned())}
                open={open}
                on_report={move |report: beui::WindowReport| {
                    if matches!(report, beui::WindowReport::Closed(_)) {
                        open.set(false);
                    }
                }}
            >|_: WindowHandle|
                <Frame
                    @test_id="demo.window.panel"
                    fill={theme.surface}
                    outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                    radius={Radius::Lg.px()}
                    padding=12.0
                >
                    <Text color={theme.text}>
                        "A window of its own where a host opens windows, an overlay elsewhere."
                    </Text>
                </Frame>
            </Window>
        </List>
    }
}

#[sample]
#[component]
fn PopupWindow() -> NodeId {
    let theme = use_theme();
    let open = create_signal(false);
    let inner = create_signal(false);
    let at = create_signal(Pos2::ZERO);
    let closed = move |shown: Signal<bool>| {
        move |report: beui::WindowReport| {
            if matches!(report, beui::WindowReport::Closed(_)) {
                shown.set(false);
            }
        }
    };
    let face = theme.control;
    let ink = theme.text;
    let own = use_window().map(|window| window.id);
    let placement = create_memo(move || match own {
        Some(window) => WindowPlacement::Over {
            window,
            at: at.get(),
        },
        None => WindowPlacement::AtPointer,
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Interactive
                @test_id="demo.popup.trigger"
                on_click_at={move |press: beui::PointerPress| {
                    at.set(press.pos + vec2(16.0, 16.0));
                    inner.set(false);
                    open.update(|open| *open = !*open);
                }}
            >
                <Frame fill=face radius={Radius::Md.px()} padding={Sides::symmetric(16.0, 10.0)}>
                    <Text color={ink}>"Open a popup"</Text>
                </Frame>
            </Interactive>
            <Window
                size={vec2(PANEL_WIDTH, WINDOW_HEIGHT)}
                at={at}
                placement
                role=WindowRole::Popup
                open={open}
                on_report={closed(open)}
            >|outer: WindowHandle| {
                let beside = vec2(PANEL_WIDTH - 32.0, WINDOW_HEIGHT - 40.0);
                let inner_at = create_memo(move || at.get() + beside);
                view! {
                    <Frame
                        @test_id="demo.popup.panel"
                        fill={theme.surface}
                        outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                        radius={Radius::Lg.px()}
                        padding=12.0
                    >
                        <Layers>
                            <List spacing=8.0>
                                <Text color={theme.text}>
                                    "A popup closes when a press lands outside it and its own popups."
                                </Text>
                                <Interactive
                                    @test_id="demo.popup.more"
                                    on_click={move || inner.update(|inner| *inner = !*inner)}
                                >
                                    <Text color={theme.accent}>"More"</Text>
                                </Interactive>
                            </List>
                            <Window
                                size={vec2(PANEL_WIDTH / 2.0, WINDOW_HEIGHT / 2.0)}
                                at={inner_at}
                                placement={WindowPlacement::Over { window: outer.id, at: Pos2::ZERO + beside }}
                                role=WindowRole::Popup
                                open={inner}
                                on_report={closed(inner)}
                            >|_: WindowHandle|
                                <Frame
                                    @test_id="demo.popup.inner"
                                    fill={theme.control}
                                    outline={move || Some(Outline::new(theme.border.get(), 1.0))}
                                    radius={Radius::Md.px()}
                                    padding=12.0
                                >
                                    <Text color={theme.text}>"Its own popup"</Text>
                                </Frame>
                            </Window>
                        </Layers>
                    </Frame>
                }
            }</Window>
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
    let preferences = use_preferences();
    let program_layer = use_preference_layer(PreferenceSource::Program);
    view! {
        <List spacing={SECTION_SPACING.px()}>
            <Checkbox
                @test_id="animation.instant"
                label="Disable animations"
                checked={Binding::new(create_memo(move || !preferences.get().motion.animates()), move |disabled: bool| {
                    let motion = disabled.then_some(Motion::Instant);
                    program_layer.set(PreferenceLayer { motion, ..program_layer.get_untracked() });
                })}
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
            <Text color={theme.text}>"Tween"</Text>
            <Frame
                width={move || Extent::px(position.get() + 20.0)}
                height=12.0
                fill={theme.accent}
            />
            <Text color={theme.text}>"Spring"</Text>
            <Frame
                width={move || Extent::px(spring_position.get() + 20.0)}
                height=12.0
                fill={theme.success}
            />
        </List>
    }
}

const PATH_TILE: f32 = 100.0;

#[component]
pub(crate) fn PathsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Fills and curves" code={vec![PathFills::SOURCE, PathTiles::SOURCE]}>
                <PathFills />
            </Sample>
            <Sample title="Joins and caps" code={vec![PathJoins::SOURCE, PathTiles::SOURCE]}>
                <PathJoins />
            </Sample>
            <Sample title="Dashes" code={vec![PathDashes::SOURCE, PathTiles::SOURCE]}>
                <PathDashes />
            </Sample>
        </ScrollPage>
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PathColors {
    accent: Color,
    success: Color,
    warning: Color,
    danger: Color,
    text: Color,
}

type PaintTile = fn(&Painter, Pos2, PathColors);

#[sample]
#[component]
fn PathTiles(tiles: Vec<PaintTile>) -> NodeId {
    let theme = use_theme();
    let colors = create_memo(move || PathColors {
        accent: theme.accent.get(),
        success: theme.success.get(),
        warning: theme.warning.get(),
        danger: theme.danger.get(),
        text: theme.text.get(),
    });
    let count = tiles.len();
    let keys = create_memo(move || (0..count).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal spacing={ROW_SPACING.px()} wrap=true>
            <ForEach value={keys}>|index: usize| {
                let paint = tiles[index];
                let draw: Prop<Draw> = Prop::dynamic(move || {
                    let colors = colors.get();
                    Rc::new(move |painter: &Painter, rect: Rect| {
                        let scale = rect.width() / PATH_TILE;
                        match scale < 1.0 {
                            true => {
                                let offset = rect.min.to_vec2() * (1.0 - scale);
                                paint(&painter.transformed(offset, scale), rect.min, colors);
                            }
                            false => paint(painter, rect.min, colors),
                        }
                    }) as Draw
                });
                view! {
                    <Drawing draw={draw} size={Some(Vec2::splat(PATH_TILE))} />
                }
            }</ForEach>
        </List>
    }
}

fn star(center: Pos2, points: usize, outer: f32, inner: f32) -> Path {
    Path::polygon((0..points * 2).map(|point| {
        let reach = if point % 2 == 0 { outer } else { inner };
        let angle = std::f32::consts::PI * (point as f32 / points as f32 - 0.5);
        center + vec2(angle.cos(), angle.sin()) * reach
    }))
}

fn pentagram(center: Pos2, reach: f32) -> Path {
    Path::polygon((0..5).map(|point| {
        let angle = std::f32::consts::PI * (point as f32 * 0.8 - 0.5);
        center + vec2(angle.cos(), angle.sin()) * reach
    }))
}

#[sample]
#[component]
fn PathFills() -> NodeId {
    let tiles: Vec<PaintTile> = vec![
        |painter, at, colors| {
            let ellipse = Path::ellipse(at + vec2(50.0, 50.0), vec2(46.0, 32.0));
            painter.fill_path(&ellipse, FillRule::NonZero, colors.accent);
        },
        |painter, at, colors| {
            let star = star(at + vec2(50.0, 54.0), 5, 48.0, 20.0);
            painter.fill_path(&star, FillRule::NonZero, colors.success);
        },
        |painter, at, colors| {
            let star = pentagram(at + vec2(50.0, 54.0), 48.0);
            painter.fill_path(&star, FillRule::NonZero, colors.warning);
        },
        |painter, at, colors| {
            let star = pentagram(at + vec2(50.0, 54.0), 48.0);
            painter.fill_path(&star, FillRule::EvenOdd, colors.warning);
        },
        |painter, at, colors| {
            let heart = Path::new()
                .move_to(at + vec2(50.0, 92.0))
                .cubic_to(
                    at + vec2(-10.0, 50.0),
                    at + vec2(10.0, 0.0),
                    at + vec2(50.0, 28.0),
                )
                .cubic_to(
                    at + vec2(90.0, 0.0),
                    at + vec2(110.0, 50.0),
                    at + vec2(50.0, 92.0),
                )
                .close();
            painter.fill_path(&heart, FillRule::NonZero, colors.danger);
        },
    ];
    view! {
        <PathTiles tiles={tiles} />
    }
}

#[sample]
#[component]
fn PathJoins() -> NodeId {
    fn zigzag(painter: &Painter, at: Pos2, colors: PathColors, join: LineJoin) {
        let zigzag = Path::polyline([
            at + vec2(20.0, 88.0),
            at + vec2(50.0, 16.0),
            at + vec2(80.0, 88.0),
        ]);
        painter.stroke_path(&zigzag, &Stroke::new(14.0).join(join), colors.accent);
        painter.stroke_path(&zigzag, &Stroke::new(1.0), colors.text);
    }
    fn cap(painter: &Painter, at: Pos2, colors: PathColors, cap: LineCap) {
        let line = Path::polyline([at + vec2(20.0, 50.0), at + vec2(80.0, 50.0)]);
        painter.stroke_path(&line, &Stroke::new(20.0).cap(cap), colors.accent);
        painter.stroke_path(&line, &Stroke::new(1.0), colors.text);
    }
    let tiles: Vec<PaintTile> = vec![
        |painter, at, colors| zigzag(painter, at, colors, LineJoin::Miter),
        |painter, at, colors| zigzag(painter, at, colors, LineJoin::Round),
        |painter, at, colors| zigzag(painter, at, colors, LineJoin::Bevel),
        |painter, at, colors| cap(painter, at, colors, LineCap::Butt),
        |painter, at, colors| cap(painter, at, colors, LineCap::Round),
        |painter, at, colors| cap(painter, at, colors, LineCap::Square),
        |painter, at, colors| {
            let curve = Path::new()
                .move_to(at + vec2(8.0, 90.0))
                .quad_to(at + vec2(30.0, -10.0), at + vec2(55.0, 50.0))
                .quad_to(at + vec2(75.0, 100.0), at + vec2(92.0, 10.0));
            let stroke = Stroke::new(8.0).join(LineJoin::Round).cap(LineCap::Round);
            painter.stroke_path(&curve, &stroke, colors.accent);
        },
    ];
    view! {
        <PathTiles tiles={tiles} />
    }
}

#[sample]
#[component]
fn PathDashes() -> NodeId {
    let tiles: Vec<PaintTile> = vec![
        |painter, at, colors| {
            let dashes = [
                Stroke::new(4.0).dashed([14.0, 6.0], 0.0),
                Stroke::new(6.0)
                    .cap(LineCap::Round)
                    .dashed([0.0, 12.0], 0.0),
                Stroke::new(4.0).dashed([16.0, 5.0, 3.0, 5.0], 8.0),
            ];
            for (index, stroke) in dashes.iter().enumerate() {
                let y = 22.0 + index as f32 * 28.0;
                let line = Path::polyline([at + vec2(8.0, y), at + vec2(92.0, y)]);
                painter.stroke_path(&line, stroke, colors.text);
            }
        },
        |painter, at, colors| {
            let ellipse = Path::ellipse(at + vec2(50.0, 50.0), vec2(44.0, 32.0));
            let stroke = Stroke::new(3.0).dashed([10.0, 6.0], 0.0);
            painter.stroke_path(&ellipse, &stroke, colors.accent);
        },
        |painter, at, colors| {
            let wave = Path::new().move_to(at + vec2(8.0, 50.0)).cubic_to(
                at + vec2(30.0, -20.0),
                at + vec2(70.0, 120.0),
                at + vec2(92.0, 50.0),
            );
            let stroke = Stroke::new(5.0)
                .cap(LineCap::Round)
                .dashed([12.0, 9.0], 0.0);
            painter.stroke_path(&wave, &stroke, colors.accent);
        },
        |painter, at, colors| {
            let star = star(at + vec2(50.0, 54.0), 6, 44.0, 24.0);
            let stroke = Stroke::new(3.0)
                .join(LineJoin::Round)
                .dashed([8.0, 4.0], 0.0);
            painter.stroke_path(&star, &stroke, colors.success);
        },
    ];
    view! {
        <PathTiles tiles={tiles} />
    }
}
