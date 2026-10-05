use super::*;
use beui::Pos2;
use beui::reactive::IntoProp;
use beui::reactive::NodeRef;
use beui::unstyled::{
    ButtonHandle, ChoiceKind, ChoiceOptionHandle, DisclosureHandle, DragHandle, DragPoint,
    DropHandle, MenuButtonHandle, MenuRowHandle, PopoverTriggerHandle, ScrollHandle,
    ScrollbarHandle, ScrollbarStyle, SliderHandle, TextInputHandle, TextInputStyle, ToggleHandle,
    TooltipHandle, thumb_length, thumb_start,
};
use beui::unstyled::{
    ColorPickerState, CompletionMenu, CompletionRowHandle, DateSegmentHandle, DateTimeBoxHandle,
    DateTimePanelHandle, DateTimeTriggerHandle, HexText, MenuStyle, NumberFaceHandle,
    NumberFieldHandle, SheetGripHandle, TreeRevealHandle, TreeRowHandle, emoji_completer,
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
const NUMBER_WIDTH: f32 = 120.0;
const SWATCH_SIZE: f32 = 32.0;
const TREE_HEIGHT: f32 = 180.0;
const NOTE_HEIGHT: f32 = 120.0;
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
            <Sample title="List row" code={vec![OpenableFiles::SOURCE, FileRowFace::SOURCE]}>
                <OpenableFiles />
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
            <Sample
                title="Number input"
                code={vec![
                    ScrubbedNumber::SOURCE,
                    NumberFace::SOURCE,
                    NumberField::SOURCE,
                    UnderlinedField::SOURCE,
                ]}
            >
                <ScrubbedNumber />
            </Sample>
            <Sample
                title="Color picker state and hex text"
                code={vec![HueAndHex::SOURCE, MeterFace::SOURCE, UnderlinedField::SOURCE]}
            >
                <HueAndHex />
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
            <Sample
                title="Tree"
                code={vec![FolderTree::SOURCE, FolderRow::SOURCE, PillFace::SOURCE]}
            >
                <FolderTree />
            </Sample>
            <Sample
                title="Text menu and completions"
                code={vec![
                    PlainNote::SOURCE,
                    CompletionRow::SOURCE,
                    MenuRow::SOURCE,
                    PopupPanel::SOURCE,
                ]}
            >
                <PlainNote />
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
            <Sample
                title="Sheet"
                code={vec![PullUpSheet::SOURCE, SheetGrip::SOURCE, PillFace::SOURCE]}
            >
                <PullUpSheet />
            </Sample>
            <Sample title="Back slide" code={vec![FolderPages::SOURCE, PillFace::SOURCE]}>
                <FolderPages />
            </Sample>
            <Sample
                title="Date picker"
                code={vec![
                    DeadlinePicker::SOURCE,
                    DateSegmentFace::SOURCE,
                    DateBox::SOURCE,
                    DatePresets::SOURCE,
                    PillFace::SOURCE,
                    PopupPanel::SOURCE,
                ]}
            >
                <DeadlinePicker />
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

#[sample]
#[component]
fn OpenableFiles() -> NodeId {
    let theme = use_theme();
    let (chosen, set_chosen) = create_signal(0usize);
    let (opened, set_opened) =
        create_signal("Double-click a file, or press Enter on it".to_owned());
    let row = move |index: usize, name: &'static str| {
        let selected = create_memo(clone!(chosen -> move || chosen.get() == index));
        let (choose, open) = (set_chosen.clone(), set_opened.clone());
        view! {
            <unstyled::ListRow
                selected={selected.clone()}
                on_click={move || choose.set(index)}
                on_activate={move || open.set(format!("Opened {name}"))}
            >
                {move |handle: ButtonHandle| view! {
                    <FileRowFace handle selected name />
                }}
            </unstyled::ListRow>
        }
    };
    view! {
        <List spacing=SECTION_SPACING>
            <Frame width=PANEL_WIDTH>
                <List spacing=2.0>
                    {row(0, "notes.md")}
                    {row(1, "budget.csv")}
                    {row(2, "photo.png")}
                </List>
            </Frame>
            <Text string={opened} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn FileRowFace(handle: ButtonHandle, selected: Memo<bool>, name: &'static str) -> NodeId {
    let ButtonHandle {
        hovered, focused, ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent_soft,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    }));
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=10.0
            padding_vertical=6.0
        >
            <Text string={name.to_owned()} color={theme.text.clone()} />
        </Frame>
    }
}

