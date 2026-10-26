use super::*;
use beui::Rect;
use beui::unstyled::{
    AnchoredSheetHandle, AnchoredStyle, ButtonHandle, ChoiceKind, ChoiceOptionHandle,
    DisclosureHandle, DragHandle, DragPoint, DropHandle, MenuRowHandle, PopupTriggerHandle,
    ScrollHandle, ScrollbarHandle, ScrollbarStyle, SliderHandle, TextInputHandle, TextInputStyle,
    ToggleHandle, TooltipHandle,
};
use beui::unstyled::{ColorAreaHandle, ColorPickerArea, HueSlider, SwatchHandle, Swatches};
use beui::unstyled::{
    ColorPickerState, CompletionMenu, CompletionRowHandle, DateSegmentHandle, DateTimeBoxHandle,
    DateTimeCalendarHandle, DateTimePanelHandle, DateTimeTriggerHandle, HexText, MenuStyle,
    NumberFaceHandle, NumberFieldHandle, SheetGripHandle, TreeRevealHandle, TreeRowHandle,
    TreeToggleHandle, emoji_completer,
};

const FACE_PADDING_HORIZONTAL: f32 = 16.0;
const FACE_PADDING_VERTICAL: f32 = 10.0;
const PILL_RADIUS: f32 = 18.0;
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
const PAD_HEIGHT: f32 = 96.0;
const DOT_SIZE: f32 = 20.0;
const SWATCHES: [Color32; 4] = [
    Color32::from_rgb(0xE5, 0x48, 0x4D),
    Color32::from_rgb(0xFF, 0xC5, 0x3D),
    Color32::from_rgb(0x30, 0xA4, 0x6C),
    Color32::from_rgb(0x00, 0x90, 0xFF),
];
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
    let presses = create_signal(0u32);
    let hovered = create_signal(false);
    let active = create_signal(false);
    let fill = create_memo(move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    });
    let label = create_memo(move || format!("Pressed {} times", presses.get()));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Pressable
                on_click={move || presses.update(|count| *count += 1)}
                on_hover_change={hovered}
                on_active_change={active}
            >
                <Frame
                    fill={ColorRole::custom(fill)}
                    radius=Radius::Md
                    padding={Sides::symmetric(FACE_PADDING_HORIZONTAL, FACE_PADDING_VERTICAL)}
                >
                    <Text>{label}</Text>
                </Frame>
            </unstyled::Pressable>
        </List>
    }
}

#[sample]
#[component]
fn PillButton() -> NodeId {
    let launched = create_signal("Not launched".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::Button
                    on_click={move || launched.set("Launched".to_owned())}
                >|handle: ButtonHandle|
                    <PillFace handle label="Launch" />
                </unstyled::Button>
            </List>
            <Text color=TextColor::Muted>{launched}</Text>
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
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.accent_active,
            (false, true) => theme.accent_hover,
            (false, false) => theme.accent,
        }
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius={Radius::custom(PILL_RADIUS)}
            border={focus_border(focused, Border::new(ColorRole::Text).width(2.0).offset(2.0))}
            padding={Sides::symmetric(FACE_PADDING_HORIZONTAL, FACE_PADDING_VERTICAL)}
        >
            <Text color=TextColor::OnAccent weight=Weight::Bold>{label.to_owned()}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn SquareCheckbox() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Toggle checked=false>|handle: ToggleHandle|
                <SquareCheckboxFace handle label="Remember me" />
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
    let border = create_memo(move || match hovered.get() {
        true => theme.accent.get(),
        false => theme.text_muted.get(),
    });
    view! {
        <Frame
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0).offset(2.0))}
            radius=Radius::Md
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame
                    width=BOX_SIZE
                    height=BOX_SIZE
                    border={create_memo(move || Some(Border::new(ColorRole::Custom(border.get())).width(2.0)))}
                    align=Align::Center
                >
                    <Layers>
                        <Show condition={checked}>
                            <Frame width=MARK_SIZE height=MARK_SIZE fill=ColorRole::Accent />
                        </Show>
                    </Layers>
                </Frame>
                <Text>{label.to_owned()}</Text>
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
            <Sample title="Prose text input" code={vec![ProseInput::SOURCE]}>
                <ProseInput />
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
                code={vec![
                    HueAndHex::SOURCE,
                    PadFace::SOURCE,
                    MeterFace::SOURCE,
                    SwatchDot::SOURCE,
                    UnderlinedField::SOURCE,
                ]}
            >
                <HueAndHex />
            </Sample>
        </ScrollPage>
    }
}

