use super::*;
use beui::reactive::IntoProp;
use beui::unstyled::{
    ButtonHandle, ChoiceKind, ChoiceOptionHandle, DisclosureHandle, DragHandle, DragPoint,
    DropHandle, MenuButtonHandle, MenuRowHandle, PopoverTriggerHandle, ScrollHandle,
    ScrollbarHandle, ScrollbarStyle, SliderHandle, TextInputHandle, TextInputStyle, ToggleHandle, TooltipHandle,
    thumb_length, thumb_start,
};

const FACE_PADDING_HORIZONTAL: f32 = 16.0;
const FACE_PADDING_VERTICAL: f32 = 10.0;
const PILL_RADIUS: u8 = 18;
const BOX_SIZE: f32 = 20.0;
const MARK_SIZE: f32 = 10.0;
const METER_HEIGHT: f32 = 28.0;
const UNDERLINE: f32 = 2.0;
const PANEL_WIDTH: f32 = 240.0;
const BIN_HEIGHT: f32 = 180.0;
const SCROLL_HEIGHT: f32 = 200.0;
const SCROLL_LINES: usize = 40;
const BAR_WIDTH: f32 = 6.0;
const BINS: [&str; 2] = ["Basket", "Crate"];
const PRODUCE: [&str; 6] = ["Apple", "Banana", "Cherry", "Leek", "Onion", "Potato"];

#[component]
pub(crate) fn PressingPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Pressable" code={vec![PressCounter::SOURCE]}>
                <PressCounter />
            </Sample>
            <Sample title="Button" code={vec![PillButton::SOURCE, PillFace::SOURCE]}>
                <PillButton />
            </Sample>
            <Sample title="Toggle" code={vec![SquareCheckbox::SOURCE, SquareCheckboxFace::SOURCE]}>
                <SquareCheckbox />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn PressCounter() -> NodeId {
    let theme = use_theme();
    let (presses, set_presses) = create_signal(0u32);
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let fill = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    }));
    let label = create_memo(move || format!("Pressed {} times", presses.get()));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Pressable
                on_click={move || set_presses.update(|count| *count += 1)}
                on_hover_change={move |on| set_hovered.set(on)}
                on_active_change={move |on| set_active.set(on)}
            >
                <Frame
                    color={fill}
                    radius=RADIUS
                    padding_horizontal=FACE_PADDING_HORIZONTAL
                    padding_vertical=FACE_PADDING_VERTICAL
                >
                    <Text string={label} color={theme.text.clone()} />
                </Frame>
            </unstyled::Pressable>
        </List>
    }
}

#[sample]
#[component]
fn PillButton() -> NodeId {
    let (launched, set_launched) = create_signal("Not launched".to_owned());
    let theme = use_theme();
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::Button
                    on_click={move || set_launched.set("Launched".to_owned())}
                    content={move |handle: ButtonHandle| view! {
                        <PillFace handle label="Launch" />
                    }}
                />
            </List>
            <Text string={launched} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn PillFace(handle: ButtonHandle, label: &'static str) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.accent_active,
            (false, true) => theme.accent_hover,
            (false, false) => theme.accent,
        }
    }));
    view! {
        <Frame
            color={fill}
            radius=PILL_RADIUS
            outline={theme.text.clone()}
            outline_width=2.0
            outline_offset=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=FACE_PADDING_HORIZONTAL
            padding_vertical=FACE_PADDING_VERTICAL
        >
            <Text string={label.to_owned()} color={theme.on_accent.clone()} bold=true />
        </Frame>
    }
}

#[sample]
#[component]
fn SquareCheckbox() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Toggle checked=false>
                {move |handle: ToggleHandle| view! {
                    <SquareCheckboxFace handle label="Remember me" />
                }}
            </unstyled::Toggle>
        </List>
    }
}

