use block_editor_beui::be_block::input_settings::{
    InputDevice, MAX_POINTER_SPEED, MAX_REPEAT_DELAY, MAX_REPEAT_RATE, MIN_POINTER_SPEED,
    MIN_REPEAT_DELAY, MIN_REPEAT_RATE, PointerSetting, PointerSettings, PointerSpeed,
};
use block_editor_beui::be_block::{InputSettings, InputSettingsContent, Layered};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{ContentProjection, Editor, HostInputDevice, InputDevices};

use super::remapper::RemapperSetting;

const VALUE_WIDTH: f32 = 64.0;

pub(super) type Settings = ContentProjection<InputSettingsContent>;
type DeviceKey = (String, u32, u32);
type Apply = Callback<PointerSetting>;

#[component]
pub fn InputSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<InputSettingsContent>();
    let read_only = editor.read_only();
    let devices = editor.host_value::<InputDevices>();
    let root = settings.project(|content| content.root());
    let inherited = editor.inherited_settings::<InputSettings>();
    let effective = create_memo(move || root.get().over(&inherited.get()));
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
        create_memo(move || delay.get().unwrap_or(effective.get().repeat_delay() as f32));
    let delay_shown = create_memo(move || format!("{} ms", delay_value.get()));
    let delay_unset = create_memo(move || delay.get().is_none());
    let rate_value =
        create_memo(move || rate.get().unwrap_or(effective.get().repeat_rate() as f32));
    let rate_shown = create_memo(move || format!("{} / s", rate_value.get()));
    let rate_unset = create_memo(move || rate.get().is_none());
    let paste = settings.project(|content| content.root().middle_click_paste);
    let paste_on = create_memo(move || effective.get().middle_click_paste());
    let paste_unset = create_memo(move || paste.get().is_none());
    let offered = editor.host_offers::<InputDevices>();
    let elsewhere = create_memo(move || !offered.get());
    view! {
        <SettingsPage>
            <Show condition={elsewhere}>
                <SettingsSection title="Input">
                    <SettingsRow
                        label="These settings apply in a Linux session, and this device is not one."
                    />
                </SettingsSection>
            </Show>
            <Show condition={offered}>
                <SettingsSection title="Keyboard">
                    <TextSetting
                        label="Layout"
                        description="XKB names, like us, de or us,de."
                        value={create_memo(move || layout.get())}
                        inherited={create_memo(move || inherited.get().keyboard_layout)}
                        read_only={read_only}
                        id="layout"
                        separated=false
                        on_change={move |layout: Option<String>| {
                            settings.operate(InputSettings::set_keyboard_layout(layout.as_deref()))
                        }}
                    />
                    <TextSetting
                        label="Variant"
                        description="One for each layout, like dvorak."
                        value={create_memo(move || variant.get())}
                        inherited={create_memo(move || inherited.get().keyboard_variant)}
                        read_only={read_only}
                        id="variant"
                        on_change={move |variant: Option<String>| {
                            settings.operate(InputSettings::set_keyboard_variant(variant.as_deref()))
                        }}
                    />
                    <TextSetting
                        label="Options"
                        description="XKB options, like caps:escape."
                        value={create_memo(move || options.get())}
                        inherited={create_memo(move || inherited.get().keyboard_options)}
                        read_only={read_only}
                        id="options"
                        on_change={move |options: Option<String>| {
                            settings.operate(InputSettings::set_keyboard_options(options.as_deref()))
                        }}
                    />
                    <SliderSetting
                        label="Repeat delay"
                        description="How long a key is held before it repeats."
                        value={delay_value}
                        shown={delay_shown}
                        unset={delay_unset}
                        min={MIN_REPEAT_DELAY as f32}
                        max={MAX_REPEAT_DELAY as f32}
                        read_only={read_only}
                        id={"repeat-delay".to_owned()}
                        on_change={move |delay: Option<f32>| {
                            settings.operate(InputSettings::set_repeat_delay(delay.map(|delay| delay.round() as u32)))
                        }}
                    />
                    <SliderSetting
                        label="Repeat rate"
                        description="How many times a second a held key repeats."
                        value={rate_value}
                        shown={rate_shown}
                        unset={rate_unset}
                        min={MIN_REPEAT_RATE as f32}
                        max={MAX_REPEAT_RATE as f32}
                        read_only={read_only}
                        id={"repeat-rate".to_owned()}
                        on_change={move |rate: Option<f32>| {
                            settings.operate(InputSettings::set_repeat_rate(rate.map(|rate| rate.round() as u32)))
                        }}
                    />
                </SettingsSection>
                <Pointers
                    settings={settings}
                    root={root}
                    effective={effective}
                    devices={devices}
                    read_only={read_only}
                />
                <SettingsSection title="Programs">
                    <SwitchSetting
                        label="Middle-click paste"
                        description="Lets programs paste the text last selected in them with the middle button."
                        on={paste_on}
                        mixed={create_memo(|| false)}
                        unset={paste_unset}
                        read_only={read_only}
                        id={"middle-click-paste".to_owned()}
                        separated=false
                        on_change={move |on: Option<bool>| {
                            settings.operate(InputSettings::set_middle_click_paste(on))
                        }}
                    />
                </SettingsSection>
                <RemapperSetting
                    editor={editor}
                    settings={settings}
                    root={root}
                    effective={effective}
                    read_only={read_only}
                />
            </Show>
        </SettingsPage>
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
    effective: &InputSettings,
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
            value: get(&effective.pointer(&device))
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
            Some(get(&effective.pointer(&identity(&device_key(device)))).unwrap_or(device_default))
        })
        .collect();
    let value = values
        .first()
        .copied()
        .or(get(&effective.every_pointer))
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
    effective: Memo<InputSettings>,
    devices: Memo<Vec<HostInputDevice>>,
    read_only: Memo<bool>,
) -> NodeId {
    let keys = create_memo(move || {
        devices.with(|devices| devices.iter().map(device_key).collect::<Vec<DeviceKey>>())
    });
    let none = create_memo(move || devices.with(Vec::is_empty));
    let chosen = create_signal(None::<DeviceKey>);
    let scope = create_memo(move || {
        chosen
            .get()
            .filter(|key| keys.with(|keys| keys.contains(key)))
    });
    let selected = create_memo(move || {
        let index = scope
            .get()
            .and_then(|key| keys.with(|keys| keys.iter().position(|known| *known == key)));
        Some(index.map_or(0, |index| index + 1))
    });
    let apply = Apply::new(move |setting: PointerSetting| {
        let root = root.get_untracked();
        let edit = match scope.get_untracked() {
            None => root.set_every_pointer(setting),
            Some(key) => root.set_device_pointer(&identity(&key), setting),
        };
        settings.operate(edit);
    });
    let state = |get: fn(&PointerSettings) -> Option<bool>,
                 default: fn(&HostInputDevice) -> Option<bool>| {
        create_memo(move || {
            let (root, effective) = (root.get(), effective.get());
            devices.with(|devices| {
                field_state(
                    &root,
                    &effective,
                    devices,
                    scope.get().as_ref(),
                    get,
                    default,
                    false,
                )
            })
        })
    };
    let tap = state(|pointer| pointer.tap_to_click, |device| device.tap_to_click);
    let natural = state(
        |pointer| pointer.natural_scroll,
        |device| device.natural_scroll,
    );
    let speed = create_memo(move || {
        let (root, effective) = (root.get(), effective.get());
        devices.with(|devices| {
            field_state(
                &root,
                &effective,
                devices,
                scope.get().as_ref(),
                |pointer| pointer.speed.map(PointerSpeed::get),
                |device| device.speed.map(|speed| speed as f32),
                0.0,
            )
        })
    });
    let speed_value = create_memo(move || speed.get().value);
    let speed_shown = create_memo(move || match speed.get() {
        FieldState { mixed: true, .. } => "Mixed".to_owned(),
        state => format!("{:+.2}", state.value),
    });
    let speed_unset = create_memo(move || !speed.get().set);
    let has_speed = create_memo(move || speed.get().supported);
    let (tap_on, tap_mixed, tap_unset, has_tap) = switch_state(tap);
    let (natural_on, natural_mixed, natural_unset, has_natural) = switch_state(natural);
    let description = create_memo(move || match none.get() {
        true => "No pointer is connected to this session.".to_owned(),
        false => String::new(),
    });
    view! {
        <SettingsSection title="Pointers" description={description}>
            <SettingsRow label="Change" description="Every pointer at once, or one on its own.">
                <Select
                    options={view! {
                        <ChoiceOption label="All pointers" />
                        <ForEach keys={keys}>|key: DeviceKey|
                            <ChoiceOption label={key.0} />
                        </ForEach>
                    }}
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    selected={selected}
                    label="Pointer"
                    @test_id={"input.pointer"}
                    on_change={move |index: Option<usize>| {
                        let key = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| keys.with_untracked(|keys| keys.get(index).cloned()));
                        chosen.set(key);
                    }}
                />
            </SettingsRow>
            <Show condition={has_speed}>
                <SliderSetting
                    label="Speed"
                    description="How far the pointer moves for each movement of the device."
                    value={speed_value}
                    shown={speed_shown}
                    unset={speed_unset}
                    min=MIN_POINTER_SPEED
                    max=MAX_POINTER_SPEED
                    read_only={read_only}
                    id={"pointer.speed".to_owned()}
                    on_change={move |speed: Option<f32>| {
                        apply.call(PointerSetting::Speed(speed.map(PointerSpeed::new)))
                    }}
                />
            </Show>
            <Show condition={has_tap}>
                <SwitchSetting
                    label="Tap to click"
                    description="A tap on a touchpad clicks."
                    on={tap_on}
                    mixed={tap_mixed}
                    unset={tap_unset}
                    read_only={read_only}
                    id={"pointer.tap-to-click".to_owned()}
                    on_change={move |tap: Option<bool>| {
                        apply.call(PointerSetting::TapToClick(tap))
                    }}
                />
            </Show>
            <Show condition={has_natural}>
                <SwitchSetting
                    label="Natural scrolling"
                    description="Content moves the way your fingers do."
                    on={natural_on}
                    mixed={natural_mixed}
                    unset={natural_unset}
                    read_only={read_only}
                    id={"pointer.natural-scroll".to_owned()}
                    on_change={move |natural: Option<bool>| {
                        apply.call(PointerSetting::NaturalScroll(natural))
                    }}
                />
            </Show>
        </SettingsSection>
    }
}