#[sample]
#[component]
fn VolumeMeter() -> NodeId {
    let volume = create_signal(0.3f32);
    let volume_text = create_memo(move || format!("Volume {}%", (volume.get() * 100.0).round()));
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::Slider value={volume}>|handle: SliderHandle|
                <MeterFace handle />
            </unstyled::Slider>
            <Text color=TextColor::Muted>{volume_text}</Text>
        </List>
    }
}

#[sample]
#[component]
fn MeterFace(handle: SliderHandle) -> NodeId {
    let SliderHandle {
        fraction, focused, ..
    } = handle;
    let filled = create_memo(move || ItemSize::Percent(fraction.get() * 100.0));
    let rest = create_memo(move || ItemSize::Percent((1.0 - fraction.get()) * 100.0));
    view! {
        <Frame
            height=METER_HEIGHT
            fill=ColorRole::Track
            radius=Radius::Md
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0).offset(2.0))}
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing={filled}
                    height=METER_HEIGHT
                    fill=ColorRole::Accent
                    radius=Radius::Md
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
    let submitted = create_signal("Press Enter to submit".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::TextInput
                value=String::new()
                placeholder="Search the docs"
                style={TextInputStyle {
                    font_size: Prop::Static(16.0),
                    color: theme.text.into_prop(),
                    placeholder_color: theme.text_muted.into_prop(),
                    selection_color: theme.accent_soft.into_prop(),
                    caret_color: theme.accent.into_prop(),
                    padding_horizontal: Prop::Static(0.0),
                    padding_vertical: Prop::Static(0.0),
                    prose: Prop::Static(None),
                    geometry: Prop::Static(text_area_geometry()),
                    menu: None,
                }}
                on_submit={move |value: String| submitted.set(format!("Searched for {value}"))}
            >|handle: TextInputHandle|
                <UnderlinedField handle />
            </unstyled::TextInput>
            <Text color=TextColor::Muted>{submitted}</Text>
        </List>
    }
}