#[sample]
#[component]
fn SquareCheckboxFace(handle: ToggleHandle, label: &'static str) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let border = create_memo(clone!(theme -> move || match hovered.get() {
        true => theme.accent.get(),
        false => theme.text_muted.get(),
    }));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_offset=2.0
            radius=RADIUS
            outline_visible={focus_ring(focused)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame
                    width=BOX_SIZE
                    height=BOX_SIZE
                    outline={border}
                    outline_width=2.0
                    outline_visible=true
                    align_horizontal=Align::Center
                    align_vertical=Align::Center
                >
                    <Frame
                        width=MARK_SIZE
                        height=MARK_SIZE
                        color={theme.accent.clone()}
                        visible={checked}
                    />
                </Frame>
                <Text string={label.to_owned()} color={theme.text.clone()} />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn ValuesPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample title="Slider" code={vec![VolumeMeter::SOURCE, MeterFace::SOURCE]}>
                <VolumeMeter />
            </Sample>
            <Sample
                title="Text input"
                code={vec![UnderlinedInput::SOURCE, UnderlinedField::SOURCE]}
            >
                <UnderlinedInput />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn VolumeMeter() -> NodeId {
    let (volume, set_volume) = create_signal(0.3f32);
    let theme = use_theme();
    let volume_text = create_memo(clone!(volume -> move || {
        format!("Volume {}%", (volume.get() * 100.0).round())
    }));
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::Slider value={volume} on_change={move |value| set_volume.set(value)}>
                {move |handle: SliderHandle| view! {
                    <MeterFace handle />
                }}
            </unstyled::Slider>
            <Text string={volume_text} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn MeterFace(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        fraction, focused, ..
    } = handle;
    let theme = use_theme();
    let filled = create_memo(clone!(fraction -> move || ItemSize::Percent(fraction.get() * 100.0)));
    let rest = create_memo(move || ItemSize::Percent((1.0 - fraction.get()) * 100.0));
    view! {
        <Frame
            height=METER_HEIGHT
            color={theme.track.clone()}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_offset=2.0
            outline_visible={focus_ring(focused)}
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing={filled}
                    height=METER_HEIGHT
                    color={theme.accent.clone()}
                    radius=RADIUS
                />
                <Spacer @sizing={rest} />
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn UnderlinedInput() -> NodeId {
    let theme = use_theme();
    let (submitted, set_submitted) = create_signal("Press Enter to submit".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::TextInput
                value=String::new()
                placeholder="Search the docs"
                style={TextInputStyle {
                    font_size: Prop::Static(16.0),
                    color: theme.text.clone().into_prop(),
                    placeholder_color: theme.text_muted.clone().into_prop(),
                    selection_color: theme.accent_soft.clone().into_prop(),
                    caret_color: theme.accent.clone().into_prop(),
                    ..TextInputStyle::default()
                }}
                on_submit={move |value: String| set_submitted.set(format!("Searched for {value}"))}
            >
                {move |handle: TextInputHandle| view! {
                    <UnderlinedField handle />
                }}
            </unstyled::TextInput>
            <Text string={submitted} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn UnderlinedField(handle: TextInputHandle) -> NodeId {
    let TextInputHandle {
        field,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let line = create_memo(
        clone!(theme -> move || match focused.get() || hovered.get() {
            true => theme.accent.get(),
            false => theme.border.get(),
        }),
    );
    view! {
        <List spacing=6.0>
            {field}
            <Frame height=UNDERLINE color={line} />
        </List>
    }
}

#[component]
pub(crate) fn SelectingPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Choice as tabs"
                code={vec![SegmentedControl::SOURCE, SegmentFace::SOURCE]}
            >
                <SegmentedControl />
            </Sample>
            <Sample
                title="Choice as a radio group"
                code={vec![SizeRadios::SOURCE, RadioFace::SOURCE]}
            >
                <SizeRadios />
            </Sample>
            <Sample title="Disclosure" code={vec![DetailsDisclosure::SOURCE]}>
                <DetailsDisclosure />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn SegmentedControl() -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                color={theme.surface_raised.clone()}
                radius=PILL_RADIUS
                padding_horizontal=4.0
                padding_vertical=4.0
            >
                <unstyled::Choice
                    options={view! {
                        <ChoiceOption label="List" />
                        <ChoiceOption label="Board" />
                        <ChoiceOption label="Calendar" />
                    }}
                    selected=Some(0)
                    kind=ChoiceKind::Tabs
                    direction=Direction::Horizontal
                >
                    {move |handle: ChoiceOptionHandle| view! {
                        <SegmentFace handle />
                    }}
                </unstyled::Choice>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn SegmentFace(handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected -> move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    }));
    let ink = create_memo(clone!(theme -> move || match selected.get() {
        true => theme.on_accent.get(),
        false => theme.text.get(),
    }));
    view! {
        <Frame
            color={fill}
            radius=PILL_RADIUS
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=14.0
            padding_vertical=6.0
        >
            <Text string={label} color={ink} />
        </Frame>
    }
}

#[sample]
#[component]
fn SizeRadios() -> NodeId {
    let sizes = ["Small", "Medium", "Large"];
    let theme = use_theme();
    let (chosen, set_chosen) = create_signal("Medium".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::Choice
                options={view! {
                    <ChoiceOption label="Small" />
                    <ChoiceOption label="Medium" />
                    <ChoiceOption label="Large" />
                }}
                selected=Some(1)
                kind=ChoiceKind::Radio
                on_change={move |selected: Option<usize>| {
                    if let Some(index) = selected {
                        set_chosen.set(sizes[index].to_owned());
                    }
                }}
            >
                {move |handle: ChoiceOptionHandle| view! {
                    <RadioFace handle />
                }}
            </unstyled::Choice>
            <Text string={chosen} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn RadioFace(handle: ChoiceOptionHandle) -> NodeId {
    let ChoiceOptionHandle {
        label,
        selected,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focus_ring(focused)}
            padding_horizontal=4.0
            padding_vertical=4.0
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame
                    width=BOX_SIZE
                    height=BOX_SIZE
                    radius=10
                    outline={theme.text_muted.clone()}
                    outline_width=2.0
                    outline_visible=true
                    align_horizontal=Align::Center
                    align_vertical=Align::Center
                >
                    <Frame
                        width=MARK_SIZE
                        height=MARK_SIZE
                        radius=5
                        color={theme.accent.clone()}
                        visible={selected}
                    />
                </Frame>
                <Text string={label} color={theme.text.clone()} />
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn DetailsDisclosure() -> NodeId {
    let theme = use_theme();
    let body = theme.text_muted.clone();
    view! {
        <unstyled::Disclosure
            spacing=8.0
            open=false
            header={move |handle: DisclosureHandle| {
                let DisclosureHandle { open, hovered, .. } = handle;
                let title = create_memo(clone!(open -> move || match open.get() {
                    true => "Hide the details".to_owned(),
                    false => "Show the details".to_owned(),
                }));
                let ink = create_memo(clone!(theme -> move || match hovered.get() {
                    true => theme.accent_hover.get(),
                    false => theme.accent.get(),
                }));
                view! {
                    <Text string={title} color={ink} underline=true />
                }
            }}
        >
            <Text
                string="A disclosure only knows whether it is open. The header and the body are \
                 whatever you give it."
                color={body}
                wrap=true
            />
        </unstyled::Disclosure>
    }
}

#[component]
pub(crate) fn PopupsPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Popover"
                code={vec![InfoPopover::SOURCE, PillFace::SOURCE, PopupPanel::SOURCE]}
            >
                <InfoPopover />
            </Sample>
            <Sample title="Tooltip" code={vec![HintTooltip::SOURCE, PopupPanel::SOURCE]}>
                <HintTooltip />
            </Sample>
            <Sample
                title="Menu button"
                code={vec![ExportMenu::SOURCE, PillFace::SOURCE, MenuRow::SOURCE, PopupPanel::SOURCE]}
            >
                <ExportMenu />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn PopupPanel(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            width=PANEL_WIDTH
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=CARD_RADIUS
            padding_horizontal=12.0
            padding_vertical=12.0
        >
            {children}
        </Frame>
    }
}

#[sample]
#[component]
fn InfoPopover() -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Popover
                label="About"
                trigger={move |handle: PopoverTriggerHandle| {
                    let PopoverTriggerHandle {
                        hovered,
                        active,
                        focused,
                        ..
                    } = handle;
                    view! {
                        <PillFace
                            handle={ButtonHandle { hovered, active, focused }}
                            label="About"
                        />
                    }
                }}
            >
                {move |handle: PopoverHandle| {
                    let close = handle.close;
                    view! {
                        <PopupPanel>
                            <List spacing=SECTION_SPACING>
                                <Text
                                    string="A popover places its content below the trigger and closes on Escape or a click outside."
                                    color={theme.text.clone()}
                                    wrap=true
                                />
                                <unstyled::Button
                                    on_click={move || close.call(())}
                                    content={move |handle: ButtonHandle| view! {
                                        <PillFace handle label="Got it" />
                                    }}
                                />
                            </List>
                        </PopupPanel>
                    }
                }}
            </unstyled::Popover>
        </List>
    }
}

#[sample]
#[component]
fn HintTooltip() -> NodeId {
    let theme = use_theme();
    let ink = theme.text.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Tooltip
                label="Saved two minutes ago"
                content={move |handle: TooltipHandle| view! {
                    <Frame padding_vertical=4.0>
                        <PopupPanel>
                            <Text string={handle.label} color={ink.clone()} />
                        </PopupPanel>
                    </Frame>
                }}
            >
                <Frame
                    color={theme.surface_raised.clone()}
                    radius=RADIUS
                    padding_horizontal=FACE_PADDING_HORIZONTAL
                    padding_vertical=FACE_PADDING_VERTICAL
                >
                    <Text string="Hover me" color={theme.text.clone()} />
                </Frame>
            </unstyled::Tooltip>
        </List>
    }
}

#[sample]
#[component]
fn ExportMenu() -> NodeId {
    let theme = use_theme();
    let (exported, set_exported) = create_signal("Nothing exported".to_owned());
    let formats = ["PDF", "PNG", "Markdown"];
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::MenuButton
                    items={view! {
                        <unstyled::MenuItem label="PDF" />
                        <unstyled::MenuItem label="PNG" />
                        <unstyled::MenuItem label="Markdown" />
                    }}
                    trigger={move |handle: MenuButtonHandle| {
                        let MenuButtonHandle {
                            hovered,
                            active,
                            focused,
                            ..
                        } = handle;
                        view! {
                            <PillFace
                                handle={ButtonHandle { hovered, active, focused }}
                                label="Export"
                            />
                        }
                    }}
                    row={move |handle: MenuRowHandle| view! {
                        <MenuRow handle />
                    }}
                    panel={move |menu: Child| view! {
                        <PopupPanel>{menu}</PopupPanel>
                    }}
                    on_select={move |path: Vec<usize>| {
                        if let [index] = path.as_slice() {
                            set_exported.set(format!("Exported as {}", formats[*index]));
                        }
                    }}
                />
            </List>
            <Text string={exported} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(
        clone!(theme -> move || match hovered.get() || focused.get() {
            true => theme.hover.get(),
            false => Color32::TRANSPARENT,
        }),
    );
    view! {
        <Frame color={fill} radius=RADIUS padding_horizontal=10.0 padding_vertical=6.0>
            <Text string={label} color={theme.text.clone()} />
        </Frame>
    }
}

#[component]
pub(crate) fn DraggingPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Draggable and drop target"
                code={vec![FruitBins::SOURCE, FruitBin::SOURCE, FruitChip::SOURCE]}
            >
                <FruitBins />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn FruitBins() -> NodeId {
    let (bins, set_bins) = create_signal(vec![0usize, 0, 0, 1, 1, 1]);
    let moves = set_bins.clone();
    view! {
        <List direction=Direction::Horizontal spacing=ROW_SPACING>
            <FruitBin @sizing=ItemSize::Percent(50.0) bin=0 bins={bins.clone()} set_bins=moves />
            <FruitBin @sizing=ItemSize::Percent(50.0) bin=1 bins set_bins />
        </List>
    }
}

#[sample]
#[component]
fn FruitBin(bin: usize, bins: ReadSignal<Vec<usize>>, set_bins: WriteSignal<Vec<usize>>) -> NodeId {
    let theme = use_theme();
    let fruits = create_memo(move || {
        (0..PRODUCE.len())
            .filter(|fruit| bins.get()[*fruit] == bin)
            .collect::<Vec<usize>>()
    });
    view! {
        <unstyled::DropTarget
            on_drop={move |(fruit, _): (usize, DragPoint)| set_bins.update(|bins| bins[fruit] = bin)}
        >
            {move |handle: DropHandle| {
                let DropHandle { over, .. } = handle;
                let edge = create_memo(clone!(theme -> move || match over.get() {
                    true => theme.accent.get(),
                    false => theme.border.get(),
                }));
                view! {
                    <Frame
                        height=BIN_HEIGHT
                        outline={edge}
                        outline_width=2.0
                        outline_visible=true
                        radius=CARD_RADIUS
                        padding_horizontal=12.0
                        padding_vertical=12.0
                    >
                        <List spacing=8.0>
                            <Text string={BINS[bin].to_owned()} color={theme.text_muted.clone()} />
                            <ForEach keys={fruits.clone()}>
                                {move |fruit: usize| view! {
                                    <unstyled::Draggable
                                        payload={Some(fruit)}
                                        preview={move |fruit: usize| view! {
                                            <FruitChip fruit lifted=true />
                                        }}
                                    >
                                        {move |handle: DragHandle| view! {
                                            <FruitChip fruit lifted={handle.dragging} />
                                        }}
                                    </unstyled::Draggable>
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                }
            }}
        </unstyled::DropTarget>
    }
}

#[sample]
#[component]
fn FruitChip(fruit: usize, lifted: Prop<bool>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || match lifted.get() {
        true => theme.accent_soft.get(),
        false => theme.surface_raised.get(),
    }));
    view! {
        <Frame color={fill} radius=RADIUS padding_horizontal=10.0 padding_vertical=6.0>
            <Text string={PRODUCE[fruit].to_owned()} color={theme.text.clone()} />
        </Frame>
    }
}

#[component]
pub(crate) fn ScrollingPage() -> NodeId {
    view! {
        <ScrollPage>
            <Sample
                title="Scroll with your own scrollbar"
                code={vec![ThinScroll::SOURCE, thin_scrollbar::SOURCE, ThinThumb::SOURCE]}
            >
                <ThinScroll />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn ThinScroll() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=CARD_RADIUS
        >
            <List spacing=0.0>
                <unstyled::Scroll
                    @sizing=ItemSize::Fixed(SCROLL_HEIGHT)
                    scrollbar={thin_scrollbar()}
                >
                    <Frame padding_horizontal=12.0 padding_vertical=12.0>
                        <List spacing=6.0>
                            <ForEach keys={(1..=SCROLL_LINES).collect::<Vec<usize>>()}>
                                {move |line: usize| view! {
                                    <Text
                                        string={format!("Line {line}")}
                                        color={theme.text.clone()}
                                    />
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                </unstyled::Scroll>
            </List>
        </Frame>
    }
}

#[sample]
fn thin_scrollbar() -> ScrollbarStyle {
    ScrollbarStyle::new(0.0, |handle: ScrollHandle| {
        let ScrollHandle {
            position,
            direction,
            scroll_to,
        } = handle;
        view! {
            <unstyled::Scrollbar
                @sizing=ItemSize::Fixed(BAR_WIDTH)
                position
                direction
                on_scroll_to={move |offset: f32| scroll_to.call(offset)}
            >
                {move |bar: ScrollbarHandle| view! {
                    <ThinThumb bar />
                }}
            </unstyled::Scrollbar>
        }
    })
}

#[sample]
#[component]
fn ThinThumb(bar: ScrollbarHandle) -> NodeId {
    let ScrollbarHandle {
        position,
        hovered,
        dragging,
        ..
    } = bar;
    let theme = use_theme();
    let before = create_memo(
        clone!(position -> move || ItemSize::Percent(thumb_start(position.get()) * 100.0)),
    );
    let length = create_memo(
        clone!(position -> move || ItemSize::Percent(thumb_length(position.get()) * 100.0)),
    );
    let color = create_memo(
        clone!(theme -> move || match hovered.get() || dragging.get() {
            true => theme.scroll_thumb_hover.get(),
            false => theme.scroll_thumb.get(),
        }),
    );
    view! {
        <List spacing=0.0>
            <Spacer @sizing={before} />
            <Frame @sizing={length} color={color} radius=3 />
        </List>
    }
}
