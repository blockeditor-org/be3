use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::be_block::display_settings::{DisplayMode, Rotation};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_ROTATE_CCW;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{HostDisplay, LayeredSettingsRow, RotationPicker};

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
    let size_selected = create_memo(move || chosen.get().map(size_of));
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
                            <ChoiceOption value=size label={size_label(size)} />
                        </ForEach>
                    }}
                    selected={Binding::new(size_selected, move |picked: Option<Size>| {
                        let Some(size) = picked else {
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
                        @test_id={format!("display.{connector}.refresh.reset")}
                        on_click={move || set.call(fastest.get_untracked())}
                    />
                }}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={rate_list}>|mode: DisplayMode|
                            <ChoiceOption value=mode label={rate_label(mode.refresh_millihertz)} />
                        </ForEach>
                    }}
                    selected={Binding::new(chosen, move |mode: Option<DisplayMode>| {
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
