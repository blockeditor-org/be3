use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::be_block::display_settings::{LockAfter, ScreenOff};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;

use super::modes::{LOCK_AFTER, SCREEN_OFF, lock_after_label, screen_off_label};
use super::ui::Settings;

#[component]
pub(super) fn PowerSection(
    settings: Settings,
    root: ReadSignal<DisplaySettings>,
    merged: Memo<DisplaySettings>,
    read_only: Memo<bool>,
) -> NodeId {
    let selected = create_memo(move || {
        let chosen = merged.get().screen_off();
        SCREEN_OFF.iter().position(|offered| *offered == chosen)
    });
    let locks = create_memo(move || {
        let chosen = merged.get().lock_after();
        LOCK_AFTER.iter().position(|offered| *offered == chosen)
    });
    let screen_off_set = create_memo(move || root.get().screen_off.is_some());
    let lock_set = create_memo(move || root.get().lock_after.is_some());
    view! {
        <SettingsSection title="Power and lock">
            <SettingsRow
                label="Turn off screens after"
                description="How long the session can go without input before every screen goes dark."
                modified={screen_off_set}
                locked={read_only}
                reset_test_id="display.screen-off.reset"
                on_reset={move || settings.operate(DisplaySettings::set_screen_off(None))}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={SCREEN_OFF.to_vec()}>|screen_off: ScreenOff|
                            <ChoiceOption label={screen_off_label(screen_off)} />
                        </ForEach>
                    }}
                    selected={selected}
                    label="Turn off screens after"
                    disabled={read_only}
                    @test_id={"display.screen-off"}
                    on_change={move |index: Option<usize>| {
                        if let Some(screen_off) = index.and_then(|index| SCREEN_OFF.get(index)) {
                            settings.operate(DisplaySettings::set_screen_off(Some(*screen_off)));
                        }
                    }}
                />
            </SettingsRow>
            <SettingsRow
                label="Lock the screen after"
                separated=true
                modified={lock_set}
                locked={read_only}
                reset_test_id="display.lock-after.reset"
                on_reset={move || settings.operate(DisplaySettings::set_lock_after(None))}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={LOCK_AFTER.to_vec()}>|lock_after: LockAfter|
                            <ChoiceOption label={lock_after_label(lock_after)} />
                        </ForEach>
                    }}
                    selected={locks}
                    label="Lock the screen after"
                    disabled={read_only}
                    @test_id={"display.lock-after"}
                    on_change={move |index: Option<usize>| {
                        if let Some(lock_after) = index.and_then(|index| LOCK_AFTER.get(index)) {
                            settings.operate(DisplaySettings::set_lock_after(Some(*lock_after)));
                        }
                    }}
                />
            </SettingsRow>
        </SettingsSection>
    }
}
