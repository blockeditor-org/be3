use std::rc::Rc;

use block_editor_beui::be_block::input_settings::{
    DEFAULT_REPEAT_DELAY, DEFAULT_REPEAT_RATE, InputDevice, MAX_POINTER_SPEED, MAX_REPEAT_DELAY,
    MAX_REPEAT_RATE, MIN_POINTER_SPEED, MIN_REPEAT_DELAY, MIN_REPEAT_RATE, PointerSettings,
    PointerSpeed,
};
use block_editor_beui::be_block::{InputSettings, InputSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_RESET_SETTINGS;
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, IconButton, Scroll, Slider, Switch, TextInput, use_theme,
};
use block_editor_beui::{ContentProjection, Editor, HostInputDevice};

const PADDING: f32 = 20.0;
const VALUE_WIDTH: f32 = 72.0;
const SECTION_SPACING: f32 = 18.0;
const ROW_SPACING: f32 = 8.0;
const LABEL_SPACING: f32 = 4.0;
const COLUMN_SPACING: f32 = 12.0;

type Settings = Rc<ContentProjection<InputSettingsContent>>;
type DeviceKey = (String, u32, u32);
type PointerChange = Rc<dyn Fn(&dyn Fn(&mut PointerSettings))>;

#[component]
pub fn InputSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<InputSettingsContent>();
    let read_only = editor.read_only();
    let devices = editor.input_devices();
    let keys = create_memo(clone!(devices -> move || {
        devices.with(|devices| devices.iter().map(device_key).collect::<Vec<DeviceKey>>())
    }));
    let none = create_memo(clone!(devices -> move || devices.with(Vec::is_empty)));
    let layout = settings.project(|content| content.root().keyboard_layout);
    let variant = settings.project(|content| content.root().keyboard_variant);
    let options = settings.project(|content| content.root().keyboard_options);
    let delay = settings.project(|content| {
        content
            .root()
            .repeat_delay
            .map(|delay| delay.milliseconds() as f32)
    });
    let rate = settings.project(|content| {
        content
            .root()
            .repeat_rate
            .map(|rate| rate.per_second() as f32)
    });
    let (set_layout, set_variant, set_options) =
        (settings.clone(), settings.clone(), settings.clone());
    let (set_delay, set_rate) = (settings.clone(), settings.clone());
    let pointers_off = read_only.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <Scroll>
                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                    <List spacing=SECTION_SPACING>
                        <List spacing=ROW_SPACING>
                            <Heading content="Keyboard" />
                            <Caption content="XKB names, like us, de or us,de." />
                            <TextSetting
                                label="Layout"
                                value={create_memo(move || layout.get())}
                                read_only={read_only.clone()}
                                id="layout"
                                on_change={move |layout: Option<String>| {
                                    set_layout.operate(InputSettings::set_keyboard_layout(layout.as_deref()))
                                }}
                            />
                            <TextSetting
                                label="Variant"
                                value={create_memo(move || variant.get())}
                                read_only={read_only.clone()}
                                id="variant"
                                on_change={move |variant: Option<String>| {
                                    set_variant.operate(InputSettings::set_keyboard_variant(variant.as_deref()))
                                }}
                            />
                            <TextSetting
                                label="Options"
                                value={create_memo(move || options.get())}
                                read_only={read_only.clone()}
                                id="options"
                                on_change={move |options: Option<String>| {
                                    set_options.operate(InputSettings::set_keyboard_options(options.as_deref()))
                                }}
                            />
                            <SliderSetting
                                label="Repeat delay"
                                value={create_memo(move || delay.get())}
                                default={DEFAULT_REPEAT_DELAY as f32}
                                min={MIN_REPEAT_DELAY as f32}
                                max={MAX_REPEAT_DELAY as f32}
                                unit=" ms"
                                read_only={read_only.clone()}
                                id={"repeat-delay".to_owned()}
                                on_change={move |delay: Option<f32>| {
                                    set_delay.operate(InputSettings::set_repeat_delay(delay.map(|delay| delay.round() as u32)))
                                }}
                            />
                            <SliderSetting
                                label="Repeat rate"
                                value={create_memo(move || rate.get())}
                                default={DEFAULT_REPEAT_RATE as f32}
                                min={MIN_REPEAT_RATE as f32}
                                max={MAX_REPEAT_RATE as f32}
                                unit=" / s"
                                read_only={read_only.clone()}
                                id={"repeat-rate".to_owned()}
                                on_change={move |rate: Option<f32>| {
                                    set_rate.operate(InputSettings::set_repeat_rate(rate.map(|rate| rate.round() as u32)))
                                }}
                            />
                        </List>
                        <List spacing=ROW_SPACING>
                            <Heading content="Pointers" />
                            <Show condition={none}>
                                <Caption content="No pointer is connected to this session." />
                            </Show>
                            <ForEach keys={keys}>
                                {move |key: DeviceKey| {
                                    let device = create_memo(clone!(devices key -> move || {
                                        devices.with(|devices| {
                                            devices.iter().find(|device| device_key(device) == key).cloned()
                                        })
                                    }));
                                    view! {
                                        <PointerDevice
                                            settings={settings.clone()}
                                            device={device}
                                            identity={InputDevice { name: key.0, vendor: key.1, product: key.2 }}
                                            read_only={pointers_off.clone()}
                                        />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

fn device_key(device: &HostInputDevice) -> DeviceKey {
    (device.name.clone(), device.vendor, device.product)
}

#[component]
fn PointerDevice(
    settings: Settings,
    device: Memo<Option<HostInputDevice>>,
    identity: InputDevice,
    read_only: Memo<bool>,
) -> NodeId {
    let id = format!("{:04x}:{:04x}", identity.vendor, identity.product);
    let name = identity.name.clone();
    let stored =
        settings.project(clone!(identity -> move |content| content.root().pointer(&identity)));
    let stored = create_memo(move || stored.get());
    let change: PointerChange = Rc::new(
        clone!(stored -> move |change: &dyn Fn(&mut PointerSettings)| {
            let mut pointer = stored.get_untracked();
            change(&mut pointer);
            settings.operate(InputSettings::set_pointer(&identity, pointer));
        }),
    );
    let speed_default = create_memo(clone!(device -> move || {
        device.get().and_then(|device| device.speed).map(|speed| speed as f32)
    }));
    let tap_default = create_memo(clone!(device -> move || {
        device.get().and_then(|device| device.tap_to_click)
    }));
    let natural_default =
        create_memo(move || device.get().and_then(|device| device.natural_scroll));
    let has_speed = create_memo(clone!(speed_default -> move || speed_default.get().is_some()));
    let has_tap = create_memo(clone!(tap_default -> move || tap_default.get().is_some()));
    let has_natural =
        create_memo(clone!(natural_default -> move || natural_default.get().is_some()));
    let speed = create_memo(clone!(stored -> move || stored.get().speed.map(PointerSpeed::get)));
    let tap = create_memo(clone!(stored -> move || stored.get().tap_to_click));
    let natural = create_memo(move || stored.get().natural_scroll);
    let (speed_off, tap_off, natural_off) = (read_only.clone(), read_only.clone(), read_only);
    let (speed_change, tap_change, natural_change) = (change.clone(), change.clone(), change);
    let (speed_id, tap_id, natural_id) = (
        format!("{id}.speed"),
        format!("{id}.tap-to-click"),
        format!("{id}.natural-scroll"),
    );
    view! {
        <List spacing=ROW_SPACING>
            <Body content={name} />
            <Show condition={has_speed}>
                <SliderSetting
                    label="Speed"
                    value={speed.clone()}
                    default={speed_default.get_untracked().unwrap_or_default()}
                    min=MIN_POINTER_SPEED
                    max=MAX_POINTER_SPEED
                    unit=""
                    read_only={speed_off.clone()}
                    id={speed_id.clone()}
                    on_change={clone!(speed_change -> move |speed: Option<f32>| {
                        speed_change(&|pointer| pointer.speed = speed.map(PointerSpeed::new))
                    })}
                />
            </Show>
            <Show condition={has_tap}>
                <SwitchSetting
                    label="Tap to click"
                    value={tap.clone()}
                    default={tap_default.clone()}
                    read_only={tap_off.clone()}
                    id={tap_id.clone()}
                    on_change={clone!(tap_change -> move |tap: Option<bool>| {
                        tap_change(&|pointer| pointer.tap_to_click = tap)
                    })}
                />
            </Show>
            <Show condition={has_natural}>
                <SwitchSetting
                    label="Natural scrolling"
                    value={natural.clone()}
                    default={natural_default.clone()}
                    read_only={natural_off.clone()}
                    id={natural_id.clone()}
                    on_change={clone!(natural_change -> move |natural: Option<bool>| {
                        natural_change(&|pointer| pointer.natural_scroll = natural)
                    })}
                />
            </Show>
        </List>
    }
}

#[component]
fn ResetButton(
    unset: Memo<bool>,
    read_only: Memo<bool>,
    id: String,
    on_reset: Callback<()>,
) -> NodeId {
    let disabled = create_memo(move || unset.get() || read_only.get());
    view! {
        <IconButton
            glyph={ICON_RESET_SETTINGS.to_owned()}
            label="Use the default"
            disabled={disabled}
            @test_id={format!("input-settings.{id}.reset")}
            on_click={move || on_reset.call(())}
        />
    }
}

#[component]
fn TextSetting(
    label: &'static str,
    value: Memo<Option<String>>,
    read_only: Memo<bool>,
    id: &'static str,
    on_change: Callback<Option<String>>,
) -> NodeId {
    let shown = create_memo(clone!(value -> move || value.get().unwrap_or_default()));
    let unset = create_memo(move || value.get().is_none());
    let typed = on_change.clone();
    let echoed = shown.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={shown}
                label={label}
                placeholder={format!("{label}: default")}
                disabled={read_only.clone()}
                @test_id={format!("input-settings.{id}")}
                on_change={move |text: String| {
                    if text != echoed.get_untracked() {
                        typed.call(Some(text));
                    }
                }}
            />
            <ResetButton
                unset={unset}
                read_only={read_only}
                id={id.to_owned()}
                on_reset={move |_: ()| on_change.call(None)}
            />
        </List>
    }
}

#[component]
fn SliderSetting(
    label: &'static str,
    value: Memo<Option<f32>>,
    default: f32,
    min: f32,
    max: f32,
    unit: &'static str,
    read_only: Memo<bool>,
    id: String,
    on_change: Callback<Option<f32>>,
) -> NodeId {
    let current = create_memo(clone!(value -> move || value.get().unwrap_or(default)));
    let shown = create_memo(clone!(current -> move || match unit {
        "" => format!("{:+.2}", current.get()),
        unit => format!("{}{unit}", current.get()),
    }));
    let unset = create_memo(move || value.get().is_none());
    let moved = on_change.clone();
    view! {
        <List spacing=LABEL_SPACING>
            <Caption content={label} />
            <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
                <Slider
                    @sizing=ItemSize::Percent(100.0)
                    value={current}
                    min={min}
                    max={max}
                    label={label}
                    disabled={read_only.clone()}
                    @test_id={format!("input-settings.{id}")}
                    on_change={move |value: f32| moved.call(Some(value))}
                />
                <Body @sizing=ItemSize::Fixed(VALUE_WIDTH) content={shown} />
                <ResetButton
                    unset={unset}
                    read_only={read_only}
                    id={id.clone()}
                    on_reset={move |_: ()| on_change.call(None)}
                />
            </List>
        </List>
    }
}

#[component]
fn SwitchSetting(
    label: &'static str,
    value: Memo<Option<bool>>,
    default: Memo<Option<bool>>,
    read_only: Memo<bool>,
    id: String,
    on_change: Callback<Option<bool>>,
) -> NodeId {
    let on = create_memo(clone!(value -> move || value.get().or(default.get()).unwrap_or(false)));
    let unset = create_memo(move || value.get().is_none());
    let switched = on_change.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
            <Body @sizing=ItemSize::Percent(100.0) content={label} />
            <Switch
                on={on}
                label={label}
                @test_id={format!("input-settings.{id}")}
                on_change={move |on: bool| switched.call(Some(on))}
            />
            <ResetButton
                unset={unset}
                read_only={read_only}
                id={id.clone()}
                on_reset={move |_: ()| on_change.call(None)}
            />
        </List>
    }
}
