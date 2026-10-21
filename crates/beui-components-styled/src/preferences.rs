use beui_components_unstyled::{ChoiceOption, MenuItem};
use beui_core::icons::ICON_SETTINGS_BACKUP_RESTORE;
use beui_core::node::NodeId;
use beui_core::preferences::{Motion, PreferenceLayer, PreferenceSource, ThemeKind};
use beui_macros::{component, view};
use beui_view::reactive::*;

use crate::select::Select;
use crate::settings::{SettingsRow, SettingsSection};

pub const DEVELOPER_OVERRIDE: &str = "Overridden by developer tools";
const RESET: &str = "Reset to default";

#[component]
pub fn ThemeSelect(
    chosen: Prop<Option<ThemeKind>>,
    inherited: Prop<ThemeKind>,
    on_change: Callback<Option<ThemeKind>>,
    #[prop(default = false)] disabled: Prop<bool>,
) -> NodeId {
    view! {
        <PreferenceSelect
            labels={create_memo(move || labels(&ThemeKind::ALL, ThemeKind::name, inherited.get()))}
            selected={create_memo(move || Some(index_of(&ThemeKind::ALL, chosen.get())))}
            on_select={move |index: Option<usize>| on_change.call(choice_at(&ThemeKind::ALL, index))}
            label="Theme"
            disabled
        />
    }
}

#[component]
pub fn MotionSelect(
    chosen: Prop<Option<Motion>>,
    inherited: Prop<Motion>,
    on_change: Callback<Option<Motion>>,
    #[prop(default = false)] disabled: Prop<bool>,
) -> NodeId {
    view! {
        <PreferenceSelect
            labels={create_memo(move || labels(&Motion::ALL, Motion::name, inherited.get()))}
            selected={create_memo(move || Some(index_of(&Motion::ALL, chosen.get())))}
            on_select={move |index: Option<usize>| on_change.call(choice_at(&Motion::ALL, index))}
            label="Motion"
            disabled
        />
    }
}

#[component]
fn PreferenceSelect(
    labels: Memo<Vec<String>>,
    selected: Memo<Option<usize>>,
    on_select: Callback<Option<usize>>,
    label: &'static str,
    disabled: Prop<bool>,
) -> NodeId {
    view! {
        <Select
            width={Extent::fraction(1.0)}
            options={view! {
                <ForEach keys={labels}>|label: String|
                    <ChoiceOption label />
                </ForEach>
            }}
            selected={Binding::new(selected, move |index: Option<usize>| on_select.call(index))}
            label
            disabled
        />
    }
}

fn labels<V: Copy>(choices: &[V], name: fn(V) -> &'static str, inherited: V) -> Vec<String> {
    std::iter::once(format!("Default ({})", name(inherited)))
        .chain(choices.iter().map(|choice| name(*choice).to_owned()))
        .collect()
}

fn index_of<V: PartialEq>(choices: &[V], chosen: Option<V>) -> usize {
    chosen
        .and_then(|chosen| choices.iter().position(|choice| *choice == chosen))
        .map_or(0, |index| index + 1)
}

fn choice_at<V: Copy>(choices: &[V], index: Option<usize>) -> Option<V> {
    index
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| choices.get(index).copied())
}

#[component]
pub fn PreferencesSection(
    #[prop(default = String::from("Appearance"))] title: Prop<String>,
) -> NodeId {
    let layers = with_document(|document| document.watch_preference_layers());
    let app = create_memo(move || layers.get().layer(PreferenceSource::App));
    let inherited = create_memo(move || layers.get().below(PreferenceSource::App).resolve());
    let developer = create_memo(move || layers.get().layer(PreferenceSource::Developer));
    let theme = create_memo(move || app.get().theme);
    let motion = create_memo(move || app.get().motion);
    let set_theme = Callback::new(move |theme: Option<ThemeKind>| {
        change_app_layer(move |layer| PreferenceLayer { theme, ..layer });
    });
    let set_motion = Callback::new(move |motion: Option<Motion>| {
        change_app_layer(move |layer| PreferenceLayer { motion, ..layer });
    });
    let theme_menu: Children<MenuItem> = [view! {
        <MenuItem
            label=RESET
            glyph={ICON_SETTINGS_BACKUP_RESTORE.to_owned()}
            disabled={create_memo(move || theme.get().is_none())}
            row_test_id={"preferences.theme.reset".to_owned()}
            on_click={move || set_theme.call(None)}
        />
    }]
    .into_iter()
    .collect();
    let motion_menu: Children<MenuItem> = [view! {
        <MenuItem
            label=RESET
            glyph={ICON_SETTINGS_BACKUP_RESTORE.to_owned()}
            disabled={create_memo(move || motion.get().is_none())}
            row_test_id={"preferences.motion.reset".to_owned()}
            on_click={move || set_motion.call(None)}
        />
    }]
    .into_iter()
    .collect();
    view! {
        <SettingsSection title>
            <SettingsRow
                label="Theme"
                description="The colors of everything in this app."
                modified={create_memo(move || theme.get().is_some())}
                note={overridden(create_memo(move || developer.get().theme.is_some()))}
                menu_test_id={"preferences.theme.menu".to_owned()}
                menu={theme_menu}
            >
                <ThemeSelect
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"preferences.theme"}
                    chosen={theme}
                    inherited={create_memo(move || inherited.get().theme)}
                    on_change={set_theme}
                />
            </SettingsRow>
            <SettingsRow
                label="Motion"
                description="Reduced motion finishes animations at once, and stops inertia and blinking carets."
                separated=true
                modified={create_memo(move || motion.get().is_some())}
                note={overridden(create_memo(move || developer.get().motion.is_some()))}
                menu_test_id={"preferences.motion.menu".to_owned()}
                menu={motion_menu}
            >
                <MotionSelect
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"preferences.motion"}
                    chosen={motion}
                    inherited={create_memo(move || inherited.get().motion)}
                    on_change={set_motion}
                />
            </SettingsRow>
        </SettingsSection>
    }
}

fn overridden(by_developer: Memo<bool>) -> Memo<String> {
    create_memo(move || match by_developer.get() {
        true => DEVELOPER_OVERRIDE.to_owned(),
        false => String::new(),
    })
}

fn change_app_layer(change: impl FnOnce(PreferenceLayer) -> PreferenceLayer) {
    with_document(|document| {
        if let Some(context) = document.context() {
            let layer = context.preference_layer(PreferenceSource::App);
            context.set_preference_layer(PreferenceSource::App, change(layer));
        }
    });
}