fn switch_state(state: Memo<FieldState<bool>>) -> (Memo<bool>, Memo<bool>, Memo<bool>, Memo<bool>) {
    (
        create_memo(move || state.get().value),
        create_memo(move || state.get().mixed),
        create_memo(move || !state.get().set),
        create_memo(move || state.get().supported),
    )
}

#[component]
fn TextSetting(
    label: &'static str,
    description: &'static str,
    value: Memo<Option<String>>,
    inherited: Memo<Option<String>>,
    read_only: Memo<bool>,
    id: &'static str,
    #[prop(default = true)] separated: bool,
    on_change: Callback<Option<String>>,
) -> NodeId {
    let shown = create_memo(move || value.get().unwrap_or_default());
    let set = create_memo(move || value.get().is_some());
    view! {
        <SettingsRow
            label={label}
            description={description}
            separated={separated}
            modified={set}
            locked={read_only}
            reset_test_id={format!("input.{id}.reset")}
            on_reset={move || on_change.call(None)}
        >
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={shown}
                label={label}
                placeholder={create_memo(move || {
                    inherited.get().unwrap_or_else(|| "Default".to_owned())
                })}
                disabled={read_only}
                @test_id={format!("input.{id}")}
                on_change={move |text: String| {
                    if text != shown.get_untracked() {
                        on_change.call(Some(text));
                    }
                }}
            />
        </SettingsRow>
    }
}