#[sample]
#[component]
fn ScrubbedNumber() -> NodeId {
    let theme = use_theme();
    let (gap, set_gap) = create_signal(12.0f64);
    let shown = create_memo(clone!(gap -> move || format!("Gap {} px", gap.get())));
    view! {
        <List spacing=SECTION_SPACING>
            <Frame width=NUMBER_WIDTH>
                <unstyled::NumberInput
                    value={gap}
                    min=0.0
                    max=100.0
                    label="Gap"
                    face={move |handle: NumberFaceHandle| view! {
                        <NumberFace handle />
                    }}
                    field={move |handle: NumberFieldHandle| view! {
                        <NumberField handle />
                    }}
                    on_change={move |value: f64| set_gap.set(value)}
                />
            </Frame>
            <Text string={shown} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn NumberFace(handle: NumberFaceHandle) -> NodeId {
    let NumberFaceHandle {
        text,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    }));
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=FACE_PADDING_HORIZONTAL
            padding_vertical=FACE_PADDING_VERTICAL
        >
            <Text string={text} color={theme.text.clone()} align=TextAlign::Center />
        </Frame>
    }
}

#[sample]
#[component]
fn NumberField(handle: NumberFieldHandle) -> NodeId {
    let NumberFieldHandle {
        text,
        editing,
        on_change,
        on_submit,
        on_focus_change,
        on_key,
    } = handle;
    view! {
        <unstyled::TextInput
            value={text}
            placeholder=""
            focused={editing}
            select_on_focus=true
            style={underlined_style()}
            on_change={move |typed| on_change.call(typed)}
            on_submit={move |typed| on_submit.call(typed)}
            on_focus_change={move |focused| on_focus_change.call(focused)}
            on_key_override={move |press| on_key.call(press)}
        >
            {move |handle: TextInputHandle| view! {
                <UnderlinedField handle />
            }}
        </unstyled::TextInput>
    }
}

fn underlined_style() -> TextInputStyle {
    let theme = use_theme();
    TextInputStyle {
        font_size: Prop::Static(16.0),
        color: theme.text.clone().into_prop(),
        placeholder_color: theme.text_muted.clone().into_prop(),
        selection_color: theme.accent_soft.clone().into_prop(),
        caret_color: theme.accent.clone().into_prop(),
        ..TextInputStyle::default()
    }
}

#[sample]
#[component]
fn HueAndHex() -> NodeId {
    let (color, set_color) = create_signal(Color32::from_rgb(0x30, 0xA4, 0x6C));
    let picker: ColorPickerState = ColorPickerState::new(
        color.into_prop(),
        Prop::Static(false),
        Callback::new(move |color: Color32| set_color.set(color)),
        Callback::default(),
    );
    let shown = picker.shown();
    let hue_color = picker.color();
    let hue = create_memo(move || hue_color.get().hue);
    let picked = picker.clone();
    let hex = HexText::new(
        shown.clone(),
        false,
        Callback::new(move |color: Color32| picked.pick(color)),
    );
    let (edit, submit) = (hex.clone(), hex.clone());
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::Slider
                value={hue}
                min=0.0
                max=360.0
                on_change={move |hue: f32| picker.set_hue(hue)}
            >
                {move |handle: SliderHandle| view! {
                    <MeterFace handle />
                }}
            </unstyled::Slider>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=SWATCH_SIZE height=SWATCH_SIZE radius=RADIUS color={shown} />
                <unstyled::TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={hex.text()}
                    placeholder={hex.placeholder()}
                    style={underlined_style()}
                    on_change={move |typed: String| edit.edit(typed)}
                    on_submit={move |typed: String| submit.submit(typed)}
                >
                    {move |handle: TextInputHandle| view! {
                        <UnderlinedField handle />
                    }}
                </unstyled::TextInput>
            </List>
        </List>
    }
}

const OUTLINE: [(&str, usize, Option<usize>); 6] = [
    ("Recipes", 0, None),
    ("Soup", 1, Some(0)),
    ("Bread", 1, Some(0)),
    ("Notes", 0, None),
    ("Monday", 1, Some(3)),
    ("Tuesday", 1, Some(3)),
];

