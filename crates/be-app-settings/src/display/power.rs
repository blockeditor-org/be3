use be_sdk_beui::LayeredSettingsRow;
use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::display_settings::{LockAfter, ScreenOff};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;

use super::modes::{LOCK_AFTER, SCREEN_OFF, lock_after_label, screen_off_label};
use super::ui::Settings;
use beui::Extent;

#[component]
pub(super) fn PowerSection(settings: Settings, read_only: Memo<bool>) -> NodeId {
    let merged = settings.effective();
    let selected = create_memo(move || Some(merged.get().screen_off()));
    let locks = create_memo(move || Some(merged.get().lock_after()));
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
                <ListItem grow=1.0>
                    <Select
                        width={Extent::fraction(1.0)}
                        options={view! {
                            <ForEach value={SCREEN_OFF.to_vec()}>|offered: ScreenOff|
                                <ChoiceOption value=offered label={screen_off_label(offered)} />
                            </ForEach>
                        }}
                        selected={Binding::new(selected, move |chosen: Option<ScreenOff>| {
                            if let Some(chosen) = chosen {
                                set_screen_off.call(Some(chosen));
                            }
                        })}
                        label="Turn off screens after"
                        disabled={read_only}
                        @test_id={"display.screen-off"}
                    />
                </ListItem>
            </LayeredSettingsRow>
            <LayeredSettingsRow
                setting={lock_row}
                label="Lock the screen after"
                separated=true
                locked={read_only}
                id={"display.lock-after".to_owned()}
            >
                <ListItem grow=1.0>
                    <Select
                        width={Extent::fraction(1.0)}
                        options={view! {
                            <ForEach value={LOCK_AFTER.to_vec()}>|offered: LockAfter|
                                <ChoiceOption value=offered label={lock_after_label(offered)} />
                            </ForEach>
                        }}
                        selected={Binding::new(locks, move |chosen: Option<LockAfter>| {
                            if let Some(chosen) = chosen {
                                set_lock_after.call(Some(chosen));
                            }
                        })}
                        label="Lock the screen after"
                        disabled={read_only}
                        @test_id={"display.lock-after"}
                    />
                </ListItem>
            </LayeredSettingsRow>
        </SettingsSection>
    }
}
