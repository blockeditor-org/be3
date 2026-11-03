use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::display_settings::{DisplayMode, Rotation};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::ICON_ROTATE_CCW;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{HostDisplay, LayeredSettingsRow, RotationPicker};

use super::modes::{Size, effective, mode_label, rate_label, rates, size_label, size_of, sizes};
use super::ui::Settings;
use beui::{Extent, ItemSize};

#[component]
pub(super) fn ScreenSection(
    id: String,
    settings: Settings,
    displays: Memo<Vec<HostDisplay>>,
    read_only: Memo<bool>,
) -> NodeId {
    let merged = settings.effective();
    let display = create_memo({
        let id = id.clone();
        move || displays.with(|displays| displays.iter().find(|display| display.id == id).cloned())
    });
    let wanted = create_memo({
        let id = id.clone();
        move || merged.get().mode(&id)
    });
    let chosen = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .map(|display| effective(display, wanted.get()))
        })
    });
    let offered = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .map(|display| display.modes.clone())
                .unwrap_or_default()
        })
    });
    let size_list = create_memo(move || offered.with(|modes| sizes(modes)));
    let rate_list = create_memo(move || match chosen.get() {
        Some(chosen) => offered.with(|modes| rates(modes, size_of(chosen))),
        None => Vec::new(),
    });
    let size_selected = create_memo(move || {
        let size = size_of(chosen.get()?);
        size_list.with(|sizes| sizes.iter().position(|known| *known == size))
    });
    let rate_selected = create_memo(move || {
        let chosen = chosen.get()?;
        rate_list.with(|rates| rates.iter().position(|known| *known == chosen))
    });
    let name = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .map(|display| display.name.clone())
                .unwrap_or_default()
        })
    });
    let showing = create_memo(move || {
        display.with(|display| match display {
            Some(display) => format!(
                "{}, showing {}",
                display.connector,
                mode_label(display.current)
            ),
            None => String::new(),
        })
    });
    let connector = display.with_untracked(|display| {
        display
            .as_ref()
            .map(|display| display.connector.clone())
            .unwrap_or_default()
    });
    let resolution_id = format!("display.{connector}.resolution");
    let refresh_id = format!("display.{connector}.refresh");
    let mode = DisplaySettings::mode_setting(&id);
    let rotation = DisplaySettings::rotation_setting(&id);
    let mode_row = settings.row(&mode);
    let rotation_row = settings.row(&rotation);
    let facing = create_memo({
        let id = id.clone();
        move || merged.with(|merged| merged.rotation(&id))
    });
    let turn = settings.setter(&rotation);
    let pick = Callback::new(move |picked: Rotation| turn.call(Some(picked)));
    let choose = settings.setter(&mode);
    let set = Callback::new(move |chosen: Option<DisplayMode>| choose.call(chosen));
    let scale = DisplaySettings::screen_scale_setting(&id);
    let scale_row = settings.row(&scale);
    let chosen_scale = settings.value(&scale);
    let below_scale = settings.fallback(&scale);
    let natural_scale = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .map_or(DisplaySettings::DEFAULT_SCALE, |display| {
                    (display.default_scale * 100.0).round() as u32
                })
        })
    });
    let inherited_scale_label = create_memo(move || {
        format!(
            "Default ({}%)",
            below_scale.get().unwrap_or_else(|| natural_scale.get())
        )
    });
    let selected_scale = create_memo(move || {
        Some(
            chosen_scale
                .get()
                .and_then(|percent| {
                    DisplaySettings::SCREEN_SCALES
                        .iter()
                        .position(|known| *known == percent)
                })
                .map_or(0, |index| index + 1),
        )
    });
    let set_scale = settings.setter(&scale);
    let fastest = create_memo(move || rate_list.with(|rates| rates.first().copied()));
    let slower = create_memo(move || chosen.get().is_some() && chosen.get() != fastest.get());
    let at_fastest = create_memo(move || !slower.get() || read_only.get());
    view! {
        <SettingsSection title={name} description={showing}>
            <LayeredSettingsRow
                setting={mode_row}
                label="Resolution"
                description="Until one is chosen, its preferred resolution at its fastest refresh rate."
                locked={read_only}
                id={format!("display.{connector}")}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={size_list}>|size: Size|
                            <ChoiceOption label={size_label(size)} />
                        </ForEach>
                    }}
                    selected={Binding::new(size_selected, move |index: Option<usize>| {
                        let Some(size) = index.and_then(|index| size_list.with_untracked(|sizes| sizes.get(index).copied())) else {
                            return;
                        };
                        let fastest = offered.with_untracked(|modes| rates(modes, size).first().copied());
                        if fastest.is_some() {
                            set.call(fastest);
                        }
                    })}
                    label="Resolution"
                    disabled={read_only}
                    @test_id={resolution_id}
                />
            </LayeredSettingsRow>
            <SettingsRow
                label="Refresh rate"
                separated=true
                modified={slower}
                locked={read_only}
                menu_test_id={format!("display.{connector}.refresh.menu")}
                menu={view! {
                    <MenuItem
                        label="Use the fastest rate"
                        glyph=ICON_ROTATE_CCW
                        disabled={at_fastest}
                        row_test_id={format!("display.{connector}.refresh.reset")}
                        on_click={move || set.call(fastest.get_untracked())}
                    />
                }}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={rate_list}>|mode: DisplayMode|
                            <ChoiceOption label={rate_label(mode.refresh_millihertz)} />
                        </ForEach>
                    }}
                    selected={Binding::new(rate_selected, move |index: Option<usize>| {
                        let mode = index.and_then(|index| rate_list.with_untracked(|rates| rates.get(index).copied()));
                        if mode.is_some() {
                            set.call(mode);
                        }
                    })}
                    label="Refresh rate"
                    disabled={read_only}
                    @test_id={refresh_id}
                />
            </SettingsRow>
            <LayeredSettingsRow
                setting={scale_row}
                label="Display scale"
                description="How large everything is on this display. Until one is chosen, one that suits its size and resolution."
                separated=true
                locked={read_only}
                id={format!("display.{connector}.scale")}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption label={inherited_scale_label} />
                        <ForEach keys={DisplaySettings::SCREEN_SCALES.to_vec()}>|percent: u32|
                            <ChoiceOption label={format!("{percent}%")} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected_scale, move |index: Option<usize>| {
                        let percent = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| DisplaySettings::SCREEN_SCALES.get(index).copied());
                        set_scale.call(percent);
                    })}
                    label="Display scale"
                    disabled={read_only}
                    @test_id={format!("display.{connector}.scale")}
                />
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={rotation_row}
                label="Rotation"
                description="Pick the screen that looks upright as you hold it."
                separated=true
                locked={read_only}
                id={format!("display.{connector}.rotation")}
            >
                <RotationPicker
                    test_id={format!("display.{connector}.rotation")}
                    display={display}
                    chosen={facing}
                    read_only={read_only}
                    on_pick={pick}
                />
            </LayeredSettingsRow>
        </SettingsSection>
    }
}
