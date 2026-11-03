use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::display_settings::{Arrangement, DisplayMode, Rotation, ScreenUse};
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
    let use_id = format!("display.{connector}.use");
    let main_id = format!("display.{connector}.main");
    let resolution_id = format!("display.{connector}.resolution");
    let refresh_id = format!("display.{connector}.refresh");
    let scale_id = format!("display.{connector}.scale");
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
    let used = DisplaySettings::used_setting(&id);
    let used_row = settings.row(&used);
    let set_used = settings.setter(&used);
    let choices = create_memo({
        let id = id.clone();
        move || {
            displays.with(|displays| {
                std::iter::once((ScreenUse::Desktop, "Part of the desktop".to_owned()))
                    .chain(displays.iter().filter(|other| other.id != id).map(|other| {
                        (
                            ScreenUse::Mirror {
                                of: other.id.clone(),
                            },
                            format!("The same as {}", other.name),
                        )
                    }))
                    .chain(std::iter::once((ScreenUse::Off, "Off".to_owned())))
                    .collect::<Vec<_>>()
            })
        }
    });
    let choice_labels = create_memo(move || {
        choices.with(|choices| {
            choices
                .iter()
                .map(|(_, label)| label.clone())
                .collect::<Vec<_>>()
        })
    });
    let use_selected = create_memo({
        let id = id.clone();
        move || {
            let used = merged.with(|merged| merged.used(&id));
            choices.with(|choices| choices.iter().position(|(choice, _)| *choice == used))
        }
    });
    let several = create_memo(move || displays.with(|displays| displays.len() > 1));
    let on_desktop = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .is_some_and(|display| display.rect.width() > 0.0)
        })
    });
    let arranged = create_memo(move || {
        displays.with(|displays| {
            displays
                .iter()
                .filter(|display| display.rect.width() > 0.0)
                .count()
                > 1
        })
    });
    let main_shown = create_memo(move || on_desktop.get() && arranged.get());
    let is_main = create_memo({
        let id = id.clone();
        move || {
            displays.with(|displays| {
                displays
                    .iter()
                    .find(|display| display.rect.width() > 0.0)
                    .is_some_and(|first| first.id == id)
            })
        }
    });
    let main_locked = create_memo(move || is_main.get() || read_only.get());
    let arrangement = DisplaySettings::arrangement_setting();
    let save_arrangement = settings.setter(&arrangement);
    let make_main = {
        let id = id.clone();
        Callback::new(move |on: bool| {
            if !on {
                return;
            }
            let places = merged.with_untracked(DisplaySettings::arrangement).places;
            save_arrangement.call(Some(Arrangement {
                main: Some(id.clone()),
                places,
            }));
        })
    };
    let fastest = create_memo(move || rate_list.with(|rates| rates.first().copied()));
    let slower = create_memo(move || chosen.get().is_some() && chosen.get() != fastest.get());
    let at_fastest = create_memo(move || !slower.get() || read_only.get());
    view! {
        <SettingsSection title={name} description={showing}>
            <Show condition={several}>
                <LayeredSettingsRow
                    setting={used_row}
                    label="Use"
                    description="Part of the desktop, showing what another display shows, or off."
                    locked={read_only}
                    id={use_id.clone()}
                >
                    <Select
                        @sizing=ItemSize::Percent(100.0)
                        width={Extent::fraction(1.0)}
                        options={view! {
                            <ForEach keys={choice_labels}>|label: String|
                                <ChoiceOption label={label} />
                            </ForEach>
                        }}
                        selected={Binding::new(use_selected, move |index: Option<usize>| {
                            let Some(choice) = index.and_then(|index| choices.with_untracked(|choices| choices.get(index).map(|(choice, _)| choice.clone()))) else {
                                return;
                            };
                            set_used.call((choice != ScreenUse::Desktop).then_some(choice));
                        })}
                        label="Use"
                        disabled={read_only}
                        @test_id={use_id.clone()}
                    />
                </LayeredSettingsRow>
            </Show>
            <Show condition={main_shown}>
                <SettingsRow
                    label="Main display"
                    description="Shows the sign-in, the desktop's Status bar and the windows that open first."
                    separated=true
                    compact=true
                    locked={read_only}
                >
                    <Switch
                        on={Binding::new(is_main, move |on: bool| make_main.call(on))}
                        label="Main display"
                        disabled={main_locked}
                        @test_id={main_id.clone()}
                    />
                </SettingsRow>
            </Show>
            <LayeredSettingsRow
                setting={mode_row}
                label="Resolution"
                description="Until one is chosen, its preferred resolution at its fastest refresh rate."
                separated={several}
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
                    @test_id={scale_id}
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
