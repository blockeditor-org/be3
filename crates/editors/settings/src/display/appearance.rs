use block_editor_beui::LayeredSettingsRow;
use block_editor_beui::be_block::{DisplaySettings, Theme};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;

use super::ui::Settings;

#[component]
pub(super) fn AppearanceSection(settings: Settings, read_only: Memo<bool>) -> NodeId {
    let theme = DisplaySettings::theme_setting();
    let chosen = settings.value(&theme);
    let below = settings.fallback(&theme);
    let inherited_label =
        create_memo(move || format!("Default ({})", below.get().unwrap_or_default().name()));
    let selected = create_memo(move || {
        Some(
            chosen
                .get()
                .and_then(|theme| Theme::ALL.iter().position(|known| *known == theme))
                .map_or(0, |index| index + 1),
        )
    });
    let scale = DisplaySettings::scale_setting();
    let chosen_scale = settings.value(&scale);
    let below_scale = settings.fallback(&scale);
    let inherited_scale_label = create_memo(move || {
        format!(
            "Default ({}%)",
            below_scale.get().unwrap_or(DisplaySettings::DEFAULT_SCALE)
        )
    });
    let selected_scale = create_memo(move || {
        Some(
            chosen_scale
                .get()
                .and_then(|percent| {
                    DisplaySettings::SCALES
                        .iter()
                        .position(|known| *known == percent)
                })
                .map_or(0, |index| index + 1),
        )
    });
    let theme_row = settings.row(&theme);
    let scale_row = settings.row(&scale);
    let set_theme = settings.setter(&theme);
    let set_scale = settings.setter(&scale);
    view! {
        <SettingsSection title="Appearance">
            <LayeredSettingsRow
                setting={theme_row}
                label="Theme"
                description="The colors of the app and every editor in it."
                locked={read_only}
                id={"display.theme".to_owned()}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption label={inherited_label} />
                        <ForEach keys={Theme::ALL.to_vec()}>|theme: Theme|
                            <ChoiceOption label={theme.name().to_owned()} />
                        </ForEach>
                    }}
                    selected={selected}
                    label="Theme"
                    disabled={read_only}
                    @test_id={"display.theme"}
                    on_change={move |index: Option<usize>| {
                        let chosen = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| Theme::ALL.get(index).copied());
                        set_theme.call(chosen);
                    }}
                />
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={scale_row}
                label="Scale"
                description="How large the app draws everything, on top of the screen's own scale."
                separated=true
                locked={read_only}
                id={"display.scale".to_owned()}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption label={inherited_scale_label} />
                        <ForEach keys={DisplaySettings::SCALES.to_vec()}>|percent: u32|
                            <ChoiceOption label={format!("{percent}%")} />
                        </ForEach>
                    }}
                    selected={selected_scale}
                    label="Scale"
                    disabled={read_only}
                    @test_id={"display.scale"}
                    on_change={move |index: Option<usize>| {
                        let percent = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| DisplaySettings::SCALES.get(index).copied());
                        set_scale.call(percent);
                    }}
                />
            </LayeredSettingsRow>
        </SettingsSection>
    }
}