#[sample]
#[component]
fn ProseInput() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame height=120.0 border={Border::new(ColorRole::Border).width(1.0)} radius=Radius::Md>
            <unstyled::TextInput
                value="A centered, bold title\nthat wraps inside its box"
                placeholder="Title"
                multiline=true
                style={TextInputStyle {
                    font_size: Prop::Static(20.0),
                    color: theme.text.into_prop(),
                    placeholder_color: theme.text_muted.into_prop(),
                    selection_color: theme.accent_soft.into_prop(),
                    caret_color: theme.accent.into_prop(),
                    padding_horizontal: Prop::Static(8.0),
                    padding_vertical: Prop::Static(0.0),
                    prose: Prop::Static(Some(beui::unstyled::Prose {
                        bold: true,
                        align: TextAlign::Center,
                        vertical_align: TextAlign::Center,
                        line_padding: (4.0, 4.0),
                        wrap: true,
                    })),
                    geometry: Prop::Static(text_area_geometry()),
                    menu: None,
                }}
            />
        </Frame>
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
    let line = create_memo(move || match focused.get() || hovered.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    view! {
        <List spacing=6.0>
            {field}
            <Frame height=UNDERLINE fill={ColorRole::custom(line)} />
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
                    ToolbarKey::SOURCE,
                    ToolbarStrip::SOURCE,
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
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame fill=ColorRole::SurfaceRaised radius={Radius::custom(PILL_RADIUS)} padding=4.0>
                <unstyled::Choice
                    options={view! {
                        <ChoiceOption label="List" />
                        <ChoiceOption label="Board" />
                        <ChoiceOption label="Calendar" />
                    }}
                    selected=Some(0)
                    kind=ChoiceKind::Tabs
                    direction=Direction::Horizontal
                    spacing=6.0
                >|handle: ChoiceOptionHandle|
                    <SegmentFace handle />
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
    let fill = create_memo(move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    });
    let ink = create_memo(move || match selected.get() {
        true => TextColor::OnAccent,
        false => TextColor::Default,
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius={Radius::custom(PILL_RADIUS)}
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0))}
            padding={Sides::symmetric(14.0, 6.0)}
        >
            <Text color={ink}>{label}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn SizeRadios() -> NodeId {
    let sizes = ["Small", "Medium", "Large"];
    let size = create_signal(Some(1));
    let chosen = create_memo(move || {
        size.get()
            .map_or_else(String::new, |index| sizes[index].to_owned())
    });
    view! {
        <List spacing=SECTION_SPACING>
            <unstyled::Choice
                options={view! {
                    <ChoiceOption label="Small" />
                    <ChoiceOption label="Medium" />
                    <ChoiceOption label="Large" />
                }}
                selected={size}
                kind=ChoiceKind::Radio
                spacing=6.0
            >|handle: ChoiceOptionHandle|
                <RadioFace handle />
            </unstyled::Choice>
            <Text color=TextColor::Muted>{chosen}</Text>
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
    view! {
        <Frame
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0))}
            radius=Radius::Md
            padding=4.0
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Frame
                    width=BOX_SIZE
                    height=BOX_SIZE
                    radius=Radius::Lg
                    border={Border::new(ColorRole::TextMuted).width(2.0)}
                    align=Align::Center
                >
                    <Layers>
                        <Show condition={selected}>
                            <Frame
                                width=MARK_SIZE
                                height=MARK_SIZE
                                radius={Radius::custom(5.0)}
                                fill=ColorRole::Accent
                            />
                        </Show>
                    </Layers>
                </Frame>
                <Text>{label}</Text>
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn DetailsDisclosure() -> NodeId {
    let theme = use_theme();
    let body = TextColor::Muted;
    view! {
        <unstyled::Disclosure
            spacing=8.0
            open=false
            header={move |handle: DisclosureHandle| {
                let DisclosureHandle { open, hovered, .. } = handle;
                let title = create_memo(move || match open.get() {
                    true => "Hide the details".to_owned(),
                    false => "Show the details".to_owned(),
                });
                let ink = create_memo(move || match hovered.get() {
                    true => TextColor::Custom(theme.accent_hover.get()),
                    false => TextColor::Accent,
                });
                view! {
                    <Text color={ink} underline=true>{title}</Text>
                }
            }}
        >
            <Text color={body}>
                "A disclosure only knows whether it is open. The header and the body are \
                 whatever you give it."
            </Text>
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
                title="Split button"
                code={vec![RunSplit::SOURCE, PillFace::SOURCE, MenuRow::SOURCE, PopupPanel::SOURCE]}
            >
                <RunSplit />
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

fn popup_surface() -> AnchoredStyle {
    AnchoredStyle::new(
        |content: Child| {
            view! {
                <PopupPanel>{content}</PopupPanel>
            }
        },
        |handle: AnchoredSheetHandle| {
            let AnchoredSheetHandle {
                open,
                on_close,
                content,
            } = handle;
            view! {
                <beui::styled::ModalSheet open={open} fit=true on_close={on_close}>||
                    <Frame padding=12.0>{content.call(())}</Frame>
                </beui::styled::ModalSheet>
            }
        },
    )
}

#[sample]
#[component]
fn PopupPanel(children: Child) -> NodeId {
    view! {
        <Frame
            width=PANEL_WIDTH
            fill=ColorRole::SurfaceRaised
            border={Border::new(ColorRole::Border).width(1.0)}
            radius=Radius::Lg
            padding=12.0
        >
            {children}
        </Frame>
    }
}

#[sample]
#[component]
fn InfoPopover() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Popover
                label="About"
                surface={popup_surface()}
                trigger={move |handle: PopupTriggerHandle| view! {
                    <PillFace handle={handle.button} label="About" />
                }}
            >|handle: PopoverHandle| {
                let close = handle.close;
                view! {
                    <List spacing=SECTION_SPACING>
                        <Text>
                            "A popover places its content below the trigger, or in a sheet when a finger opened it, and closes on Escape or a click outside."
                        </Text>
                        <unstyled::Button on_click={move || close.call(())}>|handle: ButtonHandle|
                            <PillFace handle label="Got it" />
                        </unstyled::Button>
                    </List>
                }
            }</unstyled::Popover>
        </List>
    }
}

#[sample]
#[component]
fn HintTooltip() -> NodeId {
    let ink = TextColor::Default;
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Tooltip
                label="Saved two minutes ago"
                content={move |handle: TooltipHandle| view! {
                    <Frame padding={Sides::vertical(4.0)}>
                        <PopupPanel>
                            <Text color={ink}>{handle.label}</Text>
                        </PopupPanel>
                    </Frame>
                }}
            >
                <Frame
                    fill=ColorRole::SurfaceRaised
                    radius=Radius::Md
                    padding={Sides::symmetric(FACE_PADDING_HORIZONTAL, FACE_PADDING_VERTICAL)}
                >
                    <Text>"Hover me"</Text>
                </Frame>
            </unstyled::Tooltip>
        </List>
    }
}

