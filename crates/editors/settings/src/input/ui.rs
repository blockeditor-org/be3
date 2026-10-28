use block_editor_beui::be_block::input_settings::{
    InputDevice, MAX_POINTER_SPEED, MAX_REPEAT_DELAY, MAX_REPEAT_RATE, MIN_POINTER_SPEED,
    MIN_REPEAT_DELAY, MIN_REPEAT_RATE, PointerField, PointerSetting, PointerSettings, PointerSpeed,
};
use block_editor_beui::be_block::{InputSettings, Setting};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{
    ChangesToast, Editor, HostInputDevice, InputDevicesWatch, LayeredSettings, LayeredSettingsRow,
    SettingHandle,
};

use super::remapper::RemapperSetting;
use beui::{Extent, ItemSize};

const VALUE_WIDTH: f32 = 64.0;

pub(super) type Settings = LayeredSettings<InputSettings>;
type DeviceKey = (String, u32, u32);

#[component]
pub fn InputSettingsView(editor: Editor, settings: Settings) -> NodeId {
    let read_only = editor.read_only();
    let devices = editor.watch::<InputDevicesWatch>();
    let effective = settings.effective();
    let layout = text_field(settings, InputSettings::layout_setting());
    let variant = text_field(settings, InputSettings::variant_setting());
    let options = text_field(settings, InputSettings::options_setting());
    let delay_setting = InputSettings::repeat_delay_setting();
    let rate_setting = InputSettings::repeat_rate_setting();
    let paste_setting = InputSettings::middle_click_paste_setting();
    let delay_value = create_memo(move || effective.get().repeat_delay() as f32);
    let delay_shown = create_memo(move || format!("{} ms", delay_value.get()));
    let rate_value = create_memo(move || effective.get().repeat_rate() as f32);
    let rate_shown = create_memo(move || format!("{} / s", rate_value.get()));
    let paste_on = create_memo(move || effective.get().middle_click_paste());
    let delay_row = settings.row(&delay_setting);
    let rate_row = settings.row(&rate_setting);
    let paste_row = settings.row(&paste_setting);
    let set_delay = settings.setter(&delay_setting);
    let set_rate = settings.setter(&rate_setting);
    let set_paste = settings.setter(&paste_setting);
    let offered = editor.offered::<InputDevicesWatch>();
    let elsewhere = create_memo(move || !offered.get());
    let page = NodeRef::new();
    view! {
        <SettingsPage @node_ref=&page>
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
                        field={layout}
                        read_only={read_only}
                        id="layout"
                        separated=false
                    />
                    <TextSetting
                        label="Variant"
                        description="One for each layout, like dvorak."
                        field={variant}
                        read_only={read_only}
                        id="variant"
                    />
                    <TextSetting
                        label="Options"
                        description="XKB options, like caps:escape."
                        field={options}
                        read_only={read_only}
                        id="options"
                    />
                    <SliderSetting
                        label="Repeat delay"
                        description="How long a key is held before it repeats."
                        row={delay_row}
                        value={delay_value}
                        shown={delay_shown}
                        min={MIN_REPEAT_DELAY as f32}
                        max={MAX_REPEAT_DELAY as f32}
                        read_only={read_only}
                        id={"repeat-delay".to_owned()}
                        on_change={move |delay: Option<f32>| {
                            set_delay.call(delay.map(|delay| delay.round() as u32))
                        }}
                    />
                    <SliderSetting
                        label="Repeat rate"
                        description="How many times a second a held key repeats."
                        row={rate_row}
                        value={rate_value}
                        shown={rate_shown}
                        min={MIN_REPEAT_RATE as f32}
                        max={MAX_REPEAT_RATE as f32}
                        read_only={read_only}
                        id={"repeat-rate".to_owned()}
                        on_change={move |rate: Option<f32>| {
                            set_rate.call(rate.map(|rate| rate.round() as u32))
                        }}
                    />
                </SettingsSection>
                <Pointers settings={settings} devices={devices} read_only={read_only} />
                <SettingsSection title="Programs">
                    <SwitchSetting
                        label="Middle-click paste"
                        description="Lets programs paste the text last selected in them with the middle button."
                        row={paste_row}
                        on={paste_on}
                        mixed={create_memo(|| false)}
                        read_only={read_only}
                        id={"middle-click-paste".to_owned()}
                        separated=false
                        on_change={move |on: Option<bool>| set_paste.call(on)}
                    />
                </SettingsSection>
                <RemapperSetting editor={editor} settings={settings} read_only={read_only} />
            </Show>
            <ChangesToast changes={settings.changes()} anchor={page} />
        </SettingsPage>
    }
}

