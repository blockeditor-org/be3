use block_editor_beui::be_block::{DisplaySettings, Theme};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;

use super::ui::Settings;

#[component]
pub(super) fn AppearanceSection(
    settings: Settings,
    root: ReadSignal<DisplaySettings>,
    inherited: Memo<DisplaySettings>,
    read_only: Memo<bool>,
) -> NodeId {
    let chosen = create_memo(move || root.get().theme);
    let inherited_label =
        create_memo(move || format!("Default ({})", inherited.get().theme().name()));
    let selected = create_memo(move || {
        Some(
            chosen
                .get()
                .and_then(|theme| Theme::ALL.iter().position(|known| *known == theme))
                .map_or(0, |index| index + 1),
        )
    });
    let modified = create_memo(move || chosen.get().is_some());
    let chosen_scale = create_memo(move || root.get().scale_percent);
    let inherited_scale_label =
        create_memo(move || format!("Default ({}%)", inherited.get().scale_percent()));
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
    let scale_modified = create_memo(move || chosen_scale.get().is_some());
    view! {
        <SettingsSection title="Appearance">
            <SettingsRow
                label="Theme"
                description="The colors of the app and every editor in it."
                modified={modified}
                locked={read_only}
                reset_test_id="display.theme.reset"
                on_reset={move || settings.operate(DisplaySettings::set_theme(None))}
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
                        let theme = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| Theme::ALL.get(index).copied());
                        settings.operate(DisplaySettings::set_theme(theme));
                    }}
                />
            </SettingsRow>
            <SettingsRow
                label="Scale"
                description="How large the app draws everything, on top of the screen's own scale."
                separated=true
                modified={scale_modified}
                locked={read_only}
                reset_test_id="display.scale.reset"
                on_reset={move || settings.operate(DisplaySettings::set_scale_percent(None))}
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
                        settings.operate(DisplaySettings::set_scale_percent(percent));
                    }}
                />
            </SettingsRow>
        </SettingsSection>
    }
}