#[sample]
#[component]
fn ExportMenu() -> NodeId {
    let exported = create_signal("Nothing exported".to_owned());
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
                    trigger={move |handle: PopupTriggerHandle| view! {
                        <PillFace handle={handle.button} label="Export" />
                    }}
                    menu={MenuStyle::new(
                    2.0,
                                        |handle: MenuRowHandle| view! {
                                            <MenuRow handle />
                                        },
                                        |menu: Child| view! {
                                            <PopupPanel>{menu}</PopupPanel>
                                        },
                                    )}
                    on_select={move |path: Vec<usize>| {
                        if let [index] = path.as_slice() {
                            exported.set(format!("Exported as {}", formats[*index]));
                        }
                    }}
                />
            </List>
            <Text color=TextColor::Muted>{exported}</Text>
        </List>
    }
}

#[sample]
#[component]
fn RunSplit() -> NodeId {
    let ran = create_signal("Nothing run".to_owned());
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::SplitButton
                    label="Run"
                    menu_label="More ways to run"
                    items={view! {
                        <unstyled::MenuItem label="Run with fresh data" />
                        <unstyled::MenuItem label="Run in the background" />
                    }}
                    menu={MenuStyle::new(
                    2.0,
                                        |handle: MenuRowHandle| view! {
                                            <MenuRow handle />
                                        },
                                        |menu: Child| view! {
                                            <PopupPanel>{menu}</PopupPanel>
                                        },
                                    )}
                    main={move |handle: ButtonHandle| view! {
                        <PillFace handle label="Run" />
                    }}
                    arrow={move |handle: PopupTriggerHandle| view! {
                        <PillFace handle={handle.button} label="More" />
                    }}
                    on_click={move || ran.set("Ran".to_owned())}
                    on_select={move |path: Vec<usize>| {
                        ran.set(format!("Ran option {}", path.first().map_or(0, |index| index + 1)));
                    }}
                />
            </List>
            <Text color=TextColor::Muted>{ran}</Text>
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
    let fill = create_memo(move || match hovered.get() || focused.get() {
        true => theme.hover.get(),
        false => Color32::TRANSPARENT,
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(10.0, 6.0)}
        >
            <Text>{label}</Text>
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
    let bins = create_signal(vec![0usize, 0, 0, 1, 1, 1]);
    view! {
        <List direction=Direction::Horizontal spacing=ROW_SPACING>
            <FruitBin @sizing=ItemSize::Percent(50.0) bin=0 bins />
            <FruitBin @sizing=ItemSize::Percent(50.0) bin=1 bins />
        </List>
    }
}

#[sample]
#[component]
fn FruitBin(bin: usize, bins: Signal<Vec<usize>>) -> NodeId {
    let theme = use_theme();
    let fruits = create_memo(move || {
        (0..PRODUCE.len())
            .filter(|fruit| bins.get()[*fruit] == bin)
            .collect::<Vec<usize>>()
    });
    view! {
        <unstyled::DropTarget
            on_drop={move |(fruit, _): (usize, DragPoint)| bins.update(|bins| bins[fruit] = bin)}
        >|handle: DropHandle| {
            let DropHandle { over, .. } = handle;
            let edge = create_memo(move || match over.get() {
                true => theme.accent.get(),
                false => theme.border.get(),
            });
            view! {
                <Frame
                    height=BIN_HEIGHT
                    border={create_memo(move || Some(Border::new(ColorRole::Custom(edge.get())).width(2.0)))}
                    radius=Radius::Lg
                    padding=12.0
                >
                    <List spacing=8.0>
                        <Text color=TextColor::Muted>{BINS[bin].to_owned()}</Text>
                        <ForEach keys={fruits}>|fruit: usize|
                            <unstyled::Draggable
                                payload={Some(fruit)}
                                preview={move |fruit: usize| view! {
                                    <FruitChip fruit lifted=true />
                                }}
                            >|handle: DragHandle|
                                <FruitChip fruit lifted={handle.dragging} />
                            </unstyled::Draggable>
                        </ForEach>
                    </List>
                </Frame>
            }
        }</unstyled::DropTarget>
    }
}

#[sample]
#[component]
fn FruitChip(fruit: usize, lifted: Prop<bool>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || match lifted.get() {
        true => theme.accent_soft.get(),
        false => theme.surface_raised.get(),
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(10.0, 6.0)}
        >
            <Text>{PRODUCE[fruit].to_owned()}</Text>
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
    view! {
        <Frame border={Border::new(ColorRole::Border).width(1.0)} radius=Radius::Lg>
            <List spacing=0.0>
                <unstyled::Scroll
                    @sizing=ItemSize::Fixed(SCROLL_HEIGHT)
                    scrollbar={thin_scrollbar()}
                >
                    <Frame padding=12.0>
                        <List spacing=6.0>
                            <ForEach keys={(1..=SCROLL_LINES).collect::<Vec<usize>>()}>|line: usize|
                                <Text>{format!("Line {line}")}</Text>
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
    ScrollbarStyle::new(BAR_WIDTH, |handle: ScrollHandle| {
        let ScrollHandle {
            position,
            direction,
            scroll_to,
            visibility,
            on_engage,
        } = handle;
        view! {
            <unstyled::Scrollbar
                position
                direction
                shown={create_memo(move || visibility.get() > 0.0)}
                on_scroll_to={move |offset: f32| scroll_to.call(offset)}
                on_engage={move |engaged: bool| on_engage.call(engaged)}
            >|bar: ScrollbarHandle|
                <ThinThumb bar visibility />
            </unstyled::Scrollbar>
        }
    })
}

#[sample]
#[component]
fn ThinThumb(bar: ScrollbarHandle, visibility: ReadSignal<f32>) -> NodeId {
    let ScrollbarHandle {
        position,
        direction,
        hovered,
        dragging,
        ..
    } = bar;
    let theme = use_theme();
    let color = create_memo(move || {
        let color = match hovered.get() || dragging.get() {
            true => theme.scroll_thumb_hover.get(),
            false => theme.scroll_thumb.get(),
        };
        color.scale_alpha(visibility.get())
    });
    view! {
        <unstyled::ScrollbarThumb position direction color radius=3.0 />
    }
}

#[sample]
#[component]
fn OpenableFiles() -> NodeId {
    let chosen = create_signal(0usize);
    let opened = create_signal("Double-click a file, or press Enter on it".to_owned());
    let row = move |index: usize, name: &'static str| {
        let selected = create_memo(move || chosen.get() == index);
        view! {
            <unstyled::ListRow
                selected={selected}
                on_click={move || chosen.set(index)}
                on_activate={move || opened.set(format!("Opened {name}"))}
            >|handle: ButtonHandle|
                <FileRowFace handle selected name />
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
            <Text color=TextColor::Muted>{opened}</Text>
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
    let fill = create_memo(move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent_soft,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0))}
            padding={Sides::symmetric(10.0, 6.0)}
        >
            <Text>{name.to_owned()}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn ScrubbedNumber() -> NodeId {
    let gap = create_signal(12.0f64);
    let shown = create_memo(move || format!("Gap {} px", gap.get()));
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
                />
            </Frame>
            <Text color=TextColor::Muted>{shown}</Text>
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
    let fill = create_memo(move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => theme.surface_raised,
        }
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0))}
            padding={Sides::symmetric(FACE_PADDING_HORIZONTAL, FACE_PADDING_VERTICAL)}
        >
            <Text align=TextAlign::Center>{text}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn NumberField(handle: NumberFieldHandle) -> NodeId {
    let NumberFieldHandle {
        text,
        editing,
        on_submit,
        on_focus_change,
    } = handle;
    view! {
        <unstyled::TextInput
            value={text}
            placeholder=""
            focused={editing}
            select_on_focus=true
            style={underlined_style()}
            on_submit={on_submit}
            on_focus_change={on_focus_change}
        >|handle: TextInputHandle|
            <UnderlinedField handle />
        </unstyled::TextInput>
    }
}

