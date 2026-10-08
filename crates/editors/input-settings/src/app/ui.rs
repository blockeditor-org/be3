use std::rc::Rc;

use block_editor_beui::be_block::input_settings::{
    DEFAULT_REPEAT_DELAY, DEFAULT_REPEAT_RATE, InputDevice, MAX_POINTER_SPEED, MAX_REPEAT_DELAY,
    MAX_REPEAT_RATE, MIN_POINTER_SPEED, MIN_REPEAT_DELAY, MIN_REPEAT_RATE, PointerSetting,
    PointerSettings, PointerSpeed,
};
use block_editor_beui::be_block::{InputSettings, InputSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_RESET_SETTINGS;
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, Memo, ReadSignal, Show, clone,
    component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, IconButton, Scroll, Select, Slider, Switch, TextInput, use_theme,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{ContentProjection, Editor, HostInputDevice};

const PADDING: f32 = 20.0;
const VALUE_WIDTH: f32 = 72.0;
const SECTION_SPACING: f32 = 18.0;
const ROW_SPACING: f32 = 8.0;
const LABEL_SPACING: f32 = 4.0;
const COLUMN_SPACING: f32 = 12.0;

type Settings = Rc<ContentProjection<InputSettingsContent>>;
type DeviceKey = (String, u32, u32);
type Apply = Rc<dyn Fn(PointerSetting)>;

#[component]
pub fn InputSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<InputSettingsContent>();
    let read_only = editor.read_only();
    let devices = editor.input_devices();
    let root = settings.project(|content| content.root());
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
    let delay_value =
        create_memo(clone!(delay -> move || delay.get().unwrap_or(DEFAULT_REPEAT_DELAY as f32)));
    let delay_shown =
        create_memo(clone!(delay_value -> move || format!("{} ms", delay_value.get())));
    let delay_unset = create_memo(move || delay.get().is_none());
    let rate_value =
        create_memo(clone!(rate -> move || rate.get().unwrap_or(DEFAULT_REPEAT_RATE as f32)));
    let rate_shown = create_memo(clone!(rate_value -> move || format!("{} / s", rate_value.get())));
    let rate_unset = create_memo(move || rate.get().is_none());
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
                                value={delay_value}
                                shown={delay_shown}
                                unset={delay_unset}
                                min={MIN_REPEAT_DELAY as f32}
                                max={MAX_REPEAT_DELAY as f32}
                                read_only={read_only.clone()}
                                id={"repeat-delay".to_owned()}
                                on_change={move |delay: Option<f32>| {
                                    set_delay.operate(InputSettings::set_repeat_delay(delay.map(|delay| delay.round() as u32)))
                                }}
                            />
                            <SliderSetting
                                label="Repeat rate"
                                value={rate_value}
                                shown={rate_shown}
                                unset={rate_unset}
                                min={MIN_REPEAT_RATE as f32}
                                max={MAX_REPEAT_RATE as f32}
                                read_only={read_only.clone()}
                                id={"repeat-rate".to_owned()}
                                on_change={move |rate: Option<f32>| {
                                    set_rate.operate(InputSettings::set_repeat_rate(rate.map(|rate| rate.round() as u32)))
                                }}
                            />
                        </List>
                        <Pointers
                            settings={settings.clone()}
                            root={root}
                            devices={devices}
                            read_only={pointers_off}
                        />
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

fn device_key(device: &HostInputDevice) -> DeviceKey {
    (device.name.clone(), device.vendor, device.product)
}

fn identity(key: &DeviceKey) -> InputDevice {
    InputDevice {
        name: key.0.clone(),
        vendor: key.1,
        product: key.2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FieldState<T> {
    value: T,
    mixed: bool,
    set: bool,
    supported: bool,
}

fn field_state<T: Copy + PartialEq>(
    root: &InputSettings,
    devices: &[HostInputDevice],
    scope: Option<&DeviceKey>,
    get: fn(&PointerSettings) -> Option<T>,
    default: fn(&HostInputDevice) -> Option<T>,
    fallback: T,
) -> FieldState<T> {
    if let Some(key) = scope {
        let device = identity(key);
        let device_default = devices
            .iter()
            .find(|device| device_key(device) == *key)
            .and_then(default);
        return FieldState {
            value: get(&root.pointer(&device))
                .or(device_default)
                .unwrap_or(fallback),
            mixed: false,
            set: get(&root.device_pointer(&device)).is_some(),
            supported: device_default.is_some(),
        };
    }
    let values: Vec<T> = devices
        .iter()
        .filter_map(|device| {
            let device_default = default(device)?;
            Some(get(&root.pointer(&identity(&device_key(device)))).unwrap_or(device_default))
        })
        .collect();
    let value = values
        .first()
        .copied()
        .or(get(&root.every_pointer))
        .unwrap_or(fallback);
    FieldState {
        value,
        mixed: values.iter().any(|other| *other != value),
        set: get(&root.every_pointer).is_some()
            || root.pointers.values().any(|pointer| get(pointer).is_some()),
        supported: true,
    }
}

#[component]
fn Pointers(
    settings: Settings,
    root: ReadSignal<InputSettings>,
    devices: Memo<Vec<HostInputDevice>>,
    read_only: Memo<bool>,
) -> NodeId {
    let keys = create_memo(clone!(devices -> move || {
        devices.with(|devices| devices.iter().map(device_key).collect::<Vec<DeviceKey>>())
    }));
    let none = create_memo(clone!(devices -> move || devices.with(Vec::is_empty)));
    let (chosen, choose) = create_signal(None::<DeviceKey>);
    let scope = create_memo(clone!(keys -> move || {
        chosen.get().filter(|key| keys.with(|keys| keys.contains(key)))
    }));
    let selected = create_memo(clone!(keys scope -> move || {
        let index = scope
            .get()
            .and_then(|key| keys.with(|keys| keys.iter().position(|known| *known == key)));
        Some(index.map_or(0, |index| index + 1))
    }));
    let picked = keys.clone();
    let apply: Apply = Rc::new(clone!(root scope -> move |setting: PointerSetting| {
        let root = root.get_untracked();
        let edit = match scope.get_untracked() {
            None => root.set_every_pointer(setting),
            Some(key) => root.set_device_pointer(&identity(&key), setting),
        };
        settings.operate(edit);
    }));
    let state = |get: fn(&PointerSettings) -> Option<bool>,
                 default: fn(&HostInputDevice) -> Option<bool>| {
        create_memo(clone!(root devices scope -> move || {
            let root = root.get();
            devices.with(|devices| field_state(&root, devices, scope.get().as_ref(), get, default, false))
        }))
    };
    let tap = state(|pointer| pointer.tap_to_click, |device| device.tap_to_click);
    let natural = state(
        |pointer| pointer.natural_scroll,
        |device| device.natural_scroll,
    );
    let speed = create_memo(clone!(root devices scope -> move || {
        let root = root.get();
        devices.with(|devices| {
            field_state(
                &root,
                devices,
                scope.get().as_ref(),
                |pointer| pointer.speed.map(PointerSpeed::get),
                |device| device.speed.map(|speed| speed as f32),
                0.0,
            )
        })
    }));
    let speed_value = create_memo(clone!(speed -> move || speed.get().value));
    let speed_shown = create_memo(clone!(speed -> move || match speed.get() {
        FieldState { mixed: true, .. } => "Mixed".to_owned(),
        state => format!("{:+.2}", state.value),
    }));
    let speed_unset = create_memo(clone!(speed -> move || !speed.get().set));
    let has_speed = create_memo(move || speed.get().supported);
    let (tap_on, tap_mixed, tap_unset, has_tap) = switch_state(tap);
    let (natural_on, natural_mixed, natural_unset, has_natural) = switch_state(natural);
    let (speed_off, tap_off, natural_off) = (read_only.clone(), read_only.clone(), read_only);
    let (apply_speed, apply_tap, apply_natural) = (apply.clone(), apply.clone(), apply);
    view! {
        <List spacing=ROW_SPACING>
            <Heading content="Pointers" />
            <Show condition={none}>
                <Caption content="No pointer is connected to this session." />
            </Show>
            <Select
                options={view! {
                    <ChoiceOption label="All pointers" />
                    <ForEach keys={keys.clone()}>
                        {|key: DeviceKey| view! {
                            <ChoiceOption label={key.0} />
                        }}
                    </ForEach>
                }}
                selected={selected}
                label="Pointer"
                @test_id={"input-settings.pointer"}
                on_change={move |index: Option<usize>| {
                    let key = index
                        .and_then(|index| index.checked_sub(1))
                        .and_then(|index| picked.with_untracked(|keys| keys.get(index).cloned()));
                    choose.set(key);
                }}
            />
            <Show condition={has_speed}>
                <SliderSetting
                    label="Speed"
                    value={speed_value.clone()}
                    shown={speed_shown.clone()}
                    unset={speed_unset.clone()}
                    min=MIN_POINTER_SPEED
                    max=MAX_POINTER_SPEED
                    read_only={speed_off.clone()}
                    id={"pointer.speed".to_owned()}
                    on_change={clone!(apply_speed -> move |speed: Option<f32>| {
                        apply_speed(PointerSetting::Speed(speed.map(PointerSpeed::new)))
                    })}
                />
            </Show>
            <Show condition={has_tap}>
                <SwitchSetting
                    label="Tap to click"
                    on={tap_on.clone()}
                    mixed={tap_mixed.clone()}
                    unset={tap_unset.clone()}
                    read_only={tap_off.clone()}
                    id={"pointer.tap-to-click".to_owned()}
                    on_change={clone!(apply_tap -> move |tap: Option<bool>| {
                        apply_tap(PointerSetting::TapToClick(tap))
                    })}
                />
            </Show>
            <Show condition={has_natural}>
                <SwitchSetting
                    label="Natural scrolling"
                    on={natural_on.clone()}
                    mixed={natural_mixed.clone()}
                    unset={natural_unset.clone()}
                    read_only={natural_off.clone()}
                    id={"pointer.natural-scroll".to_owned()}
                    on_change={clone!(apply_natural -> move |natural: Option<bool>| {
                        apply_natural(PointerSetting::NaturalScroll(natural))
                    })}
                />
            </Show>
        </List>
    }
}

fn switch_state(state: Memo<FieldState<bool>>) -> (Memo<bool>, Memo<bool>, Memo<bool>, Memo<bool>) {
    (
        create_memo(clone!(state -> move || state.get().value)),
        create_memo(clone!(state -> move || state.get().mixed)),
        create_memo(clone!(state -> move || !state.get().set)),
        create_memo(move || state.get().supported),
    )
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
    value: Memo<f32>,
    shown: Memo<String>,
    unset: Memo<bool>,
    min: f32,
    max: f32,
    read_only: Memo<bool>,
    id: String,
    on_change: Callback<Option<f32>>,
) -> NodeId {
    let moved = on_change.clone();
    view! {
        <List spacing=LABEL_SPACING>
            <Caption content={label} />
            <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
                <Slider
                    @sizing=ItemSize::Percent(100.0)
                    value={value}
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
    on: Memo<bool>,
    mixed: Memo<bool>,
    unset: Memo<bool>,
    read_only: Memo<bool>,
    id: String,
    on_change: Callback<Option<bool>>,
) -> NodeId {
    let switched = on_change.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
            <Body @sizing=ItemSize::Percent(100.0) content={label} />
            <Switch
                on={on}
                indeterminate={mixed}
                label={label}
                disabled={read_only.clone()}
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