#[component]
fn SliderSetting(
    label: &'static str,
    description: &'static str,
    value: Memo<f32>,
    shown: Memo<String>,
    unset: Memo<bool>,
    min: f32,
    max: f32,
    read_only: Memo<bool>,
    id: String,
    #[prop(default = true)] separated: bool,
    on_change: Callback<Option<f32>>,
) -> NodeId {
    let set = create_memo(move || !unset.get());
    view! {
        <SettingsRow
            label={label}
            description={description}
            separated={separated}
            modified={set}
            locked={read_only}
            reset_test_id={format!("input.{id}.reset")}
            on_reset={move || on_change.call(None)}
        >
            <Slider
                @sizing=ItemSize::Percent(100.0)
                value={value}
                min={min}
                max={max}
                label={label}
                disabled={read_only}
                @test_id={format!("input.{id}")}
                on_change={move |value: f32| on_change.call(Some(value))}
            />
            <Text
                @sizing=ItemSize::Fixed(VALUE_WIDTH)
                string={shown}
                color=TextColor::Muted
                align=TextAlign::End
            />
        </SettingsRow>
    }
}

#[component]
fn SwitchSetting(
    label: &'static str,
    description: &'static str,
    on: Memo<bool>,
    mixed: Memo<bool>,
    unset: Memo<bool>,
    read_only: Memo<bool>,
    id: String,
    #[prop(default = true)] separated: bool,
    on_change: Callback<Option<bool>>,
) -> NodeId {
    let set = create_memo(move || !unset.get());
    view! {
        <SettingsRow
            label={label}
            description={description}
            separated={separated}
            compact=true
            modified={set}
            locked={read_only}
            reset_test_id={format!("input.{id}.reset")}
            on_reset={move || on_change.call(None)}
        >
            <Switch
                on={on}
                indeterminate={mixed}
                label={label}
                disabled={read_only}
                @test_id={format!("input.{id}")}
                on_change={move |on: bool| on_change.call(Some(on))}
            />
        </SettingsRow>
    }
}