fn underlined_style() -> TextInputStyle {
    let theme = use_theme();
    TextInputStyle {
        font_size: Prop::Static(16.0),
        color: theme.text.into_prop(),
        placeholder_color: theme.text_muted.into_prop(),
        selection_color: theme.accent_soft.into_prop(),
        caret_color: theme.accent.into_prop(),
        padding_horizontal: Prop::Static(0.0),
        padding_vertical: Prop::Static(0.0),
        prose: Prop::Static(None),
        geometry: Prop::Static(text_area_geometry()),
        menu: None,
    }
}

#[sample]
#[component]
fn HueAndHex() -> NodeId {
    let color = create_signal(Color32::from_rgb(0x30, 0xA4, 0x6C));
    let picker: ColorPickerState =
        ColorPickerState::new(color.into(), Prop::Static(false), Callback::default());
    let shown = picker.shown();
    let hex = HexText::new(
        shown,
        false,
        Callback::new(move |color: Color32| picker.pick(color)),
    );
    view! {
        <List spacing=SECTION_SPACING>
            <ColorPickerArea picker>|handle: ColorAreaHandle|
                <PadFace handle />
            </ColorPickerArea>
            <HueSlider picker={picker}>|handle: SliderHandle|
                <MeterFace handle />
            </HueSlider>
            <Swatches
                picker={picker}
                swatches={SWATCHES.to_vec()}
                spacing=6.0
            >|handle: SwatchHandle|
                <SwatchDot handle />
            </Swatches>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame
                    width=SWATCH_SIZE
                    height=SWATCH_SIZE
                    radius=Radius::Md
                    fill={ColorRole::custom(shown)}
                />
                <unstyled::TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={hex.text()}
                    placeholder={hex.placeholder()}
                    style={underlined_style()}
                    on_submit={move |typed: String| hex.submit(typed)}
                >|handle: TextInputHandle|
                    <UnderlinedField handle />
                </unstyled::TextInput>
            </List>
        </List>
    }
}

