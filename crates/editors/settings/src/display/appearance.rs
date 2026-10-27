use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, ThemeKind};
use block_editor_beui::{Editor, LayeredSettingsRow, PreferenceSourcesWatch};

use super::ui::Settings;

#[component]
pub(super) fn AppearanceSection(
    editor: Editor,
    settings: Settings,
    read_only: Memo<bool>,
) -> NodeId {
    let sources = editor.watch::<PreferenceSourcesWatch>();
    let system = create_memo(move || sources.get().system.resolve());
    let developer = create_memo(move || sources.get().developer);
    let theme = DisplaySettings::theme_setting().with_default(Some(system.get_untracked().theme));
    let motion =
        DisplaySettings::motion_setting().with_default(Some(system.get_untracked().motion));
    let chosen_theme = settings.value(&theme);
    let below_theme = settings.fallback(&theme);
    let chosen_motion = settings.value(&motion);
    let below_motion = settings.fallback(&motion);
    let theme_row = settings.row(&theme);
    let motion_row = settings.row(&motion);
    let set_theme = settings.setter(&theme);
    let set_motion = settings.setter(&motion);
    let scale = DisplaySettings::scale_setting();
    let chosen_scale = settings.value(&scale);
    let below_scale = settings.fallback(&scale);
    let inherited_scale_label = create_memo(move || {
        format!(
            "Default ({}%)",
            below_scale.get().unwrap_or(DisplaySettings::DEFAULT_SCALE)
        )
    });
    let selected_scale = create_memo(move || Some(chosen_scale.get()));
    let scale_row = settings.row(&scale);
    let set_scale = settings.setter(&scale);
    let keyboard = DisplaySettings::keyboard_setting();
    let chosen_keyboard = settings.value(&keyboard);
    let below_keyboard = settings.fallback(&keyboard);
    let inherited_keyboard_label = create_memo(move || {
        format!(
            "Default ({}%)",
            below_keyboard
                .get()
                .unwrap_or(DisplaySettings::DEFAULT_KEYBOARD_HEIGHT)
        )
    });
    let selected_keyboard = create_memo(move || Some(chosen_keyboard.get()));
    let keyboard_row = settings.row(&keyboard);
    let set_keyboard = settings.setter(&keyboard);
    view! {
        <SettingsSection title="Appearance">
            <LayeredSettingsRow
                setting={theme_row}
                label="Theme"
                description="The colors of the app and every editor in it."
                locked={read_only}
                overridden={create_memo(move || developer.get().theme.is_some())}
                id={"display.theme".to_owned()}
            >
                <ThemeSelect
                    @sizing=ItemSize::Percent(100.0)
                    chosen={chosen_theme}
                    inherited={create_memo(move || below_theme.get().unwrap_or(system.get().theme))}
                    on_change={set_theme}
                    disabled={read_only}
                    @test_id={"display.theme"}
                />
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={motion_row}
                label="Motion"
                description="Reduced motion finishes animations at once, and stops inertia and blinking carets."
                separated=true
                locked={read_only}
                overridden={create_memo(move || developer.get().motion.is_some())}
                id={"display.motion".to_owned()}
            >
                <MotionSelect
                    @sizing=ItemSize::Percent(100.0)
                    chosen={chosen_motion}
                    inherited={create_memo(move || below_motion.get().unwrap_or(system.get().motion))}
                    on_change={set_motion}
                    disabled={read_only}
                    @test_id={"display.motion"}
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
                        <ChoiceOption value=None label={inherited_scale_label} />
                        <ForEach keys={DisplaySettings::SCALES.to_vec()}>|percent: u32|
                            <ChoiceOption value={Some(percent)} label={format!("{percent}%")} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected_scale, move |chosen: Option<Option<u32>>| {
                        if let Some(percent) = chosen {
                            set_scale.call(percent);
                        }
                    })}
                    label="Scale"
                    disabled={read_only}
                    @test_id={"display.scale"}
                />
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={keyboard_row}
                label="On-screen keyboard height"
                description="How much of the screen the app's own on-screen keyboard covers."
                separated=true
                locked={read_only}
                id={"display.keyboard".to_owned()}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption value=None label={inherited_keyboard_label} />
                        <ForEach keys={DisplaySettings::KEYBOARD_HEIGHTS.to_vec()}>|percent: u32|
                            <ChoiceOption value={Some(percent)} label={format!("{percent}%")} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected_keyboard, move |chosen: Option<Option<u32>>| {
                        if let Some(percent) = chosen {
                            set_keyboard.call(percent);
                        }
                    })}
                    label="On-screen keyboard height"
                    disabled={read_only}
                    @test_id={"display.keyboard"}
                />
            </LayeredSettingsRow>
        </SettingsSection>
    }
}

#[component]
fn ThemeSelect(
    chosen: Prop<Option<ThemeKind>>,
    inherited: Prop<ThemeKind>,
    on_change: Callback<Option<ThemeKind>>,
    disabled: Prop<bool>,
) -> NodeId {
    view! {
        <PreferenceSelect
            choices={ThemeKind::ALL.to_vec()}
            name={ThemeKind::name}
            inherited
            chosen
            on_change
            label="Theme"
            disabled
        />
    }
}

#[component]
fn MotionSelect(
    chosen: Prop<Option<Motion>>,
    inherited: Prop<Motion>,
    on_change: Callback<Option<Motion>>,
    disabled: Prop<bool>,
) -> NodeId {
    view! {
        <PreferenceSelect
            choices={Motion::ALL.to_vec()}
            name={Motion::name}
            inherited
            chosen
            on_change
            label="Motion"
            disabled
        />
    }
}

#[component]
fn PreferenceSelect<V>(
    choices: Vec<V>,
    name: fn(V) -> &'static str,
    inherited: Prop<V>,
    chosen: Prop<Option<V>>,
    on_change: Callback<Option<V>>,
    label: &'static str,
    disabled: Prop<bool>,
) -> NodeId
where
    V: Copy + PartialEq + Eq + std::hash::Hash + 'static,
{
    let inherited_label = create_memo(move || format!("Default ({})", name(inherited.get())));
    let selected = create_memo(move || Some(chosen.get()));
    view! {
        <Select
            width={Extent::fraction(1.0)}
            options={view! {
                <ChoiceOption value=None label={inherited_label} />
                <ForEach keys={choices}>|choice: V|
                    <ChoiceOption value={Some(choice)} label={name(choice)} />
                </ForEach>
            }}
            selected={Binding::new(selected, move |picked: Option<Option<V>>| {
                if let Some(picked) = picked {
                    on_change.call(picked);
                }
            })}
            label
            disabled
        />
    }
}
