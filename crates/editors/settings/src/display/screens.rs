use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::be_block::display_settings::Rotation;
use block_editor_beui::{HostDisplay, HostDisplayMode, RotationPicker};

use super::modes::{
    Size, effective, mode_label, rate_label, rates, saved, size_label, size_of, sizes,
};
use super::ui::Settings;

#[component]
pub(super) fn ScreenSection(
    id: String,
    settings: Settings,
    root: ReadSignal<DisplaySettings>,
    merged: Memo<DisplaySettings>,
    displays: Memo<Vec<HostDisplay>>,
    read_only: Memo<bool>,
) -> NodeId {
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
    let unset = create_memo({
        let id = id.clone();
        move || root.get().mode(&id).is_none()
    });
    let connector = display.with_untracked(|display| {
        display
            .as_ref()
            .map(|display| display.connector.clone())
            .unwrap_or_default()
    });
    let resolution_id = format!("display.{connector}.resolution");
    let refresh_id = format!("display.{connector}.refresh");
    let turned = create_memo({
        let id = id.clone();
        move || root.get().screen(&id).rotation.is_some()
    });
    let unturn = {
        let id = id.clone();
        move || {
            let edit = root.with_untracked(|root| root.set_rotation(&id, None));
            settings.operate(edit);
        }
    };
    let facing = create_memo({
        let id = id.clone();
        move || merged.with(|merged| merged.rotation(&id))
    });
    let pick = Callback::new({
        let id = id.clone();
        move |rotation: Rotation| {
            let edit = root.with_untracked(|root| root.set_rotation(&id, Some(rotation)));
            settings.operate(edit);
        }
    });
    let set = Callback::new(move |mode: Option<HostDisplayMode>| {
        let edit = root.with_untracked(|root| root.set_mode(&id, mode.map(saved)));
        settings.operate(edit);
    });
    let modified = create_memo(move || !unset.get());
    let fastest = create_memo(move || rate_list.with(|rates| rates.first().copied()));
    let slower = create_memo(move || chosen.get().is_some() && chosen.get() != fastest.get());
    view! {
        <SettingsSection title={name} description={showing}>
            <SettingsRow
                label="Resolution"
                description="Until one is chosen, its preferred resolution at its fastest refresh rate."
                modified={modified}
                locked={read_only}
                reset_test_id={format!("display.{connector}.reset")}
                on_reset={move || set.call(None)}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={size_list}>|size: Size|
                            <ChoiceOption label={size_label(size)} />
                        </ForEach>
                    }}
                    selected={size_selected}
                    label="Resolution"
                    disabled={read_only}
                    @test_id={resolution_id}
                    on_change={move |index: Option<usize>| {
                        let Some(size) = index.and_then(|index| size_list.with_untracked(|sizes| sizes.get(index).copied())) else {
                            return;
                        };
                        let fastest = offered.with_untracked(|modes| rates(modes, size).first().copied());
                        if fastest.is_some() {
                            set.call(fastest);
                        }
                    }}
                />
            </SettingsRow>
            <SettingsRow
                label="Refresh rate"
                separated=true
                modified={slower}
                locked={read_only}
                reset_test_id={format!("display.{connector}.refresh.reset")}
                on_reset={move || set.call(fastest.get_untracked())}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={rate_list}>|mode: HostDisplayMode|
                            <ChoiceOption label={rate_label(mode.refresh_millihertz)} />
                        </ForEach>
                    }}
                    selected={rate_selected}
                    label="Refresh rate"
                    disabled={read_only}
                    @test_id={refresh_id}
                    on_change={move |index: Option<usize>| {
                        let mode = index.and_then(|index| rate_list.with_untracked(|rates| rates.get(index).copied()));
                        if mode.is_some() {
                            set.call(mode);
                        }
                    }}
                />
            </SettingsRow>
            <SettingsRow
                label="Rotation"
                description="Pick the screen that looks upright as you hold it."
                separated=true
                modified={turned}
                locked={read_only}
                reset_test_id={format!("display.{connector}.rotation.reset")}
                on_reset={unturn}
            >
                <RotationPicker
                    test_id={format!("display.{connector}.rotation")}
                    display={display}
                    chosen={facing}
                    read_only={read_only}
                    on_pick={pick}
                />
            </SettingsRow>
        </SettingsSection>
    }
}