#[sample]
#[component]
fn FolderTree() -> NodeId {
    let (expanded, set_expanded) = create_signal(vec![0usize]);
    let (selected, set_selected) = create_signal(Some(4usize));
    let keys = create_memo(clone!(expanded -> move || {
        let open = expanded.get();
        (0..OUTLINE.len())
            .filter(|key| OUTLINE[*key].2.is_none_or(|parent| open.contains(&parent)))
            .collect::<Vec<usize>>()
    }));
    let item = Func::new(clone!(expanded -> move |key: usize| {
        let (label, depth, parent) = OUTLINE[key];
        TreeItem {
            label: label.to_owned(),
            depth,
            expandable: parent.is_none(),
            expanded: expanded.get().contains(&key),
        }
    }));
    let tree = NodeRef::new();
    let viewport = NodeRef::new();
    let rows = tree.clone();
    view! {
        <Frame width=PANEL_WIDTH height=TREE_HEIGHT>
            <List spacing=0.0>
                <unstyled::Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport}>
                    <unstyled::Tree
                        @node_ref={&rows}
                        keys
                        item
                        selected={selected}
                        ancestors={|key: usize| OUTLINE[key].2.into_iter().collect::<Vec<usize>>()}
                        on_select={move |key: usize| set_selected.set(Some(key))}
                        on_expand={move |(key, open): (usize, bool)| {
                            set_expanded.update(|expanded| match open {
                                true => expanded.push(key),
                                false => expanded.retain(|held| *held != key),
                            });
                        }}
                    >
                        {move |handle: TreeRowHandle<usize>| view! {
                            <FolderRow handle />
                        }}
                    </unstyled::Tree>
                </unstyled::Scroll>
                <unstyled::TreeReveal
                    tree
                    viewport
                    on_reveal={move |_: usize| {}}
                    button={move |handle: TreeRevealHandle| {
                        let reveal = handle.reveal;
                        view! {
                            <unstyled::Button
                                on_click={move || reveal.call()}
                                content={move |handle: ButtonHandle| view! {
                                    <PillFace handle label="Show the selection" />
                                }}
                            />
                        }
                    }}
                />
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn FolderRow(handle: TreeRowHandle<usize>) -> NodeId {
    let TreeRowHandle {
        item,
        selected,
        marked,
        focused,
        hovered,
        toggle,
        target,
        ..
    } = handle;
    let theme = use_theme();
    let indent =
        create_memo(clone!(item -> move || ItemSize::Fixed(item.get().depth as f32 * 16.0)));
    let marker = create_memo(clone!(item -> move || {
        let item = item.get();
        match (item.expandable, item.expanded) {
            (false, _) => "",
            (true, true) => "-",
            (true, false) => "+",
        }
        .to_owned()
    }));
    let label = create_memo(clone!(item -> move || item.get().label));
    let fill = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (selected.get(), marked.get(), hovered.get()) {
            (true, _, _) => theme.accent_soft,
            (false, true, _) => theme.surface_raised,
            (false, false, true) => theme.hover,
            (false, false, false) => Color32::TRANSPARENT,
        }
    }));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <Spacer @sizing={indent} />
            <unstyled::Button
                tab_stop=false
                press_focus=false
                on_click={move || toggle()}
                content={move |_: ButtonHandle| view! {
                    <Frame width=16.0>
                        <Text string={marker} color={theme.text_muted.clone()} />
                    </Frame>
                }}
            />
            <Frame
                @sizing=ItemSize::Percent(100.0)
                color={fill}
                radius=RADIUS
                outline={theme.accent.clone()}
                outline_width=2.0
                outline_visible={focus_ring(focused)}
                padding_horizontal=6.0
                padding_vertical=3.0
            >
                <unstyled::TreeRowArea target>
                    <Text string={label} color={theme.text.clone()} />
                </unstyled::TreeRowArea>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn PlainNote() -> NodeId {
    let document = Arc::new(TextBuffer::new(
        b"Right-click for Copy and Paste, or type :tada",
    )) as Arc<dyn text_editor_core::Document>;
    let state = TextAreaState::new(document);
    let menu_state = state.clone();
    let (menu_at, set_menu_at) = create_signal(None::<Pos2>);
    let close_menu = set_menu_at.clone();
    let theme = use_theme();
    view! {
        <Frame
            height=NOTE_HEIGHT
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=RADIUS
        >
            <unstyled::TextContextMenu
                state={menu_state}
                menu={MenuStyle::new(
                    |handle: MenuRowHandle| view! {
                        <MenuRow handle />
                    },
                    |menu: Child| view! {
                        <PopupPanel>{menu}</PopupPanel>
                    },
                )}
                open_at={menu_at}
                child_size=ItemSize::Percent(100.0)
                on_close={move || close_menu.set(None)}
            >
                <unstyled::TextArea
                    state
                    completer={emoji_completer()}
                    completion_menu={CompletionMenu::new(
                        |handle: CompletionRowHandle| view! {
                            <CompletionRow handle />
                        },
                        |rows: Child| view! {
                            <PopupPanel>{rows}</PopupPanel>
                        },
                    )}
                    on_menu={move |at: Pos2| set_menu_at.set(Some(at))}
                />
            </unstyled::TextContextMenu>
        </Frame>
    }
}