#[sample]
#[component]
fn PadFace(handle: ColorAreaHandle) -> NodeId {
    let ColorAreaHandle { color, x, y, .. } = handle;
    let fill = create_memo(move || color.get().to_color());
    let place = create_memo(move || {
        format!(
            "{:.0}% saturation, {:.0}% value",
            x.get() * 100.0,
            (1.0 - y.get()) * 100.0
        )
    });
    view! {
        <Frame height=PAD_HEIGHT fill={ColorRole::custom(fill)} radius=Radius::Md padding=8.0>
            <Text color={TextColor::Custom(Color32::WHITE)}>{place}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn SwatchDot(handle: SwatchHandle) -> NodeId {
    let SwatchHandle {
        color,
        option: ChoiceOptionHandle {
            selected, focused, ..
        },
    } = handle;
    let ring = create_memo(move || selected.get() || focused.get());
    view! {
        <Frame
            width=DOT_SIZE
            height=DOT_SIZE
            fill={ColorRole::custom(color)}
            radius={Radius::custom(PILL_RADIUS)}
            border={create_memo(move || ring.get().then_some(Border::new(ColorRole::Text).width(2.0).offset(2.0)))}
        />
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
    let expanded = create_signal(vec![0usize]);
    let selected = create_signal(Some(4usize));
    let keys = create_memo(move || {
        let open = expanded.get();
        (0..OUTLINE.len())
            .filter(|key| OUTLINE[*key].2.is_none_or(|parent| open.contains(&parent)))
            .collect::<Vec<usize>>()
    });
    let item = Func::new(move |key: usize| {
        let (label, depth, parent) = OUTLINE[key];
        TreeItem {
            label: label.to_owned(),
            depth,
            expandable: parent.is_none(),
            expanded: expanded.get().contains(&key),
        }
    });
    let tree = ComponentRef::new();
    let viewport = NodeRef::new();
    view! {
        <Frame width=PANEL_WIDTH height=TREE_HEIGHT>
            <List spacing=0.0>
                <unstyled::Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport}>
                    <unstyled::Tree
                        api=tree
                        keys
                        item
                        selected={selected}
                        ancestors={|key: usize| OUTLINE[key].2.into_iter().collect::<Vec<usize>>()}
                        on_select={move |key: usize| selected.set(Some(key))}
                        on_expand={move |(key, open): (usize, bool)| {
                            expanded.update(|expanded| match open {
                                true => expanded.push(key),
                                false => expanded.retain(|held| *held != key),
                            });
                        }}
                    >|handle: TreeRowHandle<usize>|
                        <FolderRow handle />
                    </unstyled::Tree>
                </unstyled::Scroll>
                <unstyled::TreeReveal
                    tree
                    viewport
                    on_reveal={move |_: usize| {}}
                    button={move |handle: TreeRevealHandle| {
                        let reveal = handle.reveal;
                        view! {
                            <unstyled::Button on_click={reveal}>|handle: ButtonHandle|
                                <PillFace handle label="Show the selection" />
                            </unstyled::Button>
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
    let indent = create_memo(move || ItemSize::Fixed(item.get().depth as f32 * 16.0));
    let label = create_memo(move || item.get().label);
    let fill = create_memo(move || {
        let theme = theme.get();
        match (selected.get(), marked.get(), hovered.get()) {
            (true, _, _) => theme.accent_soft,
            (false, true, _) => theme.surface_raised,
            (false, false, true) => theme.hover,
            (false, false, false) => Color32::TRANSPARENT,
        }
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <Spacer @sizing={indent} />
            <Frame width=16.0>
                <List spacing=0.0>
                    <unstyled::TreeToggle item={item} toggle={toggle}>|handle: TreeToggleHandle| {
                        let marker = create_memo(move || match handle.expanded.get() {
                            true => "-".to_owned(),
                            false => "+".to_owned(),
                        });
                        view! {
                            <Text color=TextColor::Muted>{marker}</Text>
                        }
                    }</unstyled::TreeToggle>
                </List>
            </Frame>
            <Frame
                @sizing=ItemSize::Percent(100.0)
                fill={ColorRole::custom(fill)}
                radius=Radius::Md
                border={focus_border(focused, Border::new(ColorRole::Accent).width(2.0))}
                padding={Sides::symmetric(6.0, 3.0)}
            >
                <unstyled::TreeRowArea target>
                    <Text>{label}</Text>
                </unstyled::TreeRowArea>
            </Frame>
        </List>
    }
}

#[sample]
#[component]
fn ToolbarKey(handle: ButtonHandle) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        label,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || {
        let theme = theme.get();
        match (active.get(), hovered.get()) {
            (true, _) => theme.pressed,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(10.0, 6.0)}
        >
            <Text>{label}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn ToolbarStrip(children: Child) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::new(ColorRole::Border).width(1.0)}
            radius=Radius::Md
            padding=4.0
        >
            {children}
        </Frame>
    }
}

#[sample]
#[component]
fn PlainNote() -> NodeId {
    let theme = use_theme();
    let document = Arc::new(TextBuffer::new(
        b"Right-click for Copy and Paste, or type :tada",
    )) as Arc<dyn text_editor_core::Document>;
    let state = TextAreaState::new(document);
    view! {
        <Frame
            height=NOTE_HEIGHT
            border={Border::new(ColorRole::Border).width(1.0)}
            radius=Radius::Md
        >
            <unstyled::TextContextMenu
                state
                menu={Some(MenuStyle::new(
                2.0,
                                |handle: MenuRowHandle| view! {
                                    <MenuRow handle />
                                },
                                |menu: Child| view! {
                                    <PopupPanel>{menu}</PopupPanel>
                                },
                            )
                            .with_toolbar(
                                |handle: ButtonHandle| view! {
                                    <ToolbarKey handle />
                                },
                                |keys: Child| view! {
                                    <ToolbarStrip>{keys}</ToolbarStrip>
                                },
                            ))}
                child_size=ItemSize::Percent(100.0)
            >|open_menu: Callback<Option<Rect>>|
                <unstyled::TextArea
                    state
                    colors={create_memo(move || beui::styled::text_area_colors(&theme.get()))}
                    look={beui::styled::text_area_look()}
                    font_size=14.0
                    padding={beui::Vec2::new(12.0, 8.0)}
                    completer={emoji_completer()}
                    completion_menu={CompletionMenu::new(
                        |handle: CompletionRowHandle| view! {
                            <CompletionRow handle />
                        },
                        |rows: Child| view! {
                            <PopupPanel>{rows}</PopupPanel>
                        },
                    )}
                    on_toolbar={move |at: Option<Rect>| open_menu.call(at)}
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
    let fill = create_memo(move || match highlighted.get() {
        true => theme.hover.get(),
        false => Color32::TRANSPARENT,
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(10.0, 4.0)}
        >
            <Text>{label}</Text>
        </Frame>
    }
}

#[sample]
#[component]
fn PullUpSheet() -> NodeId {
    let open = create_signal(false);
    let ink = TextColor::Default;
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Button on_click={move || open.set(true)}>|handle: ButtonHandle|
                <PillFace handle label="Open the sheet" />
            </unstyled::Button>
            <unstyled::ModalSheet
                open={open}
                fit=true
                rest=0.9
                stops={vec![0.9]}
                scrim={Color32::from_rgba_unmultiplied(0, 0, 0, 120)}
                grip={move |_: SheetGripHandle| view! {
                    <SheetGrip />
                }}
                panel={move |content: Child| view! {
                    <Frame fill=ColorRole::Surface radius=Radius::Lg>{content}</Frame>
                }}
                on_close={move || open.set(false)}
            >
                <Frame padding=PAGE_PADDING>
                    <List spacing=SECTION_SPACING>
                        <Text color={ink}>
                            "Swipe the sheet down, tap above, or press Escape to close."
                        </Text>
                        <List direction=Direction::Horizontal spacing=0.0>
                            <unstyled::Button
                                on_click={move || open.set(false)}
                            >|handle: ButtonHandle|
                                <PillFace handle label="Done" />
                            </unstyled::Button>
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
    fn folder_label(depth: u32) -> String {
        match depth {
            0 => "The top folder. Open one, then go back.".to_owned(),
            depth => format!("{depth} folders down. Swipe back from the edge, or press Back."),
        }
    }
    let theme = use_theme();
    let depth = create_signal(0_u32);
    let nested = create_memo(move || depth.get() > 0);
    let page = theme.background;
    let ink = TextColor::Default;
    let above = move || {
        let label = folder_label(depth.get_untracked().saturating_sub(1));
        view! {
            <Frame fill={ColorRole::custom(page)}>
                <Text color={ink}>{label}</Text>
            </Frame>
        }
    };
    let label = create_memo(move || folder_label(depth.get()));
    view! {
        <unstyled::BackSlide
            enabled={nested}
            on_back={move || depth.set(depth.get_untracked().saturating_sub(1))}
            behind={above}
        >
            <Frame fill={ColorRole::custom(page)}>
                <List spacing=SECTION_SPACING>
                    <Text color={ink}>{label}</Text>
                    <List direction=Direction::Horizontal spacing=0.0>
                        <unstyled::Button
                            on_click={move || depth.set(depth.get_untracked() + 1)}
                        >|handle: ButtonHandle|
                            <PillFace handle label="Open a folder" />
                        </unstyled::Button>
                    </List>
                </List>
            </Frame>
        </unstyled::BackSlide>
    }
}

#[sample]
#[component]
fn SheetGrip() -> NodeId {
    view! {
        <Frame height=24.0 align=Align::Center>
            <Frame width=36.0 height=4.0 radius=Radius::Xs fill=ColorRole::TextMuted />
        </Frame>
    }
}

#[sample]
#[component]
fn DeadlinePicker() -> NodeId {
    let deadline = create_signal(None::<DateTime>);
    let shown = create_memo(move || match deadline.get() {
        Some(deadline) => format!("Due {}", deadline.date.label()),
        None => "No deadline".to_owned(),
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::DateTimePicker
                    value={deadline}
                    parts=DateTimeParts::Date
                    label="Deadline"
                    surface={popup_surface()}
                    segment={move |handle: DateSegmentHandle| view! {
                        <DateSegmentFace handle />
                    }}
                    literal={move |text: String| view! {
                        <Text color=TextColor::Muted>{text}</Text>
                    }}
                    segments={move |segments: Child| view! {
                        <Frame padding={Sides::symmetric(10.0, 6.0)}>{segments}</Frame>
                    }}
                    field={move |handle: DateTimeBoxHandle| view! {
                        <DateBox handle />
                    }}
                    trigger={move |handle: DateTimeTriggerHandle| view! {
                        <PillFace handle={handle.popover.button} label="Pick" />
                    }}
                    panel={move |handle: DateTimePanelHandle| view! {
                        <DatePresets handle />
                    }}
                />
            </List>
            <Text color=TextColor::Muted>{shown}</Text>
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
    let fill = create_memo(move || match focused.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    });
    let ink = create_memo(move || match placeholder.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius={Radius::custom(3.0)}
            padding={Sides::horizontal(2.0)}
        >
            <Text color={ink}>{text}</Text>
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
    let line = create_memo(move || match focused.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    view! {
        <Frame
            border={create_memo(move || Some(Border::new(ColorRole::Custom(line.get())).width(1.0)))}
            radius=Radius::Md
            padding={Sides::vertical(4.0).right(4.0)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                {field}
                {trigger}
            </List>
        </Frame>
    }
}

#[sample]
#[component]
fn DatePresets(handle: DateTimePanelHandle) -> NodeId {
    let DateTimePanelHandle {
        field,
        calendar: DateTimeCalendarHandle { today, pick, .. },
        now,
        now_label,
        clear,
        ..
    } = handle;
    let start = today.get_untracked().unwrap_or_else(Date::today);
    let preset = move |label: &'static str, date: Date| {
        view! {
            <unstyled::Button on_click={move || pick.call(date)}>|handle: ButtonHandle|
                <PillFace handle label />
            </unstyled::Button>
        }
    };
    view! {
        <List spacing=SECTION_SPACING>
            {field}
            {preset("In a week", start.add_days(7))}
            {preset("In a month", start.add_months(1))}
            <List direction=Direction::Horizontal spacing=8.0>
                <unstyled::Button on_click={now}>|handle: ButtonHandle|
                    <PillFace handle label=now_label />
                </unstyled::Button>
                <unstyled::Button on_click={clear}>|handle: ButtonHandle|
                    <PillFace handle label="Clear" />
                </unstyled::Button>
            </List>
        </List>
    }
}