fn device_key(device: &HostInputDevice) -> DeviceKey {
    (
        device.device.name.clone(),
        device.device.vendor,
        device.device.product,
    )
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
    supported: bool,
}

fn field_state<T: Copy + PartialEq>(
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
        supported: true,
    }
}

#[component]
fn Pointers(
    settings: Settings,
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
    let scopes = create_memo(move || vec![scope.get()]);
    let selected = create_memo(move || Some(scope.get()));
    let description = create_memo(move || match none.get() {
        true => "No pointer is connected to this session.".to_owned(),
        false => String::new(),
    });
    view! {
        <SettingsSection title="Pointers" description={description}>
            <SettingsRow label="Change" description="Every pointer at once, or one on its own.">
                <Select
                    options={view! {
                        <ChoiceOption value=None label="All pointers" />
                        <ForEach keys={keys}>|key: DeviceKey|
                            <ChoiceOption value={Some(key.clone())} label={key.0} />
                        </ForEach>
                    }}
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    selected={Binding::new(selected, move |picked: Option<Option<DeviceKey>>| {
                        if let Some(key) = picked {
                            chosen.set(key);
                        }
                    })}
                    label="Pointer"
                    @test_id={"input.pointer"}
                />
            </SettingsRow>
            <ForEach keys={scopes}>|scope: Option<DeviceKey>|
                <PointerRows
                    settings={settings}
                    devices={devices}
                    scope={scope}
                    read_only={read_only}
                />
            </ForEach>
        </SettingsSection>
    }
}

#[component]
fn PointerRows(
    settings: Settings,
    devices: Memo<Vec<HostInputDevice>>,
    scope: Option<DeviceKey>,
    read_only: Memo<bool>,
) -> NodeId {
    let effective = settings.effective();
    let device = scope.as_ref().map(identity);
    let speed_setting = InputSettings::pointer_setting(device.clone(), PointerField::Speed);
    let tap_setting = InputSettings::pointer_setting(device.clone(), PointerField::TapToClick);
    let natural_setting = InputSettings::pointer_setting(device, PointerField::NaturalScroll);
    let state = |get: fn(&PointerSettings) -> Option<bool>,
                 default: fn(&HostInputDevice) -> Option<bool>| {
        let scope = scope.clone();
        create_memo(move || {
            let effective = effective.get();
            devices.with(|devices| {
                field_state(&effective, devices, scope.as_ref(), get, default, false)
            })
        })
    };
    let tap = state(
        |pointer| pointer.tap_to_click,
        |device| device.defaults.tap_to_click,
    );
    let natural = state(
        |pointer| pointer.natural_scroll,
        |device| device.defaults.natural_scroll,
    );
    let speed = create_memo({
        let scope = scope.clone();
        move || {
            let effective = effective.get();
            devices.with(|devices| {
                field_state(
                    &effective,
                    devices,
                    scope.as_ref(),
                    |pointer| pointer.speed.map(PointerSpeed::get),
                    |device| device.defaults.speed.map(PointerSpeed::get),
                    0.0,
                )
            })
        }
    });
    let speed_value = create_memo(move || speed.get().value);
    let speed_shown = create_memo(move || match speed.get() {
        FieldState { mixed: true, .. } => "Mixed".to_owned(),
        state => format!("{:+.2}", state.value),
    });
    let has_speed = create_memo(move || speed.get().supported);
    let (tap_on, tap_mixed, has_tap) = switch_state(tap);
    let (natural_on, natural_mixed, has_natural) = switch_state(natural);
    let (speed_row, tap_row, natural_row) = (
        settings.row(&speed_setting),
        settings.row(&tap_setting),
        settings.row(&natural_setting),
    );
    let (set_speed, set_tap, set_natural) = (
        settings.setter(&speed_setting),
        settings.setter(&tap_setting),
        settings.setter(&natural_setting),
    );
    view! {
        <List spacing=Space::None>
            <Show condition={has_speed}>
                <SliderSetting
                    label="Speed"
                    description="How far the pointer moves for each movement of the device."
                    row={speed_row}
                    value={speed_value}
                    shown={speed_shown}
                    min=MIN_POINTER_SPEED
                    max=MAX_POINTER_SPEED
                    read_only={read_only}
                    id={"pointer.speed".to_owned()}
                    on_change={move |speed: Option<f32>| {
                        set_speed.call(speed.map(|speed| PointerSetting::Speed(Some(PointerSpeed::new(speed)))))
                    }}
                />
            </Show>
            <Show condition={has_tap}>
                <SwitchSetting
                    label="Tap to click"
                    description="A tap on a touchpad clicks."
                    row={tap_row}
                    on={tap_on}
                    mixed={tap_mixed}
                    read_only={read_only}
                    id={"pointer.tap-to-click".to_owned()}
                    on_change={move |tap: Option<bool>| {
                        set_tap.call(tap.map(|tap| PointerSetting::TapToClick(Some(tap))))
                    }}
                />
            </Show>
            <Show condition={has_natural}>
                <SwitchSetting
                    label="Natural scrolling"
                    description="Content moves the way your fingers do."
                    row={natural_row}
                    on={natural_on}
                    mixed={natural_mixed}
                    read_only={read_only}
                    id={"pointer.natural-scroll".to_owned()}
                    on_change={move |natural: Option<bool>| {
                        set_natural.call(natural.map(|natural| PointerSetting::NaturalScroll(Some(natural))))
                    }}
                />
            </Show>
        </List>
    }
}