#[sample]
#[component]
fn CompletionRow(handle: CompletionRowHandle) -> NodeId {
    let CompletionRowHandle {
        completion,
        highlighted,
        ..
    } = handle;
    let theme = use_theme();
    let label = create_memo(move || {
        completion
            .get()
            .map(|item| format!("{} {}", item.insert, item.label))
            .unwrap_or_default()
    });
    let fill = create_memo(clone!(theme -> move || match highlighted.get() {
        true => theme.hover.get(),
        false => Color32::TRANSPARENT,
    }));
    view! {
        <Frame color={fill} radius=RADIUS padding_horizontal=10.0 padding_vertical=4.0>
            <Text string={label} color={theme.text.clone()} />
        </Frame>
    }
}

#[sample]
#[component]
fn PullUpSheet() -> NodeId {
    let theme = use_theme();
    let (open, set_open) = create_signal(false);
    let closing = set_open.clone();
    let done = set_open.clone();
    let ink = theme.text.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Button
                on_click={move || set_open.set(true)}
                content={move |handle: ButtonHandle| view! {
                    <PillFace handle label="Open the sheet" />
                }}
            />
            <unstyled::ModalSheet
                open={open}
                fit=true
                scrim={Color32::from_rgba_unmultiplied(0, 0, 0, 120)}
                grip={move |_: SheetGripHandle| view! {
                    <SheetGrip />
                }}
                panel={move |content: Child| view! {
                    <Frame color={theme.surface.clone()} radius=CARD_RADIUS>{content}</Frame>
                }}
                on_close={move || closing.set(false)}
            >
                <Frame padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>
                    <List spacing=SECTION_SPACING>
                        <Text
                            string="Swipe the sheet down, tap above, or press Escape to close."
                            color={ink}
                            wrap=true
                        />
                        <List direction=Direction::Horizontal spacing=0.0>
                            <unstyled::Button
                                on_click={move || done.set(false)}
                                content={move |handle: ButtonHandle| view! {
                                    <PillFace handle label="Done" />
                                }}
                            />
                        </List>
                    </List>
                </Frame>
            </unstyled::ModalSheet>
        </List>
    }
}

#[sample]
#[component]
fn FolderPages() -> NodeId {
    let theme = use_theme();
    let (depth, set_depth) = create_signal(0_u32);
    let nested = create_memo(clone!(depth -> move || depth.get() > 0));
    let label = create_memo(clone!(depth -> move || match depth.get() {
        0 => "The top folder. Open one, then go back.".to_owned(),
        depth => format!("{depth} folders down. Swipe back from the edge, or press Back."),
    }));
    let rising = set_depth.clone();
    let ink = theme.text.clone();
    view! {
        <unstyled::BackSlide
            enabled={nested}
            on_back={clone!(depth -> move || rising.set(depth.get_untracked().saturating_sub(1)))}
        >
            <List spacing=SECTION_SPACING>
                <Text string={label} color={ink} wrap=true />
                <List direction=Direction::Horizontal spacing=0.0>
                    <unstyled::Button
                        on_click={move || set_depth.set(depth.get_untracked() + 1)}
                        content={move |handle: ButtonHandle| view! {
                            <PillFace handle label="Open a folder" />
                        }}
                    />
                </List>
            </List>
        </unstyled::BackSlide>
    }
}

