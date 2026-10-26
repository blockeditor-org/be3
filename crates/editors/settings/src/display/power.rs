use block_editor_beui::LayeredSettingsRow;
use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::be_block::display_settings::{LockAfter, ScreenOff};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::modes::{LOCK_AFTER, SCREEN_OFF, lock_after_label, screen_off_label};
use super::ui::Settings;
use beui::{Extent, ItemSize};

#[component]
pub(super) fn PowerSection(settings: Settings, read_only: Memo<bool>) -> NodeId {
    let merged = settings.effective();
    let selected = create_memo(move || {
        let chosen = merged.get().screen_off();
        SCREEN_OFF.iter().position(|offered| *offered == chosen)
    });
    let locks = create_memo(move || {
        let chosen = merged.get().lock_after();
        LOCK_AFTER.iter().position(|offered| *offered == chosen)
    });
    let screen_off = DisplaySettings::screen_off_setting();
    let lock_after = DisplaySettings::lock_after_setting();
    let screen_off_row = settings.row(&screen_off);
    let lock_row = settings.row(&lock_after);
    let set_screen_off = settings.setter(&screen_off);
    let set_lock_after = settings.setter(&lock_after);
    view! {
        <SettingsSection title="Power and lock">
            <LayeredSettingsRow
                setting={screen_off_row}
                label="Turn off screens after"
                description="How long the session can go without input before every screen goes dark."
                locked={read_only}
                id={"display.screen-off".to_owned()}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={SCREEN_OFF.to_vec()}>|offered: ScreenOff|
                            <ChoiceOption label={screen_off_label(offered)} />
                        </ForEach>
                    }}
                    selected={Binding::new(selected, move |index: Option<usize>| {
                        if let Some(chosen) = index.and_then(|index| SCREEN_OFF.get(index)) {
                            set_screen_off.call(Some(*chosen));
                        }
                    })}
                    label="Turn off screens after"
                    disabled={read_only}
                    @test_id={"display.screen-off"}
                />
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={lock_row}
                label="Lock the screen after"
                separated=true
                locked={read_only}
                id={"display.lock-after".to_owned()}
            >
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ForEach keys={LOCK_AFTER.to_vec()}>|offered: LockAfter|
                            <ChoiceOption label={lock_after_label(offered)} />
                        </ForEach>
                    }}
                    selected={Binding::new(locks, move |index: Option<usize>| {
                        if let Some(chosen) = index.and_then(|index| LOCK_AFTER.get(index)) {
                            set_lock_after.call(Some(*chosen));
                        }
                    })}
                    label="Lock the screen after"
                    disabled={read_only}
                    @test_id={"display.lock-after"}
                />
            </LayeredSettingsRow>
        </SettingsSection>
    }
}