fn switch_state(state: Memo<FieldState<bool>>) -> (Memo<bool>, Memo<bool>, Memo<bool>) {
    (
        create_memo(move || state.get().value),
        create_memo(move || state.get().mixed),
        create_memo(move || state.get().supported),
    )
}

#[derive(Clone, Copy)]
struct TextField {
    value: Memo<Option<String>>,
    inherited: Memo<Option<String>>,
    row: SettingHandle,
    on_change: Callback<Option<String>>,
}

fn text_field(settings: Settings, setting: Setting<InputSettings, String>) -> TextField {
    TextField {
        value: settings.value(&setting),
        inherited: settings.fallback(&setting),
        row: settings.row(&setting),
        on_change: settings.setter(&setting),
    }
}

#[component]
fn TextSetting(
    label: &'static str,
    description: &'static str,
    field: TextField,
    read_only: Memo<bool>,
    id: &'static str,
    #[prop(default = true)] separated: bool,
) -> NodeId {
    let TextField {
        value,
        inherited,
        row,
        on_change,
    } = field;
    let shown = create_memo(move || value.get().unwrap_or_default());
    view! {
        <LayeredSettingsRow
            setting={row}
            label={label}
            description={description}
            separated={separated}
            locked={read_only}
            id={format!("input.{id}")}
        >
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={Binding::new(shown, move |text: String| {
                    if text != shown.get_untracked() {
                        on_change.call(Some(text));
                    }
                })}
                label={label}
                placeholder={create_memo(move || {
                    inherited.get().unwrap_or_else(|| "Default".to_owned())
                })}
                disabled={read_only}
                @test_id={format!("input.{id}")}
            />
        </LayeredSettingsRow>
    }
}

#[component]
fn SliderSetting(
    label: &'static str,
    description: &'static str,
    row: SettingHandle,
    value: Memo<f32>,
    shown: Memo<String>,
    min: f32,
    max: f32,
    read_only: Memo<bool>,
    id: String,
    #[prop(default = true)] separated: bool,
    on_change: Callback<Option<f32>>,
) -> NodeId {
    view! {
        <LayeredSettingsRow
            setting={row}
            label={label}
            description={description}
            separated={separated}
            locked={read_only}
            id={format!("input.{id}")}
        >
            <Slider
                @sizing=ItemSize::Percent(100.0)
                value={Binding::new(value, move |value: f32| on_change.call(Some(value)))}
                min={min}
                max={max}
                label={label}
                disabled={read_only}
                @test_id={format!("input.{id}")}
            />
            <Text @sizing=ItemSize::Fixed(VALUE_WIDTH) color=TextColor::Muted align=TextAlign::End>
                {shown}
            </Text>
        </LayeredSettingsRow>
    }
}

#[component]
fn SwitchSetting(
    label: &'static str,
    description: &'static str,
    row: SettingHandle,
    on: Memo<bool>,
    mixed: Memo<bool>,
    read_only: Memo<bool>,
    id: String,
    #[prop(default = true)] separated: bool,
    on_change: Callback<Option<bool>>,
) -> NodeId {
    view! {
        <LayeredSettingsRow
            setting={row}
            label={label}
            description={description}
            separated={separated}
            compact=true
            locked={read_only}
            id={format!("input.{id}")}
        >
            <Switch
                on={Binding::new(on, move |on: bool| on_change.call(Some(on)))}
                indeterminate={mixed}
                label={label}
                disabled={read_only}
                @test_id={format!("input.{id}")}
            />
        </LayeredSettingsRow>
    }
}