#[sample]
#[component]
fn SheetGrip() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame height=24.0 align_horizontal=Align::Center align_vertical=Align::Center>
            <Frame width=36.0 height=4.0 radius=2 color={theme.text_muted.clone()} />
        </Frame>
    }
}

#[sample]
#[component]
fn DeadlinePicker() -> NodeId {
    let theme = use_theme();
    let (deadline, set_deadline) = create_signal(None::<DateTime>);
    let shown = create_memo(clone!(deadline -> move || match deadline.get() {
        Some(deadline) => format!("Due {}", deadline.date.label()),
        None => "No deadline".to_owned(),
    }));
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::DateTimePicker
                    value={deadline}
                    parts=DateTimeParts::Date
                    label="Deadline"
                    segment={move |handle: DateSegmentHandle| view! {
                        <DateSegmentFace handle />
                    }}
                    literal={move |text: String| view! {
                        <Text string={text} color={use_theme().text_muted.clone()} />
                    }}
                    segments={move |segments: Child| view! {
                        <Frame padding_horizontal=10.0 padding_vertical=6.0>{segments}</Frame>
                    }}
                    field={move |handle: DateTimeBoxHandle| view! {
                        <DateBox handle />
                    }}
                    trigger={move |handle: DateTimeTriggerHandle| {
                        let PopoverTriggerHandle {
                            hovered,
                            active,
                            focused,
                            ..
                        } = handle.popover;
                        view! {
                            <PillFace
                                handle={ButtonHandle { hovered, active, focused }}
                                label="Pick"
                            />
                        }
                    }}
                    panel={move |handle: DateTimePanelHandle| view! {
                        <DatePresets handle />
                    }}
                    on_change={move |next: Option<DateTime>| set_deadline.set(next)}
                />
            </List>
            <Text string={shown} color={theme.text_muted.clone()} />
        </List>
    }
}

#[sample]
#[component]
fn DateSegmentFace(handle: DateSegmentHandle) -> NodeId {
    let DateSegmentHandle {
        text,
        placeholder,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    }));
    let ink = create_memo(clone!(theme -> move || match placeholder.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    view! {
        <Frame color={fill} radius=3 padding_horizontal=2.0>
            <Text string={text} color={ink} />
        </Frame>
    }
}

#[sample]
#[component]
fn DateBox(handle: DateTimeBoxHandle) -> NodeId {
    let DateTimeBoxHandle {
        field,
        trigger,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let line = create_memo(clone!(theme -> move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let has_trigger = trigger.is_some();
    view! {
        <Frame
            outline={line}
            outline_width=1.0
            outline_visible=true
            radius=RADIUS
            padding_right=4.0
            padding_vertical=4.0
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                {field}
                <Show condition=has_trigger>{trigger.unwrap_or_else(|| unreachable!())}</Show>
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn DatePresets(handle: DateTimePanelHandle) -> NodeId {
    let DateTimePanelHandle {
        field,
        today,
        pick_date,
        now,
        now_label,
        clear,
        ..
    } = handle;
    let start = today.get_untracked().unwrap_or_else(Date::today);
    let preset = move |label: &'static str, date: Date| {
        let pick = pick_date.clone();
        view! {
            <unstyled::Button
                on_click={move || pick.call(date)}
                content={move |handle: ButtonHandle| view! {
                    <PillFace handle label />
                }}
            />
        }
    };
    view! {
        <PopupPanel>
            <List spacing=SECTION_SPACING>
                {field}
                {preset("In a week", start.add_days(7))}
                {preset("In a month", start.add_months(1))}
                <List direction=Direction::Horizontal spacing=8.0>
                    <unstyled::Button
                        on_click={move || now.call()}
                        content={move |handle: ButtonHandle| view! {
                            <PillFace handle label=now_label />
                        }}
                    />
                    <unstyled::Button
                        on_click={move || clear.call()}
                        content={move |handle: ButtonHandle| view! {
                            <PillFace handle label="Clear" />
                        }}
                    />
                </List>
            </List>
        </PopupPanel>
    }
}
